//! Patch-specific static data the domain needs, built by the `static-data`
//! crate from Data Dragon.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuneTree {
    pub id: i64,
    pub name: String,
    /// Icon path relative to [`StaticCatalog::rune_image_base_url`].
    pub icon: String,
    /// Row 0 holds keystones; rows 1..=3 the regular runes.
    pub slots: Vec<Vec<i64>>,
}

/// Display data for one rune or stat shard.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuneInfo {
    pub name: String,
    /// Either a path relative to [`StaticCatalog::rune_image_base_url`]
    /// (Data Dragon runes) or an absolute `https://` URL (stat shards, which
    /// Data Dragon does not publish).
    pub icon: String,
    /// What the rune does, as plain text; paragraphs separated by blank lines.
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StaticCatalog {
    /// Data Dragon version, e.g. `16.19.1`.
    pub version: String,
    /// Base URL for this patch's images, e.g. `https://ddragon.leagueoflegends.com/cdn/16.19.1/img`.
    pub image_base_url: String,
    /// Base URL for rune images, which Data Dragon does not version,
    /// e.g. `https://ddragon.leagueoflegends.com/cdn/img`.
    pub rune_image_base_url: String,
    /// Champion numeric key -> display name.
    pub champions: HashMap<i64, String>,
    /// Summoner spell numeric key -> display name.
    pub summoner_spells: HashMap<i64, String>,
    /// Item id -> display name.
    pub items: HashMap<i64, String>,
    pub rune_trees: Vec<RuneTree>,
    /// Rune and stat shard id -> name and icon.
    pub runes: HashMap<i64, RuneInfo>,
    /// Stat shard choices per row. Data Dragon does not publish these.
    pub stat_shard_rows: Vec<Vec<i64>>,
}

impl StaticCatalog {
    pub fn rune_tree(&self, id: i64) -> Option<&RuneTree> {
        self.rune_trees.iter().find(|t| t.id == id)
    }

    pub fn profile_icon_url(&self, icon_id: i64) -> Option<String> {
        (!self.image_base_url.is_empty())
            .then(|| format!("{}/profileicon/{icon_id}.png", self.image_base_url))
    }

    /// Full URL for a rune or tree icon path, `None` when it cannot be resolved.
    pub fn rune_icon_url(&self, icon: &str) -> Option<String> {
        if icon.starts_with("https://") {
            return Some(icon.to_owned());
        }
        (!icon.is_empty() && !self.rune_image_base_url.is_empty())
            .then(|| format!("{}/{icon}", self.rune_image_base_url))
    }

    pub fn champion_name(&self, id: i64) -> String {
        self.champions
            .get(&id)
            .cloned()
            .unwrap_or_else(|| format!("Champion {id}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rune_icons_resolve_relative_and_absolute_paths() {
        let c = StaticCatalog {
            rune_image_base_url: "https://cdn.test/img".into(),
            ..StaticCatalog::default()
        };
        assert_eq!(
            c.rune_icon_url("perk-images/Styles/7200_Domination.png")
                .as_deref(),
            Some("https://cdn.test/img/perk-images/Styles/7200_Domination.png")
        );
        assert_eq!(
            c.rune_icon_url("https://other.test/a.png").as_deref(),
            Some("https://other.test/a.png")
        );
        assert_eq!(c.rune_icon_url(""), None);
    }
}
