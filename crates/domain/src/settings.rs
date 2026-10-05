use crate::spells::FlashKey;
use serde::{Deserialize, Serialize};

/// User settings. Every field has a default so older files keep loading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub import_runes: bool,
    pub import_item_sets: bool,
    pub import_summoner_spells: bool,
    pub flash_key: FlashKey,
    /// When every rune page slot is used, replace the current page.
    pub replace_current_page_when_full: bool,
    pub start_with_windows: bool,
    pub start_minimized: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            import_runes: true,
            import_item_sets: true,
            import_summoner_spells: true,
            flash_key: FlashKey::D,
            replace_current_page_when_full: false,
            start_with_windows: false,
            start_minimized: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"flashKey":"F"}"#).unwrap();
        assert_eq!(s.flash_key, FlashKey::F);
        assert!(s.import_runes);
    }
}
