//! The session service: consumes LCU events, keeps the session state, runs
//! the import once per locked pick, and publishes view models to the UI.

use crate::import::{run_import, ImportReport};
use crate::settings_store::{SettingsError, SettingsStore};
use domain::builds::BuildCatalog;
use domain::champ_select::{locked_pick, LockedPick};
use domain::ports::{LcuEvent, LcuPort};
use domain::session::{transition, SessionEvent, SessionState};
use domain::settings::Settings;
use domain::static_catalog::StaticCatalog;
use domain::summoner::Summoner;
use ipc_types::{ImportReportDto, SessionStateDto, StatusDto, SummonerDto};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use thiserror::Error;

/// Where the core publishes state. The desktop app forwards these as Tauri events.
pub trait EventSink: Send + Sync + fmt::Debug {
    fn status(&self, status: &StatusDto);
    fn import(&self, report: &ImportReportDto);
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("The League client is not running")]
    NotConnected,
    #[error("Lock in a champion first")]
    NothingToImport,
    #[error("Game data is still loading")]
    StaticDataLoading,
    #[error(transparent)]
    Settings(#[from] SettingsError),
}

impl CoreError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotConnected => "not_connected",
            Self::NothingToImport => "nothing_to_import",
            Self::StaticDataLoading => "static_data_loading",
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
    /// The pick we last imported for, so we import once per champ select.
    imported_for: Option<LockedPick>,
    /// The current lock, kept for "import again".
    current_pick: Option<LockedPick>,
    last_report: Option<ImportReportDto>,
}

#[derive(Debug)]
pub struct Core {
    state: Mutex<State>,
    builds: BuildCatalog,
    settings_store: SettingsStore,
    sink: Arc<dyn EventSink>,
}

impl Core {
    pub fn new(
        builds: BuildCatalog,
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
                last_report: None,
            }),
            builds,
            settings_store,
            sink,
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

    pub fn set_catalog(&self, catalog: Arc<StaticCatalog>) {
        self.lock().catalog = Some(catalog);
        self.publish_status();
    }

    fn publish_status(&self) {
        let status = self.status();
        self.sink.status(&status);
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
                }
                self.publish_status();
            }
            LcuEvent::Phase(phase) => {
                let changed = {
                    let mut s = self.lock();
                    let next = transition(s.session, &SessionEvent::PhaseChanged(phase));
                    if next != SessionState::ChampSelect {
                        s.imported_for = None;
                        s.current_pick = None;
                    }
                    std::mem::replace(&mut s.session, next) != next
                };
                if changed {
                    self.publish_status();
                }
            }
            LcuEvent::ChampSelect(None) => {}
            LcuEvent::ChampSelect(Some(session)) => {
                let Some(pick) = locked_pick(&session) else {
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

    async fn import(&self, pick: LockedPick) -> Result<ImportReportDto, CoreError> {
        let (port, catalog, settings, summoner_id) = {
            let mut s = self.lock();
            let port = s.port.clone().ok_or(CoreError::NotConnected)?;
            let catalog = s.catalog.clone().ok_or(CoreError::StaticDataLoading)?;
            s.imported_for = Some(pick);
            (
                port,
                catalog,
                s.settings.clone(),
                s.summoner.as_ref().map_or(0, |x| x.summoner_id),
            )
        };
        let build = self.builds.select(pick.champion_id, pick.role);
        let report: ImportReport =
            run_import(port.as_ref(), pick, build, &catalog, &settings, summoner_id).await;
        tracing::info!(champion = %report.champion_name, ?report, "import finished");
        let dto = ImportReportDto::from(report);
        self.lock().last_report = Some(dto.clone());
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
