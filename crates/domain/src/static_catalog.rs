//! Patch-specific static data the domain needs, built by the `static-data`
//! crate from Data Dragon.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuneTree {
    pub id: i64,
    pub name: String,
    /// Row 0 holds keystones; rows 1..=3 the regular runes.
    pub slots: Vec<Vec<i64>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StaticCatalog {
    /// Data Dragon version, e.g. `16.19.1`.
    pub version: String,
    /// Base URL for this patch's images, e.g. `https://ddragon.leagueoflegends.com/cdn/16.19.1/img`.
    pub image_base_url: String,
    /// Champion numeric key -> display name.
    pub champions: HashMap<i64, String>,
    /// Summoner spell numeric key -> display name.
    pub summoner_spells: HashMap<i64, String>,
    /// Item id -> display name.
    pub items: HashMap<i64, String>,
    pub rune_trees: Vec<RuneTree>,
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

    pub fn champion_name(&self, id: i64) -> String {
        self.champions
            .get(&id)
            .cloned()
            .unwrap_or_else(|| format!("Champion {id}"))
    }
}
