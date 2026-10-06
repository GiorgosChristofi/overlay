//! The champ-select import: rune page, item set and summoner spells.
//! Each step runs independently, so one failure never blocks the others.

use domain::builds::{Build, RunePageOption};
use domain::champ_select::{ChampionPick, GameMode};
use domain::item_set::item_set_for;
use domain::ports::LcuPort;
use domain::runes::{self, NewRunePage, SlotPlan};
use domain::settings::Settings;
use domain::spells::{arrange, FlashKey};
use domain::static_catalog::StaticCatalog;
use ipc_types::{ImportReportDto, RunePageDto, StepResultDto};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    Done(String),
    Skipped(String),
    Failed(String),
}

/// What looking up a build for a pick found.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildLookup {
    Found(Build),
    /// No source has a build for this champion.
    Missing,
    /// No source had a build, and at least one could not be asked.
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportReport {
    pub champion_name: String,
    pub role: String,
    pub runes: StepOutcome,
    pub item_set: StepOutcome,
    pub summoner_spells: StepOutcome,
    pub source: Option<String>,
    pub rune_pages: Vec<RunePageOption>,
    pub selected_rune_page: Option<usize>,
}

impl From<StepOutcome> for StepResultDto {
    fn from(o: StepOutcome) -> Self {
        match o {
            StepOutcome::Done(detail) => Self::Done { detail },
            StepOutcome::Skipped(reason) => Self::Skipped { reason },
            StepOutcome::Failed(error) => Self::Failed { error },
        }
    }
}

/// Ids fit in `i32`; anything else is not a real rune and maps to 0.
fn small(id: i64) -> i32 {
    i32::try_from(id).unwrap_or(0)
}

#[allow(clippy::cast_possible_truncation)] // rates are 0..=1; f32 is plenty for display
pub(crate) fn rune_page_dto(page: &RunePageOption) -> RunePageDto {
    let stats = page.stats.as_ref();
    RunePageDto {
        primary_style_id: small(page.runes.primary_style_id),
        sub_style_id: small(page.runes.sub_style_id),
        perk_ids: page.runes.perk_ids.iter().copied().map(small).collect(),
        games: stats.map(|s| s.games),
        win_rate: stats.and_then(|s| s.win_rate()).map(|r| r as f32),
        pick_rate: stats.map(|s| s.pick_rate as f32),
    }
}

impl From<ImportReport> for ImportReportDto {
    fn from(r: ImportReport) -> Self {
        Self {
            champion_name: r.champion_name,
            role: r.role,
            runes: r.runes.into(),
            item_set: r.item_set.into(),
            summoner_spells: r.summoner_spells.into(),
            source: r.source,
            rune_pages: r.rune_pages.iter().map(rune_page_dto).collect(),
            selected_rune_page: r.selected_rune_page.and_then(|i| u32::try_from(i).ok()),
        }
    }
}

const TURNED_OFF: &str = "Turned off in settings";

/// Runs every enabled step for a locked pick, using rune page `rune_page`
/// of the build.
#[allow(clippy::too_many_arguments)]
pub async fn run_import(
    port: &dyn LcuPort,
    pick: ChampionPick,
    lookup: &BuildLookup,
    rune_page: usize,
    catalog: &StaticCatalog,
    settings: &Settings,
    summoner_id: i64,
) -> ImportReport {
    let champion_name = catalog.champion_name(pick.champion_id);
    let build = match lookup {
        BuildLookup::Found(build) => build,
        BuildLookup::Missing | BuildLookup::Unavailable(_) => {
            let outcome = match lookup {
                BuildLookup::Unavailable(reason) => StepOutcome::Failed(reason.clone()),
                _ => StepOutcome::Skipped(format!("No build for {champion_name} yet")),
            };
            return ImportReport {
                champion_name,
                role: pick.role_label().to_owned(),
                runes: outcome.clone(),
                item_set: outcome.clone(),
                summoner_spells: outcome,
                source: None,
                rune_pages: Vec::new(),
                selected_rune_page: None,
            };
        }
    };

    let page = build.rune_pages.get(rune_page);
    let runes = match (settings.import_runes, page) {
        (false, _) => StepOutcome::Skipped(TURNED_OFF.into()),
        (true, None) => StepOutcome::Skipped("This build has no rune page".into()),
        (true, Some(page)) => {
            import_runes(
                port,
                page,
                catalog,
                &champion_name,
                settings.replace_current_page_when_full,
            )
            .await
        }
    };
    let item_set = if settings.import_item_sets {
        import_item_set(port, build, catalog, summoner_id).await
    } else {
        StepOutcome::Skipped(TURNED_OFF.into())
    };
    let summoner_spells = if settings.import_summoner_spells {
        import_spells(port, build, catalog, settings.flash_key).await
    } else {
        StepOutcome::Skipped(TURNED_OFF.into())
    };
    ImportReport {
        champion_name,
        // A source may resolve "no assigned role" to the champion's main role.
        role: match pick.mode {
            GameMode::Aram => pick.role_label().to_owned(),
            GameMode::SummonersRift => build.role.label().to_owned(),
        },
        runes,
        item_set,
        summoner_spells,
        source: Some(build.source.clone()),
        rune_pages: build.rune_pages.clone(),
        selected_rune_page: page.map(|_| rune_page),
    }
}

/// Sends one of the build's rune pages, chosen by the player. Runs even
/// when automatic rune import is turned off, because the player asked.
pub async fn apply_rune_page(
    port: &dyn LcuPort,
    page: &RunePageOption,
    catalog: &StaticCatalog,
    champion_name: &str,
    settings: &Settings,
) -> StepOutcome {
    import_runes(
        port,
        page,
        catalog,
        champion_name,
        settings.replace_current_page_when_full,
    )
    .await
}

async fn import_runes(
    port: &dyn LcuPort,
    option: &RunePageOption,
    catalog: &StaticCatalog,
    champion: &str,
    replace_current: bool,
) -> StepOutcome {
    if let Err(e) = runes::validate(&option.runes, catalog) {
        return StepOutcome::Skipped(format!(
            "Build is out of date for patch {}: {e}",
            catalog.version
        ));
    }
    let pages = match port.rune_pages().await {
        Ok(p) => p,
        Err(e) => return StepOutcome::Failed(format!("Could not read rune pages: {e}")),
    };
    let owned = match port.owned_rune_page_count().await {
        Ok(n) => n,
        Err(e) => return StepOutcome::Failed(format!("Could not read rune page slots: {e}")),
    };
    match runes::plan_slot(&pages, owned, replace_current) {
        SlotPlan::NoFreeSlot => {
            return StepOutcome::Skipped(
                "All rune page slots are in use. Allow replacing the current page in Settings."
                    .into(),
            )
        }
        SlotPlan::Replace(id) => {
            if let Err(e) = port.delete_rune_page(id).await {
                return StepOutcome::Failed(format!("Could not free a rune page slot: {e}"));
            }
        }
        SlotPlan::Create => {}
    }
    let page = NewRunePage::from_selection(champion, &option.runes);
    match port.create_rune_page(&page).await {
        Ok(()) => StepOutcome::Done(format!("Rune page \u{201c}{}\u{201d} is active", page.name)),
        Err(e) => {
            if e.to_string().contains("Max pages") {
                if let Some(to_replace) = pages
                    .iter()
                    .find(|p| p.is_editable && p.is_deletable && p.current)
                    .or_else(|| pages.iter().find(|p| p.is_editable && p.is_deletable))
                {
                    tracing::warn!(
                        id = to_replace.id,
                        name = %to_replace.name,
                        "Max pages reached; deleting page to retry creation"
                    );
                    if port.delete_rune_page(to_replace.id).await.is_ok() {
                        if let Ok(()) = port.create_rune_page(&page).await {
                            return StepOutcome::Done(format!(
                                "Rune page \u{201c}{}\u{201d} is active",
                                page.name
                            ));
                        }
                    }
                }
            }
            StepOutcome::Failed(format!("The client rejected the rune page: {e}"))
        }
    }
}

async fn import_item_set(
    port: &dyn LcuPort,
    build: &Build,
    catalog: &StaticCatalog,
    summoner_id: i64,
) -> StepOutcome {
    if summoner_id == 0 {
        return StepOutcome::Failed("Your summoner profile has not loaded yet".into());
    }
    let (set, dropped) = item_set_for(build, catalog);
    let count: usize = set.blocks.iter().map(|(_, items)| items.len()).sum();
    if count == 0 {
        return StepOutcome::Skipped(format!(
            "None of this build's items exist in patch {}",
            catalog.version
        ));
    }
    match port.upsert_item_set(summoner_id, &set).await {
        Ok(()) if dropped.is_empty() => {
            StepOutcome::Done(format!("{count} items added to the shop's recommended tab"))
        }
        Ok(()) => StepOutcome::Done(format!(
            "{count} items added; {} removed this patch were left out",
            dropped.len()
        )),
        Err(e) => StepOutcome::Failed(format!("Could not save the item set: {e}")),
    }
}

async fn import_spells(
    port: &dyn LcuPort,
    build: &Build,
    catalog: &StaticCatalog,
    flash_key: FlashKey,
) -> StepOutcome {
    let Some(spells) = build.summoner_spells else {
        return StepOutcome::Skipped("This build has no summoner spell data".into());
    };
    let name = |id: i64| catalog.summoner_spells.get(&id).cloned();
    let [first, second] = arrange(spells, flash_key);
    let (Some(d), Some(f)) = (name(first), name(second)) else {
        return StepOutcome::Skipped(format!(
            "Build uses a summoner spell missing from patch {}",
            catalog.version
        ));
    };
    match port.set_summoner_spells([first, second]).await {
        Ok(()) => StepOutcome::Done(format!("{d} on D, {f} on F")),
        Err(e) => StepOutcome::Failed(format!("Could not set summoner spells: {e}")),
    }
}
