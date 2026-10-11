//! Recommended builds. They come from a [`BuildSource`](crate::ports::BuildSource):
//! live statistics (OP.GG) first, then the curated seed file shipped with
//! the app as an offline fallback.

use crate::champ_select::Role;
use crate::runes::{self, RuneError, RuneSelection};
use crate::static_catalog::StaticCatalog;
use serde::Deserialize;
use thiserror::Error;

pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Where the shipped seed builds say they come from.
pub const SEED_SOURCE: &str = "Built-in build";

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(from = "SeedBuild")]
pub struct Build {
    pub champion_id: i64,
    /// The role these statistics are for. A source may resolve
    /// [`Role::Any`] to the champion's most played role.
    pub role: Role,
    /// Rune pages, best first. The first one is applied automatically.
    pub rune_pages: Vec<RunePageOption>,
    /// `None` when the source has no spell data.
    pub summoner_spells: Option<[i64; 2]>,
    pub item_blocks: Vec<ItemBlock>,
    /// Shown to the player, e.g. `OP.GG EUNE · Emerald+ · patch 16.19`.
    pub source: String,
}

/// One rune page the player can choose, with how it performs when known.
#[derive(Debug, Clone, PartialEq)]
pub struct RunePageOption {
    pub runes: RuneSelection,
    pub stats: Option<PickStats>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickStats {
    pub games: u32,
    pub wins: u32,
    /// Share of all games on this champion and role, `0.0..=1.0`.
    pub pick_rate: f64,
}

impl PickStats {
    pub fn win_rate(&self) -> Option<f64> {
        (self.games > 0).then(|| f64::from(self.wins) / f64::from(self.games))
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemBlock {
    pub title: String,
    pub items: Vec<i64>,
    #[serde(default)]
    pub kind: BlockKind,
    /// Share of games that built each item, parallel to `items`; empty when
    /// the source has no per-item statistics.
    #[serde(default)]
    pub pick_rates: Vec<f64>,
}

/// What a block is for. The in-game column treats starting items as an
/// early-game section and picks the next item to build from the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlockKind {
    Starter,
    Core,
    Boots,
    Situational,
    #[default]
    #[serde(other)]
    Other,
}

/// The seed file's shape: one rune page per build.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeedBuild {
    champion_id: i64,
    role: Role,
    runes: RuneSelection,
    summoner_spells: [i64; 2],
    item_blocks: Vec<ItemBlock>,
}

impl From<SeedBuild> for Build {
    fn from(b: SeedBuild) -> Self {
        Self {
            champion_id: b.champion_id,
            role: b.role,
            rune_pages: vec![RunePageOption {
                runes: b.runes,
                stats: None,
            }],
            summoner_spells: Some(b.summoner_spells),
            item_blocks: b.item_blocks,
            source: SEED_SOURCE.into(),
        }
    }
}

impl Build {
    /// Drops rune pages that are invalid this patch, keeping the order.
    /// Returns the first error so it can be shown when nothing is left.
    pub fn retain_valid_rune_pages(&mut self, catalog: &StaticCatalog) -> Option<RuneError> {
        let mut first_error = None;
        self.rune_pages
            .retain(|page| match runes::validate(&page.runes, catalog) {
                Ok(()) => true,
                Err(e) => {
                    first_error.get_or_insert(e);
                    false
                }
            });
        first_error
    }
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildCatalog {
    pub schema_version: u32,
    pub builds: Vec<Build>,
}

#[derive(Debug, Error)]
pub enum BuildCatalogError {
    #[error("build file is not valid JSON: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("build file schema {found} is not supported (expected {expected})")]
    Schema { found: u32, expected: u32 },
}

impl BuildCatalog {
    pub fn from_json(raw: &str) -> Result<Self, BuildCatalogError> {
        let catalog: Self = serde_json::from_str(raw)?;
        if catalog.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(BuildCatalogError::Schema {
                found: catalog.schema_version,
                expected: SUPPORTED_SCHEMA_VERSION,
            });
        }
        Ok(catalog)
    }

    /// Best build for a champion: exact role, then a role-agnostic build,
    /// then any build for that champion.
    pub fn select(&self, champion_id: i64, role: Role) -> Option<&Build> {
        let for_champ = || {
            self.builds
                .iter()
                .filter(move |b| b.champion_id == champion_id)
        };
        for_champ()
            .find(|b| b.role == role)
            .or_else(|| for_champ().find(|b| b.role == Role::Any))
            .or_else(|| for_champ().next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::static_catalog::RuneTree;

    const RAW: &str = r#"{
      "schemaVersion": 1,
      "builds": [
        { "championId": 99, "role": "middle", "runes": { "primaryStyleId": 8200, "subStyleId": 8000, "perkIds": [1,2,3,4,5,6,7,8,9] }, "summonerSpells": [4, 14], "itemBlocks": [] },
        { "championId": 99, "role": "support", "runes": { "primaryStyleId": 8200, "subStyleId": 8000, "perkIds": [1,2,3,4,5,6,7,8,9] }, "summonerSpells": [4, 3], "itemBlocks": [] }
      ]
    }"#;

    #[test]
    fn selects_exact_role() {
        let c = BuildCatalog::from_json(RAW).unwrap();
        assert_eq!(
            c.select(99, Role::Support).unwrap().summoner_spells,
            Some([4, 3])
        );
    }

    #[test]
    fn falls_back_to_any_build_for_champion() {
        let c = BuildCatalog::from_json(RAW).unwrap();
        assert_eq!(c.select(99, Role::Top).unwrap().role, Role::Middle);
        assert!(c.select(1, Role::Top).is_none());
    }

    #[test]
    fn seed_builds_have_one_rune_page_and_say_where_they_come_from() {
        let c = BuildCatalog::from_json(RAW).unwrap();
        let b = c.select(99, Role::Middle).unwrap();
        assert_eq!(b.rune_pages.len(), 1);
        assert_eq!(b.rune_pages[0].stats, None);
        assert_eq!(b.source, SEED_SOURCE);
    }

    #[test]
    fn rejects_unknown_schema() {
        let err = BuildCatalog::from_json(r#"{"schemaVersion": 7, "builds": []}"#).unwrap_err();
        assert!(matches!(err, BuildCatalogError::Schema { found: 7, .. }));
    }

    #[test]
    fn win_rate_needs_games() {
        let s = PickStats {
            games: 4,
            wins: 3,
            pick_rate: 0.5,
        };
        assert_eq!(s.win_rate(), Some(0.75));
        assert_eq!(PickStats { games: 0, ..s }.win_rate(), None);
    }

    #[test]
    fn invalid_rune_pages_are_dropped_in_order() {
        let catalog = StaticCatalog {
            rune_trees: vec![
                RuneTree {
                    id: 1,
                    slots: vec![vec![10], vec![11], vec![12], vec![13]],
                    ..RuneTree::default()
                },
                RuneTree {
                    id: 2,
                    slots: vec![vec![20], vec![21], vec![22], vec![23]],
                    ..RuneTree::default()
                },
            ],
            stat_shard_rows: vec![vec![5008], vec![5008], vec![5001]],
            ..StaticCatalog::default()
        };
        let page = |keystone: i64| RunePageOption {
            runes: RuneSelection {
                primary_style_id: 1,
                sub_style_id: 2,
                perk_ids: vec![keystone, 11, 12, 13, 21, 22, 5008, 5008, 5001],
            },
            stats: None,
        };
        let mut b = Build {
            champion_id: 1,
            role: Role::Any,
            rune_pages: vec![page(99), page(10), page(98)],
            summoner_spells: None,
            item_blocks: vec![],
            source: String::new(),
        };
        let err = b.retain_valid_rune_pages(&catalog);
        assert_eq!(b.rune_pages, vec![page(10)]);
        assert!(matches!(
            err,
            Some(RuneError::InvalidPerk {
                position: 0,
                perk: 99
            })
        ));
    }
}
