#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code may fail loudly

//! Data Dragon sync against a fake CDN serving trimmed fixtures.

use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use static_data::DataDragon;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const VERSION: &str = "16.19.1";

fn fixture(name: &str) -> Option<String> {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .ok()
}

/// Starts a fake CDN; returns its URL and a counter of data-file downloads.
async fn fake_cdn() -> (String, Arc<AtomicUsize>) {
    let downloads = Arc::new(AtomicUsize::new(0));
    let counter = downloads.clone();
    let app = Router::new()
        .route(
            "/api/versions.json",
            get(|| async { format!(r#"["{VERSION}","16.18.1"]"#) }),
        )
        .route(
            "/cdn/{version}/data/{locale}/{file}",
            get(
                move |Path((version, _locale, file)): Path<(String, String, String)>| {
                    let counter = counter.clone();
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        match (version == VERSION).then(|| fixture(&file)).flatten() {
                            Some(body) => (StatusCode::OK, body),
                            None => (StatusCode::NOT_FOUND, String::new()),
                        }
                    }
                },
            ),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, downloads)
}

#[tokio::test]
async fn downloads_latest_patch_and_parses_it() {
    let (url, _) = fake_cdn().await;
    let cache = tempfile::tempdir().unwrap();
    let catalog = DataDragon::new(url.clone(), cache.path(), "en_US")
        .unwrap()
        .load()
        .await
        .unwrap();

    assert_eq!(catalog.version, VERSION);
    assert_eq!(catalog.champion_name(99), "Lux");
    assert_eq!(
        catalog.summoner_spells.get(&4).map(String::as_str),
        Some("Flash")
    );
    assert_eq!(
        catalog.summoner_spells.len(),
        3,
        "non-numeric keys are skipped"
    );
    assert_eq!(
        catalog.items.get(&1055).map(String::as_str),
        Some("Doran's Blade")
    );
    assert_eq!(
        catalog.rune_tree(8200).unwrap().slots[0],
        vec![8214, 8229, 8230]
    );
    assert_eq!(catalog.stat_shard_rows.len(), 3);
    assert_eq!(catalog.rune_image_base_url, format!("{url}/cdn/img"));
    for shard in catalog.stat_shard_rows.iter().flatten() {
        let info = &catalog.runes[shard];
        assert!(!info.name.is_empty(), "shard {shard} has no name");
        assert!(
            !info.description.is_empty(),
            "shard {shard} has no description"
        );
        assert!(
            catalog
                .rune_icon_url(&info.icon)
                .is_some_and(|u| u.starts_with("https://")),
            "shard {shard} has no icon"
        );
    }
}

#[tokio::test]
async fn second_load_uses_the_cache() {
    let (url, downloads) = fake_cdn().await;
    let cache = tempfile::tempdir().unwrap();
    let dd = DataDragon::new(url, cache.path(), "en_US").unwrap();
    dd.load().await.unwrap();
    dd.load().await.unwrap();
    assert_eq!(
        downloads.load(Ordering::SeqCst),
        4,
        "each file is downloaded once"
    );
}

#[tokio::test]
async fn works_offline_from_cache() {
    let (url, _) = fake_cdn().await;
    let cache = tempfile::tempdir().unwrap();
    DataDragon::new(url, cache.path(), "en_US")
        .unwrap()
        .load()
        .await
        .unwrap();

    let offline = DataDragon::new("http://127.0.0.1:1", cache.path(), "en_US").unwrap();
    assert_eq!(offline.load().await.unwrap().version, VERSION);
}

#[tokio::test]
async fn offline_with_empty_cache_is_a_clear_error() {
    let cache = tempfile::tempdir().unwrap();
    let offline = DataDragon::new("http://127.0.0.1:1", cache.path(), "en_US").unwrap();
    assert!(matches!(
        offline.load().await,
        Err(static_data::StaticDataError::NothingAvailable)
    ));
}
