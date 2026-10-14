//! DTOs sent between the Rust core and the React UI.
//!
//! Running `cargo test -p ipc-types` regenerates `ui/src/bindings/*.ts`.
//! CI fails if the committed bindings differ, so a Rust change that would
//! break the UI is caught at build time.
//!
//! Numbers are `u32`/`i32` on purpose: ts-rs maps 64-bit integers to
//! `bigint`, which JSON does not produce.

use domain::settings::Settings;
use domain::spells::FlashKey;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Tauri event names. Mirrored in `ui/src/lib/backend.ts`.
pub const EVENT_STATUS: &str = "core://status";
pub const EVENT_IMPORT: &str = "core://import";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SessionStateDto {
    Disconnected,
    ClientReady,
    Lobby,
    ChampSelect,
    InGame,
    PostGame,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SummonerDto {
    pub riot_id: String,
    pub level: u32,
    pub profile_icon_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StatusDto {
    pub session: SessionStateDto,
    pub summoner: Option<SummonerDto>,
    /// Data Dragon patch in use, `None` while loading.
    pub patch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "camelCase")]
#[ts(export)]
pub enum StepResultDto {
    Done { detail: String },
    Skipped { reason: String },
    Failed { error: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportReportDto {
    pub champion_name: String,
    pub role: String,
    pub runes: StepResultDto,
    pub item_set: StepResultDto,
    pub summoner_spells: StepResultDto,
    /// Where the build came from, e.g. `OP.GG all regions · Emerald+ · patch 16.19`.
    pub source: Option<String>,
    /// Rune pages the player can switch between, best first.
    pub rune_pages: Vec<RunePageDto>,
    /// Index into `rune_pages` of the page last sent to the client.
    pub selected_rune_page: Option<u32>,
}

/// One choosable rune page. Perk order: keystone, 3 primary, 2 secondary, 3 shards.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunePageDto {
    pub primary_style_id: i32,
    pub sub_style_id: i32,
    pub perk_ids: Vec<i32>,
    /// Statistics, when the source has them.
    pub games: Option<u32>,
    /// `0.0..=1.0`.
    pub win_rate: Option<f32>,
    /// `0.0..=1.0`.
    pub pick_rate: Option<f32>,
}

/// The current patch's rune trees and stat shards, for drawing pages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuneCatalogDto {
    pub trees: Vec<RuneTreeDto>,
    pub stat_shard_rows: Vec<Vec<RuneDto>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuneTreeDto {
    pub id: i32,
    pub name: String,
    pub icon_url: Option<String>,
    /// Row 0 holds keystones; rows 1..=3 the regular runes.
    pub slots: Vec<Vec<RuneDto>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuneDto {
    pub id: i32,
    pub name: String,
    pub icon_url: Option<String>,
    /// What the rune does, plain text; paragraphs separated by blank lines.
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum FlashKeyDto {
    D,
    F,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SettingsDto {
    pub import_runes: bool,
    pub import_item_sets: bool,
    pub import_summoner_spells: bool,
    pub flash_key: FlashKeyDto,
    pub replace_current_page_when_full: bool,
    pub start_with_windows: bool,
    pub start_minimized: bool,
    pub overlay_enabled: bool,
}

/// Every command error the UI can receive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppErrorDto {
    /// Stable machine-readable code, e.g. `not_connected`.
    pub code: String,
    /// Sentence shown to the player.
    pub message: String,
}

impl From<FlashKey> for FlashKeyDto {
    fn from(k: FlashKey) -> Self {
        match k {
            FlashKey::D => Self::D,
            FlashKey::F => Self::F,
        }
    }
}

impl From<FlashKeyDto> for FlashKey {
    fn from(k: FlashKeyDto) -> Self {
        match k {
            FlashKeyDto::D => Self::D,
            FlashKeyDto::F => Self::F,
        }
    }
}

impl From<Settings> for SettingsDto {
    fn from(s: Settings) -> Self {
        Self {
            import_runes: s.import_runes,
            import_item_sets: s.import_item_sets,
            import_summoner_spells: s.import_summoner_spells,
            flash_key: s.flash_key.into(),
            replace_current_page_when_full: s.replace_current_page_when_full,
            start_with_windows: s.start_with_windows,
            start_minimized: s.start_minimized,
            overlay_enabled: s.overlay_enabled,
        }
    }
}

impl From<SettingsDto> for Settings {
    fn from(s: SettingsDto) -> Self {
        Self {
            import_runes: s.import_runes,
            import_item_sets: s.import_item_sets,
            import_summoner_spells: s.import_summoner_spells,
            flash_key: s.flash_key.into(),
            replace_current_page_when_full: s.replace_current_page_when_full,
            start_with_windows: s.start_with_windows,
            start_minimized: s.start_minimized,
            overlay_enabled: s.overlay_enabled,
        }
    }
}

// ---------- in-game overlay ----------
//
// The desktop app sends the overlay process (`desktop --overlay`) one JSON
// [`OverlayMessage`] per stdin line; the overlay answers with
// [`OverlayCommand`] lines on its stdout. Rust on both ends, so no
// TypeScript is generated.

/// A message to the overlay process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "view", rename_all = "camelCase")]
pub enum OverlayMessage {
    /// What to show; replaces the previous view.
    Items(Box<OverlayViewDto>),
    /// No build (yet): show the empty state.
    Clear,
    /// Epic objective timers for the minimap, about once a second in a
    /// Summoner's Rift game; `None` when there is no such game.
    Objectives(Option<ObjectiveTimersDto>),
}

/// The next spawn in each pit, against the game clock.
///
/// Times are game time, not wall time: the overlay counts down from
/// `game_time_ms` itself between messages, so timers stay on the game's
/// clock even though it is only read once a second.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveTimersDto {
    /// The game clock when it was read.
    pub game_time_ms: u64,
    /// One per pit at most; a pit whose monster is up has none.
    pub timers: Vec<ObjectiveTimerDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveTimerDto {
    pub pit: PitDto,
    /// What spawns, e.g. `Dragon`, `Elder`, `Baron`.
    pub label: String,
    /// Game time it spawns.
    pub spawn_at_ms: u64,
}

/// Where a timer is drawn on the minimap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PitDto {
    /// Bottom river.
    Dragon,
    /// Top river: Voidgrubs, Herald, Baron.
    Baron,
    /// The River Scuttler on the Baron side (first spawn only).
    ScuttleTop,
    /// The River Scuttler on the Dragon side (first spawn only).
    ScuttleBottom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayViewDto {
    pub champion_name: String,
    pub role: String,
    pub source: String,
    /// Current gold, `None` outside a running game.
    pub gold: Option<u32>,
    /// Starting items not bought yet. While present, they are what to buy
    /// first; the list below leaves them out.
    pub starting: Option<StartingItemsDto>,
    /// The next item to build (never boots), broken into its recipe.
    pub next: Option<RecipeNodeDto>,
    /// The recommended boots until bought, broken into their recipe.
    pub boots: Option<RecipeNodeDto>,
    /// `next` was chosen by the player, not picked automatically.
    pub next_pinned: bool,
    /// Items the player removed from the list this game.
    pub removed_count: u32,
    /// Recommended items not bought yet; starting items first while relevant.
    pub blocks: Vec<OverlayBlockDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OverlayBlockKind {
    Starter,
    Core,
    Boots,
    Situational,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayBlockDto {
    pub kind: OverlayBlockKind,
    pub title: String,
    pub items: Vec<OverlayItemDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayItemDto {
    pub id: i32,
    pub name: String,
    pub icon_url: Option<String>,
    /// Full shop price.
    pub cost: u32,
    /// How many the build recommends (e.g. 2 potions).
    pub count: u32,
    /// Share of games that built it, `0.0..=1.0`, when known.
    pub pick_rate: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartingItemsDto {
    pub items: Vec<OverlayItemDto>,
    /// Price of everything listed, counts included.
    pub cost: u32,
    /// The player has the gold for all of them now.
    pub affordable: bool,
}

/// What the player did in the overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum OverlayCommand {
    /// Build this item next instead of the automatic choice.
    SetNext { item_id: i32 },
    /// Back to the automatic next item.
    ClearNext,
    /// Take an item off the list for this game.
    Remove { item_id: i32 },
    /// Put every removed item back.
    RestoreRemoved,
}

/// One item of a recipe tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeNodeDto {
    pub item: OverlayItemDto,
    /// Held in the inventory.
    pub owned: bool,
    /// Gold still needed for it, counting owned components.
    pub remaining_cost: u32,
    /// Not owned and the player has the gold for it now.
    pub affordable: bool,
    pub components: Vec<RecipeNodeDto>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let s = Settings {
            flash_key: FlashKey::F,
            start_minimized: true,
            ..Settings::default()
        };
        assert_eq!(Settings::from(SettingsDto::from(s.clone())), s);
    }

    #[test]
    fn step_result_json_shape_matches_ui_expectation() {
        let json = serde_json::to_value(StepResultDto::Skipped {
            reason: "off".into(),
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "status": "skipped", "reason": "off" })
        );
    }

    #[test]
    fn overlay_messages_are_tagged_json_lines() {
        assert_eq!(
            serde_json::to_string(&OverlayMessage::Clear).unwrap(),
            r#"{"type":"clear"}"#
        );
        let view = OverlayViewDto {
            champion_name: "Lux".into(),
            role: "Middle".into(),
            source: "OP.GG".into(),
            gold: Some(500),
            starting: None,
            next: None,
            boots: None,
            next_pinned: false,
            removed_count: 0,
            blocks: vec![],
        };
        let timers = OverlayMessage::Objectives(Some(ObjectiveTimersDto {
            game_time_ms: 61_000,
            timers: vec![ObjectiveTimerDto {
                pit: PitDto::Dragon,
                label: "Dragon".into(),
                spawn_at_ms: 300_000,
            }],
        }));
        let timers_line = serde_json::to_string(&timers).unwrap();
        assert_eq!(
            timers_line,
            r#"{"type":"objectives","view":{"gameTimeMs":61000,"timers":[{"pit":"dragon","label":"Dragon","spawnAtMs":300000}]}}"#
        );
        assert_eq!(
            serde_json::from_str::<OverlayMessage>(&timers_line).unwrap(),
            timers
        );
        assert_eq!(
            serde_json::to_string(&OverlayMessage::Objectives(None)).unwrap(),
            r#"{"type":"objectives","view":null}"#
        );
        let line = serde_json::to_string(&OverlayMessage::Items(Box::new(view.clone()))).unwrap();
        assert!(!line.contains('\n'));
        assert_eq!(
            serde_json::from_str::<OverlayMessage>(&line).unwrap(),
            OverlayMessage::Items(Box::new(view))
        );
    }

    #[test]
    fn overlay_commands_are_tagged_camel_case() {
        let line = serde_json::to_string(&OverlayCommand::SetNext { item_id: 6655 }).unwrap();
        assert_eq!(line, r#"{"type":"setNext","itemId":6655}"#);
        assert_eq!(
            serde_json::from_str::<OverlayCommand>(r#"{"type":"restoreRemoved"}"#).unwrap(),
            OverlayCommand::RestoreRemoved
        );
    }

    #[test]
    fn session_state_is_camel_case() {
        assert_eq!(
            serde_json::to_string(&SessionStateDto::ChampSelect).unwrap(),
            "\"champSelect\""
        );
    }
}
