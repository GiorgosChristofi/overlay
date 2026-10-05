use crate::StaticDataError;
use domain::static_catalog::{RuneTree, StaticCatalog};
use serde::Deserialize;
use std::collections::HashMap;

/// Stat shard rows. Data Dragon does not publish shards, so they live here.
/// Update when Riot changes shards; `domain::runes::validate` rejects builds
/// that use a shard missing from these rows.
pub const STAT_SHARD_ROWS: [[i64; 3]; 3] = [
    [5008, 5005, 5007], // adaptive force, attack speed, ability haste
    [5008, 5010, 5001], // adaptive force, move speed, health scaling
    [5011, 5013, 5001], // health, tenacity and slow resist, health scaling
];

#[derive(Debug, Clone, Copy)]
pub struct RawFiles<'a> {
    pub champions: &'a str,
    pub summoners: &'a str,
    pub runes: &'a str,
    pub items: &'a str,
}

#[derive(Deserialize)]
struct Keyed {
    data: HashMap<String, KeyedEntry>,
}

#[derive(Deserialize)]
struct KeyedEntry {
    key: String,
    name: String,
}

#[derive(Deserialize)]
struct Items {
    data: HashMap<String, ItemEntry>,
}

#[derive(Deserialize)]
struct ItemEntry {
    name: String,
}

#[derive(Deserialize)]
struct Tree {
    id: i64,
    name: String,
    slots: Vec<Slot>,
}

#[derive(Deserialize)]
struct Slot {
    runes: Vec<Rune>,
}

#[derive(Deserialize)]
struct Rune {
    id: i64,
}

fn invalid(file: &str, e: impl std::fmt::Display) -> StaticDataError {
    StaticDataError::Invalid {
        file: file.into(),
        reason: e.to_string(),
    }
}

/// `{"data": {"Aatrox": {"key": "266", "name": "Aatrox"}}}` -> `{266: "Aatrox"}`.
/// Entries with a non-numeric key are skipped rather than failing the patch.
fn keyed_names(raw: &str, file: &str) -> Result<HashMap<i64, String>, StaticDataError> {
    let parsed: Keyed = serde_json::from_str(raw).map_err(|e| invalid(file, e))?;
    Ok(parsed
        .data
        .into_values()
        .filter_map(|e| e.key.parse().ok().map(|k| (k, e.name)))
        .collect())
}

pub fn parse_catalog(version: &str, raw: &RawFiles<'_>) -> Result<StaticCatalog, StaticDataError> {
    let items: Items = serde_json::from_str(raw.items).map_err(|e| invalid("item.json", e))?;
    let trees: Vec<Tree> =
        serde_json::from_str(raw.runes).map_err(|e| invalid("runesReforged.json", e))?;
    Ok(StaticCatalog {
        version: version.to_owned(),
        image_base_url: String::new(),
        champions: keyed_names(raw.champions, "champion.json")?,
        summoner_spells: keyed_names(raw.summoners, "summoner.json")?,
        items: items
            .data
            .into_iter()
            .filter_map(|(id, e)| id.parse().ok().map(|id| (id, e.name)))
            .collect(),
        rune_trees: trees
            .into_iter()
            .map(|t| RuneTree {
                id: t.id,
                name: t.name,
                slots: t
                    .slots
                    .into_iter()
                    .map(|s| s.runes.into_iter().map(|r| r.id).collect())
                    .collect(),
            })
            .collect(),
        stat_shard_rows: STAT_SHARD_ROWS.iter().map(|row| row.to_vec()).collect(),
    })
}
