#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code may fail loudly

//! `OpggClient` against a fake OP.GG serving a recorded response.

use axum::http::{StatusCode, Uri};
use axum::routing::get;
use axum::Router;
use domain::champ_select::{ChampionPick, GameMode, Role};
use domain::ports::BuildSource;
use domain::PortError;
use opgg::OpggClient;
use std::sync::{Arc, Mutex};

const LUX_MID: &str = include_str!("fixtures/lux-mid.json");

/// Serves the Lux fixture for champion 99, a 500 for champion 500 and a 404
/// for anything else. Returns the base URL and the requested paths.
async fn fake_opgg() -> (String, Arc<Mutex<Vec<String>>>) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let log = requests.clone();
    let app = Router::new().fallback(get(move |uri: Uri| {
        let log = log.clone();
        async move {
            let path = uri.path_and_query().unwrap().to_string();
            log.lock().unwrap().push(path.clone());
            if path.contains("/champions/ranked/99/") || path.contains("/champions/aram/99/") {
                (StatusCode::OK, LUX_MID.to_string())
            } else if path.contains("/500/") {
                (StatusCode::INTERNAL_SERVER_ERROR, "down".to_string())
            } else {
                (StatusCode::NOT_FOUND, String::new())
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, requests)
}

fn pick(champion_id: i64, role: Role, mode: GameMode) -> ChampionPick {
    ChampionPick {
        champion_id,
        role,
        mode,
    }
}

#[tokio::test]
async fn asks_for_the_assigned_role_in_the_region() {
    let (url, requests) = fake_opgg().await;
    let client = OpggClient::new(url, "EUNE").unwrap();
    let build = client
        .build(pick(99, Role::Middle, GameMode::SummonersRift))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(build.role, Role::Middle);
    assert_eq!(build.source, "OP.GG EUNE · Emerald+ · patch 16.19");
    assert_eq!(
        *requests.lock().unwrap(),
        ["/api/EUNE/champions/ranked/99/MID?tier=emerald_plus"]
    );
}

#[tokio::test]
async fn without_a_role_uses_the_champions_main_role() {
    let (url, requests) = fake_opgg().await;
    let client = OpggClient::new(url, "EUNE").unwrap();
    let build = client
        .build(pick(99, Role::Any, GameMode::SummonersRift))
        .await
        .unwrap()
        .unwrap();
    // The fixture says Lux is played most as support.
    assert_eq!(build.role, Role::Support);
    assert_eq!(
        *requests.lock().unwrap(),
        [
            "/api/EUNE/champions/ranked/99/MID?tier=emerald_plus",
            "/api/EUNE/champions/ranked/99/SUPPORT?tier=emerald_plus"
        ]
    );
}

#[tokio::test]
async fn aram_uses_aram_statistics() {
    let (url, requests) = fake_opgg().await;
    let client = OpggClient::new(url, "EUNE").unwrap();
    let build = client
        .build(pick(99, Role::Any, GameMode::Aram))
        .await
        .unwrap()
        .unwrap();
    assert!(build.source.contains("ARAM"), "{}", build.source);
    assert_eq!(
        *requests.lock().unwrap(),
        ["/api/EUNE/champions/aram/99/none"]
    );
}

#[tokio::test]
async fn unknown_champion_is_no_build_not_an_error() {
    let (url, _) = fake_opgg().await;
    let client = OpggClient::new(url, "EUNE").unwrap();
    let result = client
        .build(pick(1, Role::Top, GameMode::SummonersRift))
        .await;
    assert_eq!(result, Ok(None));
}

#[tokio::test]
async fn server_errors_are_reported() {
    let (url, _) = fake_opgg().await;
    let client = OpggClient::new(url, "EUNE").unwrap();
    let err = client
        .build(pick(500, Role::Top, GameMode::SummonersRift))
        .await
        .unwrap_err();
    assert!(
        matches!(err, PortError::Http { status: 500, .. }),
        "{err:?}"
    );
}

#[tokio::test]
async fn global_region_combines_every_server() {
    let (url, requests) = fake_opgg().await;
    let client = OpggClient::new(url, opgg::GLOBAL_REGION).unwrap();
    let build = client
        .build(pick(99, Role::Middle, GameMode::SummonersRift))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(build.source, "OP.GG all regions · Emerald+ · patch 16.19");
    assert_eq!(
        *requests.lock().unwrap(),
        ["/api/global/champions/ranked/99/MID?tier=emerald_plus"]
    );
}

/// Checks the real OP.GG still answers in the shape we parse.
/// `cargo test -p opgg -- --ignored` (network).
#[tokio::test]
#[ignore = "network: live OP.GG"]
async fn live_opgg_still_parses() {
    let client = OpggClient::new(opgg::OPGG_API_URL, opgg::GLOBAL_REGION).unwrap();
    for p in [
        pick(99, Role::Middle, GameMode::SummonersRift),
        pick(64, Role::Jungle, GameMode::SummonersRift),
        pick(222, Role::Any, GameMode::SummonersRift),
        pick(86, Role::Any, GameMode::Aram),
    ] {
        let b = client.build(p).await.unwrap().expect("a build");
        assert!(!b.rune_pages.is_empty(), "{p:?}: no rune pages");
        assert!(b.rune_pages.iter().all(|r| r.runes.perk_ids.len() == 9));
        assert!(b.summoner_spells.is_some(), "{p:?}: no spells");
        assert!(b.item_blocks.len() >= 3, "{p:?}: {:?}", b.item_blocks);
        eprintln!(
            "{p:?} -> {:?} {} pages, spells {:?}, {}",
            b.role,
            b.rune_pages.len(),
            b.summoner_spells,
            b.source
        );
    }
}

#[tokio::test]
async fn repeated_picks_are_served_from_cache() {
    let (url, requests) = fake_opgg().await;
    let client = OpggClient::new(url, "EUNE").unwrap();
    let p = pick(99, Role::Middle, GameMode::SummonersRift);
    let first = client.build(p).await.unwrap();
    let second = client.build(p).await.unwrap();
    assert_eq!(first, second);
    assert_eq!(requests.lock().unwrap().len(), 1);
}
