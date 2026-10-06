#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code may fail loudly

//! The shipped seed builds must be valid against the fixture patch, and
//! (ignored by default, run nightly in CI) against live Data Dragon.

use domain::builds::BuildCatalog;
use domain::runes::validate;
use static_data::{parse_catalog, DataDragon, RawFiles};

fn seed() -> BuildCatalog {
    BuildCatalog::from_json(include_str!("../../../data/builds/seed.json")).unwrap()
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn check(catalog: &domain::static_catalog::StaticCatalog) {
    for b in &seed().builds {
        let name = catalog.champion_name(b.champion_id);
        assert!(
            catalog.champions.contains_key(&b.champion_id),
            "unknown champion {}",
            b.champion_id
        );
        for page in &b.rune_pages {
            validate(&page.runes, catalog).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        for spell in b.summoner_spells.into_iter().flatten() {
            assert!(
                catalog.summoner_spells.contains_key(&spell),
                "{name}: unknown spell {spell}"
            );
        }
        for item in b.item_blocks.iter().flat_map(|blk| &blk.items) {
            assert!(
                catalog.items.contains_key(item),
                "{name}: unknown item {item}"
            );
        }
    }
}

#[test]
fn seed_builds_match_fixture_patch() {
    let (c, s, r, i) = (
        fixture("champion.json"),
        fixture("summoner.json"),
        fixture("runesReforged.json"),
        fixture("item.json"),
    );
    let catalog = parse_catalog(
        "fixture",
        &RawFiles {
            champions: &c,
            summoners: &s,
            runes: &r,
            items: &i,
        },
    )
    .unwrap();
    check(&catalog);
}

#[tokio::test]
#[ignore = "needs network; run with --ignored (nightly CI job)"]
async fn seed_builds_match_live_patch() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = DataDragon::new(static_data::DATA_DRAGON_URL, dir.path(), "en_US")
        .unwrap()
        .load()
        .await
        .unwrap();
    check(&catalog);
}
