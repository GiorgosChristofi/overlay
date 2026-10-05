//! Recommended builds. In Phase 1 they come from a curated, versioned seed
//! file; in v2 from the aggregation pipeline. Both produce a [`BuildCatalog`].

use crate::champ_select::Role;
use crate::runes::RuneSelection;
use serde::Deserialize;
use thiserror::Error;

pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Build {
    pub champion_id: i64,
    pub role: Role,
    pub runes: RuneSelection,
    pub summoner_spells: [i64; 2],
    pub item_blocks: Vec<ItemBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemBlock {
    pub title: String,
    pub items: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
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
        assert_eq!(c.select(99, Role::Support).unwrap().summoner_spells, [4, 3]);
    }

    #[test]
    fn falls_back_to_any_build_for_champion() {
        let c = BuildCatalog::from_json(RAW).unwrap();
        assert_eq!(c.select(99, Role::Top).unwrap().role, Role::Middle);
        assert!(c.select(1, Role::Top).is_none());
    }

    #[test]
    fn rejects_unknown_schema() {
        let err = BuildCatalog::from_json(r#"{"schemaVersion": 7, "builds": []}"#).unwrap_err();
        assert!(matches!(err, BuildCatalogError::Schema { found: 7, .. }));
    }
}
