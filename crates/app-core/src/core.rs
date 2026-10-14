//! The session service: consumes LCU events, keeps the session state, runs
//! the import once per champion the player hovers or locks in, and publishes
//! view models to the UI.

use crate::import::{self, run_import, BuildLookup, ImportReport, StepOutcome};
use crate::overlay::{overlay_view, OverlayChoices, OverlayInputs};
use crate::settings_store::{SettingsError, SettingsStore};
use domain::builds::Build;
use domain::champ_select::{selected_pick, ChampionPick, GameMode};
use domain::objectives::{self, ObjectiveKill, Pit};
use domain::ports::{BuildSource, LcuEvent, LcuPort, LiveSnapshot};
use domain::session::{transition, SessionEvent, SessionState};
use domain::settings::Settings;
use domain::shopping::Purchases;
use domain::static_catalog::StaticCatalog;
use domain::summoner::Summoner;
use ipc_types::{
    ImportReportDto, ObjectiveTimerDto, ObjectiveTimersDto, OverlayCommand, OverlayMessage, PitDto,
    RuneCatalogDto, RuneDto, RuneTreeDto, SessionStateDto, StatusDto, SummonerDto,
};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use thiserror::Error;

/// Where the core publishes state. The desktop app forwards these as Tauri events.
pub trait EventSink: Send + Sync + fmt::Debug {
    fn status(&self, status: &StatusDto);
    fn import(&self, report: &ImportReportDto);
    /// The in-game item column's view; sent only when it changed.
    fn overlay(&self, message: &OverlayMessage);
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("The League client is not running")]
    NotConnected,
    #[error("Pick a champion first")]
    NothingToImport,
    #[error("Game data is still loading")]
    StaticDataLoading,
    #[error("That rune page is no longer available")]
    NoSuchRunePage,
    #[error("You picked another champion meanwhile")]
    PickChanged,
    #[error(transparent)]
    Settings(#[from] SettingsError),
}

impl CoreError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotConnected => "not_connected",
            Self::NothingToImport => "nothing_to_import",
            Self::StaticDataLoading => "static_data_loading",
            Self::NoSuchRunePage => "no_such_rune_page",
            Self::PickChanged => "pick_changed",
            Self::Settings(_) => "settings_io",
        }
    }
}

#[derive(Debug)]
struct State {
    session: SessionState,
    port: Option<Arc<dyn LcuPort>>,
    summoner: Option<Summoner>,
    catalog: Option<Arc<StaticCatalog>>,
    settings: Settings,
    /// The pick we last imported for, so each champion is imported once,
    /// however many session updates arrive while it stays hovered or locked.
    imported_for: Option<ChampionPick>,
    /// The current hover or lock, kept for "import again".
    current_pick: Option<ChampionPick>,
    /// The build last imported and the pick it is for, so the player can
    /// switch rune pages without asking the sources again.
    build: Option<(ChampionPick, Build)>,
    /// Index of the rune page of `build` that was last sent to the client.
    selected_rune_page: usize,
    last_report: Option<ImportReportDto>,
    /// Items seen in the inventory during the current game.
    purchases: Purchases,
    /// What the player changed in the column this game.
    choices: OverlayChoices,
    /// Latest read of the running game, `None` outside one.
    live: Option<LiveSnapshot>,
    /// Last view sent to the overlay, to send only changes.
    last_overlay: Option<OverlayMessage>,
    /// Last objective timers sent to the overlay, to send only changes.
    last_objectives: Option<ObjectiveTimersDto>,
    /// Kills from before a Practice Tool reset, which no longer count.
    kills_before_reset: Vec<ObjectiveKill>,
}

impl State {
    fn overlay_message(&self) -> OverlayMessage {
        let (Some((pick, build)), Some(catalog)) = (&self.build, &self.catalog) else {
            return OverlayMessage::Clear;
        };
        let role = match pick.mode {
            GameMode::Aram => pick.role_label(),
            GameMode::SummonersRift => build.role.label(),
        };
        OverlayMessage::Items(Box::new(overlay_view(OverlayInputs {
            build,
            champion_name: &catalog.champion_name(pick.champion_id),
            role,
            catalog,
            purchases: &self.purchases,
            choices: &self.choices,
            live: self.live.as_ref(),
        })))
    }

    /// Epic objective timers, from the latest read of a Summoner's Rift game.
    fn objective_timers(&self) -> Option<ObjectiveTimersDto> {
        let live = self.live.as_ref().filter(|l| l.summoners_rift)?;
        Some(ObjectiveTimersDto {
            game_time_ms: live.game_time_ms,
            timers: objectives::upcoming(
                &objectives::since_reset(&live.objective_kills, &self.kills_before_reset),
                live.game_time_ms,
            )
            .into_iter()
            .map(|u| ObjectiveTimerDto {
                pit: match u.pit {
                    Pit::Dragon => PitDto::Dragon,
                    Pit::Baron => PitDto::Baron,
                    Pit::ScuttleTop => PitDto::ScuttleTop,
                    Pit::ScuttleBottom => PitDto::ScuttleBottom,
                },
                label: u.monster.label().to_owned(),
                spawn_at_ms: u.spawn_at_ms,
            })
            .collect(),
        })
    }

    /// A new game (or none): forget what was bought.
    fn reset_game(&mut self) {
        self.purchases = Purchases::default();
        self.kills_before_reset.clear();
        self.choices = OverlayChoices::default();
        self.live = None;
    }
}

#[derive(Debug)]
pub struct Core {
    state: Mutex<State>,
    /// Asked in order; the first one with a build wins.
    sources: Vec<Arc<dyn BuildSource>>,
    settings_store: SettingsStore,
    sink: Arc<dyn EventSink>,
    /// Held for a whole import. "Import again" runs beside the event loop,
    /// and a slow import finishing after a newer one would undo it.
    import_running: tokio::sync::Mutex<()>,
}

impl Core {
    pub fn new(
        sources: Vec<Arc<dyn BuildSource>>,
        settings_store: SettingsStore,
        sink: Arc<dyn EventSink>,
    ) -> Arc<Self> {
        let settings = settings_store.load();
        Arc::new(Self {
            state: Mutex::new(State {
                session: SessionState::Disconnected,
                port: None,
                summoner: None,
                catalog: None,
                settings,
                imported_for: None,
                current_pick: None,
                build: None,
                selected_rune_page: 0,
                last_report: None,
                purchases: Purchases::default(),
                choices: OverlayChoices::default(),
                live: None,
                last_overlay: None,
                last_objectives: None,
                kills_before_reset: Vec::new(),
            }),
            sources,
            settings_store,
            sink,
            import_running: tokio::sync::Mutex::new(()),
        })
    }

    // A poisoned lock only means another thread panicked mid-update; the
    // state itself is still usable, so recover instead of crashing the app.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn status(&self) -> StatusDto {
        let s = self.lock();
        StatusDto {
            session: session_dto(s.session),
            summoner: s.summoner.as_ref().map(|sum| SummonerDto {
                riot_id: sum.riot_id(),
                level: sum.summoner_level,
                profile_icon_url: s
                    .catalog
                    .as_ref()
                    .and_then(|c| c.profile_icon_url(sum.profile_icon_id)),
            }),
            patch: s.catalog.as_ref().map(|c| c.version.clone()),
        }
    }

    pub fn settings(&self) -> Settings {
        self.lock().settings.clone()
    }

    pub fn save_settings(&self, settings: Settings) -> Result<(), CoreError> {
        self.settings_store.save(&settings)?;
        self.lock().settings = settings;
        Ok(())
    }

    pub fn last_import(&self) -> Option<ImportReportDto> {
        self.lock().last_report.clone()
    }

    /// Rune trees and stat shards of the current patch, `None` while loading.
    pub fn rune_catalog(&self) -> Option<RuneCatalogDto> {
        let catalog = self.lock().catalog.clone()?;
        let rune = |id: &i64| {
            let info = catalog.runes.get(id);
            RuneDto {
                id: i32::try_from(*id).unwrap_or(0),
                name: info.map_or_else(|| format!("Rune {id}"), |i| i.name.clone()),
                icon_url: info.and_then(|i| catalog.rune_icon_url(&i.icon)),
                description: info.map(|i| i.description.clone()).unwrap_or_default(),
            }
        };
        let row = |ids: &Vec<i64>| ids.iter().map(rune).collect();
        Some(RuneCatalogDto {
            trees: catalog
                .rune_trees
                .iter()
                .map(|t| RuneTreeDto {
                    id: i32::try_from(t.id).unwrap_or(0),
                    name: t.name.clone(),
                    icon_url: catalog.rune_icon_url(&t.icon),
                    slots: t.slots.iter().map(row).collect(),
                })
                .collect(),
            stat_shard_rows: catalog.stat_shard_rows.iter().map(row).collect(),
        })
    }

    pub fn set_catalog(&self, catalog: Arc<StaticCatalog>) {
        self.lock().catalog = Some(catalog);
        self.publish_status();
    }

    fn publish_status(&self) {
        let status = self.status();
        self.sink.status(&status);
    }

    /// A game is running (the live game data is worth reading).
    pub fn in_game(&self) -> bool {
        self.lock().session == SessionState::InGame
    }

    /// The overlay's current view (sent to a freshly started overlay).
    pub fn overlay(&self) -> OverlayMessage {
        self.lock().overlay_message()
    }

    /// Recomputes the overlay view and sends it if it changed.
    fn publish_overlay(&self) {
        let changed = {
            let mut s = self.lock();
            let message = s.overlay_message();
            (s.last_overlay.as_ref() != Some(&message)).then(|| {
                s.last_overlay = Some(message.clone());
                message
            })
        };
        if let Some(message) = changed {
            self.sink.overlay(&message);
        }
        self.publish_objectives();
    }

    /// Sends the objective timers when they changed (in game: every read,
    /// as the clock moves), and `None` once when the game is gone.
    fn publish_objectives(&self) {
        let changed = {
            let mut s = self.lock();
            let timers = s.objective_timers();
            (s.last_objectives != timers).then(|| {
                s.last_objectives.clone_from(&timers);
                timers
            })
        };
        if let Some(timers) = changed {
            self.sink.overlay(&OverlayMessage::Objectives(timers));
        }
    }

    /// Something the player did in the column (chose or removed an item).
    pub fn overlay_command(&self, command: OverlayCommand) {
        self.lock().choices.apply(command);
        self.publish_overlay();
    }

    /// Feeds one read of the running game (`None`: no game data right now).
    /// Call about once a second while in game.
    pub fn live_tick(&self, snapshot: Option<LiveSnapshot>) {
        {
            let mut s = self.lock();
            // A read that was under way when the game ended.
            if s.session != SessionState::InGame {
                return;
            }
            if let Some(snap) = &snapshot {
                // The Practice Tool's "Reset Game" turns the clock back: what
                // happened before no longer counts. (Fast-forward only moves
                // it on, which the timers follow by themselves.)
                let previous = s
                    .live
                    .as_ref()
                    .map(|l| (l.game_time_ms, l.objective_kills.clone()));
                if let Some((was_ms, kills)) = previous {
                    if objectives::is_reset(was_ms, snap.game_time_ms) {
                        tracing::info!(from_ms = was_ms, to_ms = snap.game_time_ms, "game reset");
                        for kill in kills.iter().chain(&snap.objective_kills) {
                            if !s.kills_before_reset.contains(kill) {
                                s.kills_before_reset.push(*kill);
                            }
                        }
                        s.purchases = Purchases::default();
                    }
                }
                s.purchases.observe(&snap.inventory);
                // Once per game: which mode it is and whether it is timed,
                // so a player missing timers can see why in the log.
                let mode = s.live.as_ref().map(|l| &l.game_mode);
                if mode != Some(&snap.game_mode) {
                    tracing::info!(
                        game_mode = %snap.game_mode,
                        objective_timers = snap.summoners_rift,
                        "reading the live game"
                    );
                }
            }
            s.live = snapshot;
        }
        self.publish_overlay();
    }

    /// Handles one LCU event. Call sequentially from a single task.
    pub async fn handle(&self, event: LcuEvent) {
        match event {
            LcuEvent::Connected(port) => {
                {
                    let mut s = self.lock();
                    s.session = transition(s.session, &SessionEvent::ClientConnected);
                    s.port = Some(port.clone());
                }
                match port.current_summoner().await {
                    Ok(summoner) => self.lock().summoner = Some(summoner),
                    Err(e) => tracing::warn!(error = %e, "could not load the current summoner"),
                }
                self.publish_status();
            }
            LcuEvent::Disconnected => {
                {
                    let mut s = self.lock();
                    s.session = SessionState::Disconnected;
                    s.port = None;
                    s.summoner = None;
                    s.imported_for = None;
                    s.current_pick = None;
                    s.build = None;
                    s.reset_game();
                }
                self.publish_status();
                self.publish_overlay();
            }
            LcuEvent::Phase(phase) => {
                let changed = {
                    let mut s = self.lock();
                    let next = transition(s.session, &SessionEvent::PhaseChanged(phase));
                    if next != SessionState::ChampSelect {
                        s.imported_for = None;
                        s.current_pick = None;
                    }
                    if next != SessionState::InGame {
                        s.reset_game();
                    }
                    std::mem::replace(&mut s.session, next) != next
                };
                if changed {
                    self.publish_status();
                    self.publish_overlay();
                }
            }
            LcuEvent::ChampSelect(None) => {}
            LcuEvent::ChampSelect(Some(session)) => {
                let Some(pick) = selected_pick(&session) else {
                    return;
                };
                let should_import = {
                    let mut s = self.lock();
                    s.current_pick = Some(pick);
                    s.imported_for != Some(pick)
                };
                if should_import {
                    if let Err(e) = self.import(pick).await {
                        tracing::info!(error = %e, "import skipped");
                    }
                }
            }
        }
    }

    /// Runs the import again for the current lock (the UI's "Import again").
    pub async fn reimport(&self) -> Result<ImportReportDto, CoreError> {
        let pick = self.lock().current_pick.ok_or(CoreError::NothingToImport)?;
        self.import(pick).await
    }

    /// Asks each source in turn. A source that fails is skipped, so OP.GG
    /// being unreachable falls back to the seed builds.
    async fn lookup(&self, pick: ChampionPick, catalog: &StaticCatalog) -> BuildLookup {
        let mut first_failure: Option<String> = None;
        for source in &self.sources {
            match source.build(pick).await {
                Ok(Some(mut build)) => {
                    let all = build.rune_pages.clone();
                    if let Some(e) = build.retain_valid_rune_pages(catalog) {
                        tracing::warn!(source = source.name(), error = %e, "dropped rune pages invalid this patch");
                        if build.rune_pages.is_empty() {
                            // Keep them so the runes step can say why it skipped.
                            build.rune_pages = all;
                        }
                    }
                    if first_failure.is_some() {
                        build.source = format!("{} (live statistics unavailable)", build.source);
                    }
                    return BuildLookup::Found(build);
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(source = source.name(), error = %e, "build source failed");
                    first_failure
                        .get_or_insert_with(|| format!("Could not reach {}: {e}", source.name()));
                }
            }
        }
        first_failure.map_or(BuildLookup::Missing, BuildLookup::Unavailable)
    }

    fn is_current(&self, pick: ChampionPick) -> bool {
        self.lock().current_pick == Some(pick)
    }

    async fn import(&self, pick: ChampionPick) -> Result<ImportReportDto, CoreError> {
        let _running = self.import_running.lock().await;
        if !self.is_current(pick) {
            return Err(CoreError::PickChanged);
        }
        let (port, catalog, settings, summoner_id, previous_page) = {
            let mut s = self.lock();
            let port = s.port.clone().ok_or(CoreError::NotConnected)?;
            let catalog = s.catalog.clone().ok_or(CoreError::StaticDataLoading)?;
            s.imported_for = Some(pick);
            // "Import again" keeps the rune page the player chose.
            let same_pick = s.build.as_ref().is_some_and(|(p, _)| *p == pick);
            (
                port,
                catalog,
                s.settings.clone(),
                s.summoner.as_ref().map_or(0, |x| x.summoner_id),
                if same_pick { s.selected_rune_page } else { 0 },
            )
        };
        let lookup = self.lookup(pick, &catalog).await;
        if !self.is_current(pick) {
            return Err(CoreError::PickChanged);
        }
        let rune_page = match &lookup {
            BuildLookup::Found(b) if previous_page < b.rune_pages.len() => previous_page,
            _ => 0,
        };
        let report: ImportReport = run_import(
            port.as_ref(),
            pick,
            &lookup,
            rune_page,
            &catalog,
            &settings,
            summoner_id,
        )
        .await;
        tracing::info!(champion = %report.champion_name, ?report, "import finished");
        let dto = ImportReportDto::from(report);
        {
            let mut s = self.lock();
            s.build = match lookup {
                BuildLookup::Found(build) => Some((pick, build)),
                BuildLookup::Missing | BuildLookup::Unavailable(_) => None,
            };
            s.selected_rune_page = rune_page;
            s.last_report = Some(dto.clone());
            s.reset_game();
        }
        self.sink.import(&dto);
        self.publish_overlay();
        Ok(dto)
    }

    /// Sends rune page `index` of the current build (the player chose it).
    pub async fn apply_rune_page(&self, index: usize) -> Result<ImportReportDto, CoreError> {
        let (port, catalog, settings, pick, page) = {
            let s = self.lock();
            let port = s.port.clone().ok_or(CoreError::NotConnected)?;
            let catalog = s.catalog.clone().ok_or(CoreError::StaticDataLoading)?;
            let (pick, build) = s.build.as_ref().ok_or(CoreError::NothingToImport)?;
            let page = build
                .rune_pages
                .get(index)
                .cloned()
                .ok_or(CoreError::NoSuchRunePage)?;
            (port, catalog, s.settings.clone(), *pick, page)
        };
        let champion = catalog.champion_name(pick.champion_id);
        let outcome =
            import::apply_rune_page(port.as_ref(), &page, &catalog, &champion, &settings).await;
        tracing::info!(%champion, index, ?outcome, "rune page chosen");
        let applied = matches!(outcome, StepOutcome::Done(_));
        let dto = {
            let mut s = self.lock();
            // The player may have picked another champion meanwhile.
            if s.build.as_ref().map(|(p, _)| *p) != Some(pick) {
                return Err(CoreError::NoSuchRunePage);
            }
            if applied {
                s.selected_rune_page = index;
            }
            let report = s.last_report.as_mut().ok_or(CoreError::NothingToImport)?;
            report.runes = outcome.into();
            if applied {
                report.selected_rune_page = u32::try_from(index).ok();
            }
            report.clone()
        };
        self.sink.import(&dto);
        Ok(dto)
    }
}

fn session_dto(s: SessionState) -> SessionStateDto {
    match s {
        SessionState::Disconnected => SessionStateDto::Disconnected,
        SessionState::ClientReady => SessionStateDto::ClientReady,
        SessionState::Lobby => SessionStateDto::Lobby,
        SessionState::ChampSelect => SessionStateDto::ChampSelect,
        SessionState::InGame => SessionStateDto::InGame,
        SessionState::PostGame => SessionStateDto::PostGame,
    }
}
