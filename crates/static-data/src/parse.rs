use crate::StaticDataError;
use domain::static_catalog::{ItemInfo, RuneInfo, RuneTree, StaticCatalog};
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

const STAT_SHARD_ICON_BASE: &str =
    "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/perk-images/statmods";

/// Names, icons (CommunityDragon) and effects of the shards in [`STAT_SHARD_ROWS`].
const STAT_SHARDS: [(i64, &str, &str, &str); 7] = [
    (
        5008,
        "Adaptive Force",
        "statmodsadaptiveforceicon.png",
        "+9 Adaptive Force (5.4 bonus AD or 9 AP)",
    ),
    (
        5005,
        "Attack Speed",
        "statmodsattackspeedicon.png",
        "+10% Attack Speed",
    ),
    (
        5007,
        "Ability Haste",
        "statmodscdrscalingicon.png",
        "+8 Ability Haste",
    ),
    (
        5010,
        "Move Speed",
        "statmodsmovementspeedicon.png",
        "+2.5% Move Speed",
    ),
    (
        5001,
        "Health Scaling",
        "statmodshealthplusicon.png",
        "+10-180 Health (based on level)",
    ),
    (
        5011,
        "Health",
        "statmodshealthscalingicon.png",
        "+65 Health",
    ),
    (
        5013,
        "Tenacity and Slow Resist",
        "statmodstenacityicon.png",
        "+15% Tenacity and Slow Resist",
    ),
];

/// Turns Data Dragon's tooltip markup into plain text: `<br>`, `<hr>` and
/// `<li>` become line breaks, every other tag is dropped (keeping its text).
fn plain_text(markup: &str) -> String {
    let mut text = String::with_capacity(markup.len());
    let mut rest = markup;
    while let Some(start) = rest.find('<') {
        text.push_str(&rest[..start]);
        let Some(len) = rest[start..].find('>') else {
            // A stray '<' is text, not a tag.
            text.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let tag = rest[start + 1..start + len]
            .trim_start_matches('/')
            .trim()
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(tag.as_str(), "br" | "hr" | "li") {
            text.push('\n');
        }
        rest = &rest[start + len + 1..];
    }
    text.push_str(rest);
    let text = text
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");

    // Tidy whitespace: single spaces within lines, at most one blank line.
    let mut out = String::with_capacity(text.len());
    let mut blank_run = 0;
    for line in text.lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() {
            blank_run += 1;
            continue;
        }
        if !out.is_empty() {
            out.push_str(if blank_run > 0 { "\n\n" } else { "\n" });
        }
        out.push_str(&line);
        blank_run = 0;
    }
    out
}

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
    /// Component ids as strings, e.g. `["3802", "3145"]`.
    #[serde(default)]
    from: Vec<String>,
    #[serde(default)]
    gold: Gold,
}

#[derive(Deserialize, Default)]
struct Gold {
    #[serde(default)]
    total: u32,
}

impl ItemEntry {
    fn info(self) -> ItemInfo {
        ItemInfo {
            name: self.name,
            total_gold: self.gold.total,
            from: self.from.iter().filter_map(|id| id.parse().ok()).collect(),
        }
    }
}

#[derive(Deserialize)]
struct Tree {
    id: i64,
    name: String,
    #[serde(default)]
    icon: String,
    slots: Vec<Slot>,
}

#[derive(Deserialize)]
struct Slot {
    runes: Vec<Rune>,
}

#[derive(Deserialize)]
struct Rune {
    id: i64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    icon: String,
    #[serde(default, rename = "longDesc")]
    long_desc: String,
    #[serde(default, rename = "shortDesc")]
    short_desc: String,
}

impl Rune {
    /// The full tooltip text, as the client shows it; the short one if missing.
    fn description(&self) -> String {
        let markup = if self.long_desc.is_empty() {
            &self.short_desc
        } else {
            &self.long_desc
        };
        plain_text(markup)
    }
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
    let mut runes: HashMap<i64, RuneInfo> = trees
        .iter()
        .flat_map(|t| t.slots.iter().flat_map(|s| &s.runes))
        .map(|r| {
            let info = RuneInfo {
                name: r.name.clone(),
                icon: r.icon.clone(),
                description: r.description(),
            };
            (r.id, info)
        })
        .collect();
    runes.extend(STAT_SHARDS.iter().map(|(id, name, file, effect)| {
        let info = RuneInfo {
            name: (*name).to_owned(),
            icon: format!("{STAT_SHARD_ICON_BASE}/{file}"),
            description: (*effect).to_owned(),
        };
        (*id, info)
    }));
    Ok(StaticCatalog {
        version: version.to_owned(),
        image_base_url: String::new(),
        rune_image_base_url: String::new(),
        runes,
        champions: keyed_names(raw.champions, "champion.json")?,
        summoner_spells: keyed_names(raw.summoners, "summoner.json")?,
        items: items
            .data
            .into_iter()
            .filter_map(|(id, e)| id.parse().ok().map(|id| (id, e.info())))
            .collect(),
        rune_trees: trees
            .into_iter()
            .map(|t| RuneTree {
                id: t.id,
                name: t.name,
                icon: t.icon,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_markup_becomes_plain_paragraphs() {
        let raw = "Damaging a Champion deals <lol-uikit-tooltipped-keyword key='X'><font color='#48C4B7'>adaptive damage</font></lol-uikit-tooltipped-keyword>.<br><br>Damage: 30 <scaleAD>(+0.1 bonus AD)</scaleAD><br>Cooldown: 35s<br><rules>Only champions.</rules>";
        assert_eq!(
            plain_text(raw),
            "Damaging a Champion deals adaptive damage.\n\nDamage: 30 (+0.1 bonus AD)\nCooldown: 35s\nOnly champions."
        );
    }

    #[test]
    fn items_carry_their_recipe_and_price() {
        let raw = r#"{"name": "Luden's Echo", "from": ["3802", "3145", "bad"], "gold": {"base": 450, "total": 2750}}"#;
        let info = serde_json::from_str::<ItemEntry>(raw).unwrap().info();
        assert_eq!(info.name, "Luden's Echo");
        assert_eq!(info.total_gold, 2750);
        assert_eq!(info.from, [3802, 3145]);
        let basic = serde_json::from_str::<ItemEntry>(r#"{"name": "Tome"}"#)
            .unwrap()
            .info();
        assert!(basic.from.is_empty());
    }

    #[test]
    fn plain_text_handles_entities_stray_brackets_and_extra_breaks() {
        assert_eq!(
            plain_text("a &amp; b<br/><br /><br>  c &lt; 3 <i>x</i>"),
            "a & b\n\nc < 3 x"
        );
        assert_eq!(plain_text("5 < 6"), "5 < 6");
        assert_eq!(plain_text(""), "");
    }
}
