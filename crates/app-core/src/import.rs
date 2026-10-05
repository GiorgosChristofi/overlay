//! The champ-select import: rune page, item set and summoner spells.
//! Each step runs independently, so one failure never blocks the others.

use domain::builds::Build;
use domain::champ_select::LockedPick;
use domain::item_set::item_set_for;
use domain::ports::LcuPort;
use domain::runes::{self, NewRunePage, SlotPlan};
use domain::settings::Settings;
use domain::spells::{arrange, FlashKey};
use domain::static_catalog::StaticCatalog;
use ipc_types::{ImportReportDto, StepResultDto};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    Done(String),
    Skipped(String),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReport {
    pub champion_name: String,
    pub role: String,
    pub runes: StepOutcome,
    pub item_set: StepOutcome,
    pub summoner_spells: StepOutcome,
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

impl From<ImportReport> for ImportReportDto {
    fn from(r: ImportReport) -> Self {
        Self {
            champion_name: r.champion_name,
            role: r.role,
            runes: r.runes.into(),
            item_set: r.item_set.into(),
            summoner_spells: r.summoner_spells.into(),
        }
    }
}

const TURNED_OFF: &str = "Turned off in settings";

/// Runs every enabled step for a locked pick.
pub async fn run_import(
    port: &dyn LcuPort,
    pick: LockedPick,
    build: Option<&Build>,
    catalog: &StaticCatalog,
    settings: &Settings,
    summoner_id: i64,
) -> ImportReport {
    let champion_name = catalog.champion_name(pick.champion_id);
    let role = pick.role.label().to_owned();
    let Some(build) = build else {
        let none = StepOutcome::Skipped(format!("No build for {champion_name} yet"));
        return ImportReport {
            champion_name,
            role,
            runes: none.clone(),
            item_set: none.clone(),
            summoner_spells: none,
        };
    };

    let runes = if settings.import_runes {
        import_runes(
            port,
            build,
            catalog,
            &champion_name,
            settings.replace_current_page_when_full,
        )
        .await
    } else {
        StepOutcome::Skipped(TURNED_OFF.into())
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
        role,
        runes,
        item_set,
        summoner_spells,
    }
}

async fn import_runes(
    port: &dyn LcuPort,
    build: &Build,
    catalog: &StaticCatalog,
    champion: &str,
    replace_current: bool,
) -> StepOutcome {
    if let Err(e) = runes::validate(&build.runes, catalog) {
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
    let page = NewRunePage::from_selection(champion, &build.runes);
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
    let name = |id: i64| catalog.summoner_spells.get(&id).cloned();
    let [first, second] = arrange(build.summoner_spells, flash_key);
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
