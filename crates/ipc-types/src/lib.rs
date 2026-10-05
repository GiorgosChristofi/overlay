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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportReportDto {
    pub champion_name: String,
    pub role: String,
    pub runes: StepResultDto,
    pub item_set: StepResultDto,
    pub summoner_spells: StepResultDto,
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
        }
    }
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
    fn session_state_is_camel_case() {
        assert_eq!(
            serde_json::to_string(&SessionStateDto::ChampSelect).unwrap(),
            "\"champSelect\""
        );
    }
}
