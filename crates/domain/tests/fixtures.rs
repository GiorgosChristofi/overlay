#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code may fail loudly

//! Contract tests: every champ select payload in `fixtures/` must parse, and
//! the expected lock must be found. New recordings are picked up automatically.

use domain::champ_select::{selected_pick, ChampSelectSession, ChampionPick, GameMode, Role};
use std::path::PathBuf;

fn fixture_files(prefix: &str) -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut files = Vec::new();
    for dir in std::fs::read_dir(&root)
        .unwrap()
        .flatten()
        .filter(|d| d.path().is_dir())
    {
        for f in std::fs::read_dir(dir.path()).unwrap().flatten() {
            let name = f.file_name().to_string_lossy().into_owned();
            if name.starts_with(prefix) && name.ends_with(".json") {
                files.push(f.path());
            }
        }
    }
    files.sort();
    files
}

#[test]
fn every_champ_select_fixture_parses() {
    let files = fixture_files("champ-select-session");
    assert!(!files.is_empty(), "no fixtures found");
    for path in files {
        let raw = std::fs::read_to_string(&path).unwrap();
        let parsed: Result<ChampSelectSession, _> = serde_json::from_str(&raw);
        assert!(
            parsed.is_ok(),
            "{} failed to parse: {:?}",
            path.display(),
            parsed.err()
        );
    }
}

fn load(name: &str) -> ChampSelectSession {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/synthetic")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn draft_fixture_yields_locked_mid_lux() {
    let s = load("champ-select-session-draft-locked.json");
    assert_eq!(
        selected_pick(&s),
        Some(ChampionPick {
            champion_id: 99,
            role: Role::Middle,
            mode: GameMode::SummonersRift,
        })
    );
}

#[test]
fn aram_fixture_yields_assigned_champion() {
    let s = load("champ-select-session-aram.json");
    assert_eq!(
        selected_pick(&s),
        Some(ChampionPick {
            champion_id: 222,
            role: Role::Any,
            mode: GameMode::Aram,
        })
    );
}
