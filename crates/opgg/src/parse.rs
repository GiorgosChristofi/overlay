//! Turns an OP.GG champion response into a [`Build`]. Pure, so it is tested
//! against recorded payloads without a network.
//!
//! Only the fields we use are modelled, and each defaults when missing or
//! null, so OP.GG adding, removing or emptying a field degrades one part of
//! the build instead of failing the whole import.

use domain::builds::{BlockKind, Build, ItemBlock, PickStats, RunePageOption};
use domain::champ_select::Role;
use domain::runes::RuneSelection;
use serde::{Deserialize, Deserializer};

/// How many rune pages the player can choose from.
pub const MAX_RUNE_PAGES: usize = 5;
/// How many late-game items go into the "Situational" block.
const MAX_SITUATIONAL_ITEMS: usize = 6;

/// OP.GG sends `null` for lists it has no data for (e.g. `positions` in
/// ARAM); treat that like a missing field.
fn nullable<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Response {
    #[serde(deserialize_with = "nullable")]
    data: Data,
    #[serde(deserialize_with = "nullable")]
    meta: Meta,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Meta {
    #[serde(deserialize_with = "nullable")]
    version: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Data {
    #[serde(deserialize_with = "nullable")]
    summary: Summary,
    #[serde(deserialize_with = "nullable")]
    summoner_spells: Vec<Entry>,
    #[serde(deserialize_with = "nullable")]
    starter_items: Vec<Entry>,
    #[serde(deserialize_with = "nullable")]
    core_items: Vec<Entry>,
    #[serde(deserialize_with = "nullable")]
    boots: Vec<Entry>,
    #[serde(deserialize_with = "nullable")]
    last_items: Vec<Entry>,
    #[serde(deserialize_with = "nullable")]
    rune_pages: Vec<RunePageGroup>,
    #[serde(deserialize_with = "nullable")]
    runes: Vec<RuneBuild>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Summary {
    /// Most played first.
    #[serde(deserialize_with = "nullable")]
    positions: Vec<Position>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Position {
    #[serde(deserialize_with = "nullable")]
    name: String,
}

/// A list of ids (items or spells) with how often it was picked.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Entry {
    #[serde(deserialize_with = "nullable")]
    ids: Vec<i64>,
    #[serde(deserialize_with = "nullable")]
    play: u32,
    #[serde(deserialize_with = "nullable")]
    win: u32,
    #[serde(deserialize_with = "nullable")]
    pick_rate: f64,
}

/// Rune pages grouped by keystone and secondary tree, most picked first.
#[derive(Deserialize, Default)]
#[serde(default)]
struct RunePageGroup {
    #[serde(deserialize_with = "nullable")]
    pick_rate: f64,
    #[serde(deserialize_with = "nullable")]
    builds: Vec<RuneBuild>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RuneBuild {
    #[serde(deserialize_with = "nullable")]
    primary_page_id: i64,
    #[serde(deserialize_with = "nullable")]
    primary_rune_ids: Vec<i64>,
    #[serde(deserialize_with = "nullable")]
    secondary_page_id: i64,
    #[serde(deserialize_with = "nullable")]
    secondary_rune_ids: Vec<i64>,
    #[serde(deserialize_with = "nullable")]
    stat_mod_ids: Vec<i64>,
    #[serde(deserialize_with = "nullable")]
    play: u32,
    #[serde(deserialize_with = "nullable")]
    win: u32,
    #[serde(deserialize_with = "nullable")]
    pick_rate: f64,
}

impl RuneBuild {
    fn option(&self, pick_rate: f64) -> RunePageOption {
        let perk_ids = self
            .primary_rune_ids
            .iter()
            .chain(&self.secondary_rune_ids)
            .chain(&self.stat_mod_ids)
            .copied()
            .collect();
        RunePageOption {
            runes: RuneSelection {
                primary_style_id: self.primary_page_id,
                sub_style_id: self.secondary_page_id,
                perk_ids,
            },
            stats: Some(PickStats {
                games: self.play,
                wins: self.win,
                pick_rate,
            }),
        }
    }
}

/// What a response says, beyond the build itself.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    /// `None` when OP.GG has no rune data for this champion and mode.
    pub build: Option<Build>,
    /// The champion's most played role, from the response summary.
    pub most_played: Option<Role>,
}

/// OP.GG position names, as used in its URLs.
pub fn position(role: Role) -> Option<&'static str> {
    match role {
        Role::Top => Some("TOP"),
        Role::Jungle => Some("JUNGLE"),
        Role::Middle => Some("MID"),
        Role::Bottom => Some("ADC"),
        Role::Support => Some("SUPPORT"),
        Role::Any => None,
    }
}

pub fn role_from_position(name: &str) -> Role {
    match name {
        "TOP" => Role::Top,
        "JUNGLE" => Role::Jungle,
        "MID" => Role::Middle,
        "ADC" => Role::Bottom,
        "SUPPORT" => Role::Support,
        _ => Role::Any,
    }
}

/// Parses one champion response.
///
/// `source` is a label like `OP.GG EUNE · Emerald+`; the patch is appended.
pub fn parse(
    raw: &str,
    champion_id: i64,
    role: Role,
    source: &str,
) -> Result<Parsed, serde_json::Error> {
    let Response { data, meta } = serde_json::from_str(raw)?;
    let most_played = data
        .summary
        .positions
        .first()
        .map(|p| role_from_position(&p.name))
        .filter(|r| *r != Role::Any);

    let rune_pages = rune_pages(&data);
    if rune_pages.is_empty() {
        return Ok(Parsed {
            build: None,
            most_played,
        });
    }
    let summoner_spells = data
        .summoner_spells
        .iter()
        .find_map(|e| <[i64; 2]>::try_from(e.ids.as_slice()).ok());
    let source = if meta.version.is_empty() {
        source.to_owned()
    } else {
        format!("{source} · patch {}", meta.version)
    };
    Ok(Parsed {
        build: Some(Build {
            champion_id,
            role,
            rune_pages,
            summoner_spells,
            item_blocks: item_blocks(&data),
            source,
        }),
        most_played,
    })
}

/// The most common page of each keystone/secondary-tree group. Falls back to
/// the flat `runes` list when OP.GG sends no groups.
fn rune_pages(data: &Data) -> Vec<RunePageOption> {
    let grouped = data.rune_pages.iter().filter_map(|group| {
        group
            .builds
            .first()
            .map(|b| b.option(group.pick_rate * b.pick_rate))
    });
    let flat = data.runes.iter().map(|b| b.option(b.pick_rate));
    let candidates: Vec<RunePageOption> = if data.rune_pages.is_empty() {
        flat.collect()
    } else {
        grouped.collect()
    };

    let mut pages: Vec<RunePageOption> = Vec::with_capacity(MAX_RUNE_PAGES);
    for page in candidates {
        if pages.len() == MAX_RUNE_PAGES {
            break;
        }
        if page.runes.perk_ids.len() == 9 && !pages.iter().any(|p| p.runes == page.runes) {
            pages.push(page);
        }
    }
    pages
}

fn win_rate_suffix(e: &Entry) -> String {
    if e.play == 0 {
        return String::new();
    }
    let rate = f64::from(e.win) * 100.0 / f64::from(e.play);
    format!(" · {rate:.0}% win rate")
}

fn item_blocks(data: &Data) -> Vec<ItemBlock> {
    let mut blocks = Vec::new();
    let mut used: Vec<i64> = Vec::new();
    for (title, kind, entries) in [
        ("Starting items", BlockKind::Starter, &data.starter_items),
        ("Core build", BlockKind::Core, &data.core_items),
        ("Boots", BlockKind::Boots, &data.boots),
    ] {
        if let Some(best) = entries.iter().find(|e| !e.ids.is_empty()) {
            blocks.push(ItemBlock {
                title: format!("{title}{}", win_rate_suffix(best)),
                items: best.ids.clone(),
                kind,
                pick_rates: Vec::new(),
            });
            used.extend(&best.ids);
        }
    }

    // Most built items after the core, with how often each one is built.
    let mut situational: Vec<i64> = Vec::new();
    let mut pick_rates: Vec<f64> = Vec::new();
    for entry in &data.last_items {
        for id in &entry.ids {
            if situational.len() < MAX_SITUATIONAL_ITEMS
                && !used.contains(id)
                && !situational.contains(id)
            {
                situational.push(*id);
                pick_rates.push(entry.pick_rate);
            }
        }
    }
    if !situational.is_empty() {
        blocks.push(ItemBlock {
            title: "Situational".into(),
            items: situational,
            kind: BlockKind::Situational,
            pick_rates,
        });
    }
    blocks
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    const LUX_MID: &str = include_str!("../tests/fixtures/lux-mid.json");

    fn lux() -> Parsed {
        parse(LUX_MID, 99, Role::Middle, "OP.GG EUNE · Emerald+").unwrap()
    }

    #[test]
    fn recorded_response_becomes_a_full_build() {
        let p = lux();
        assert_eq!(p.most_played, Some(Role::Support));
        let b = p.build.unwrap();
        assert_eq!(b.champion_id, 99);
        assert_eq!(b.role, Role::Middle);
        assert_eq!(b.source, "OP.GG EUNE · Emerald+ · patch 16.19");
        assert_eq!(b.summoner_spells, Some([4, 12]));
    }

    #[test]
    fn offers_the_top_page_of_each_rune_group_best_first() {
        let b = lux().build.unwrap();
        assert_eq!(b.rune_pages.len(), MAX_RUNE_PAGES);
        let first = &b.rune_pages[0];
        assert_eq!(
            first.runes,
            RuneSelection {
                primary_style_id: 8200,
                sub_style_id: 8100,
                perk_ids: vec![8229, 8226, 8210, 8237, 8106, 8126, 5008, 5008, 5001],
            }
        );
        let stats = first.stats.unwrap();
        assert_eq!((stats.games, stats.wins), (4492, 2323));
        // Group pick rate (61.24%) times the page's share of its group (78.39%).
        assert!((stats.pick_rate - 0.6124 * 0.7839).abs() < 1e-9);
        let keystones: Vec<i64> = b.rune_pages.iter().map(|p| p.runes.perk_ids[0]).collect();
        assert_eq!(keystones[..2], [8229, 8128]);
    }

    #[test]
    fn item_blocks_cover_start_core_boots_and_situational() {
        let b = lux().build.unwrap();
        let titles: Vec<&str> = b
            .item_blocks
            .iter()
            .map(|blk| blk.title.split(" · ").next().unwrap())
            .collect();
        assert_eq!(
            titles,
            ["Starting items", "Core build", "Boots", "Situational"]
        );
        assert_eq!(b.item_blocks[0].items, [1056, 2003, 2003]);
        assert_eq!(b.item_blocks[1].items, [6655, 4646, 4645]);
        assert_eq!(b.item_blocks[1].title, "Core build · 54% win rate");
        assert_eq!(b.item_blocks[2].items, [3020]);
        let kinds: Vec<BlockKind> = b.item_blocks.iter().map(|blk| blk.kind).collect();
        assert_eq!(
            kinds,
            [
                BlockKind::Starter,
                BlockKind::Core,
                BlockKind::Boots,
                BlockKind::Situational
            ]
        );
        // Lux's most built items after the core: Malignance (35%), then Rylai's (31%).
        assert_eq!(b.item_blocks[3].items[..2], [3118, 3089]);
        assert_eq!(b.item_blocks[3].pick_rates[..2], [0.3547, 0.3119]);
        let situational = &b.item_blocks[3].items;
        assert_eq!(situational.len(), MAX_SITUATIONAL_ITEMS);
        assert!(
            !situational
                .iter()
                .any(|i| [6655, 4646, 4645, 3020].contains(i)),
            "situational repeats core items: {situational:?}"
        );
    }

    #[test]
    fn falls_back_to_flat_rune_list() {
        let raw = r#"{"data":{"runes":[
            {"primary_page_id":8000,"primary_rune_ids":[8010,9111,9104,8299],"secondary_page_id":8400,"secondary_rune_ids":[8444,8242],"stat_mod_ids":[5005,5008,5011],"play":10,"win":6,"pick_rate":0.4}
        ]}}"#;
        let b = parse(raw, 86, Role::Top, "OP.GG").unwrap().build.unwrap();
        assert_eq!(b.rune_pages.len(), 1);
        assert_eq!(b.rune_pages[0].stats.unwrap().pick_rate, 0.4);
        assert_eq!(b.summoner_spells, None);
        assert!(b.item_blocks.is_empty());
        assert_eq!(b.source, "OP.GG");
    }

    #[test]
    fn incomplete_and_duplicate_pages_are_skipped() {
        let raw = r#"{"data":{"runes":[
            {"primary_page_id":8000,"primary_rune_ids":[8010],"secondary_page_id":8400,"secondary_rune_ids":[],"stat_mod_ids":[]},
            {"primary_page_id":8000,"primary_rune_ids":[8010,9111,9104,8299],"secondary_page_id":8400,"secondary_rune_ids":[8444,8242],"stat_mod_ids":[5005,5008,5011]},
            {"primary_page_id":8000,"primary_rune_ids":[8010,9111,9104,8299],"secondary_page_id":8400,"secondary_rune_ids":[8444,8242],"stat_mod_ids":[5005,5008,5011]}
        ]}}"#;
        let b = parse(raw, 86, Role::Top, "OP.GG").unwrap().build.unwrap();
        assert_eq!(b.rune_pages.len(), 1);
    }

    #[test]
    fn null_fields_count_as_missing() {
        let raw = r#"{"data":{"summary":{"positions":null},"boots":null,"summoner_spells":[{"ids":null},{"ids":[4,32],"play":null}],"runes":[
            {"primary_page_id":8000,"primary_rune_ids":[8010,9111,9104,8299],"secondary_page_id":8400,"secondary_rune_ids":[8444,8242],"stat_mod_ids":[5005,5008,5011],"play":null,"win":null,"pick_rate":null}
        ]},"meta":null}"#;
        let p = parse(raw, 86, Role::Any, "OP.GG").unwrap();
        assert_eq!(p.most_played, None);
        let b = p.build.unwrap();
        assert_eq!(b.summoner_spells, Some([4, 32]));
        assert_eq!(b.rune_pages[0].stats.unwrap().games, 0);
    }

    #[test]
    fn no_rune_data_means_no_build() {
        let p = parse(r#"{"data":{}}"#, 1, Role::Top, "OP.GG").unwrap();
        assert_eq!(p.build, None);
        assert!(parse("not json", 1, Role::Top, "OP.GG").is_err());
    }

    #[test]
    fn positions_round_trip() {
        for role in [
            Role::Top,
            Role::Jungle,
            Role::Middle,
            Role::Bottom,
            Role::Support,
        ] {
            assert_eq!(role_from_position(position(role).unwrap()), role);
        }
        assert_eq!(position(Role::Any), None);
    }
}
