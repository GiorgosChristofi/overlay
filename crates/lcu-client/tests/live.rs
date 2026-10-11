#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code may fail loudly

//! `LiveClient` against a fake game: not running, loading, in game.

use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use domain::ports::LiveGamePort;
use lcu_client::LiveClient;

const FIXTURE: &str = include_str!("../../../fixtures/synthetic/live-allgamedata.json");

async fn serve(status: StatusCode, body: &'static str) -> String {
    let app = Router::new().route(
        "/liveclientdata/allgamedata",
        get(move || async move { (status, body) }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

#[tokio::test]
async fn no_game_running_is_no_snapshot() {
    // Bind and drop a listener to get a port nothing listens on.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let client = LiveClient::new(format!("http://127.0.0.1:{port}"), None).unwrap();
    assert_eq!(client.snapshot().await, Ok(None));
}

#[tokio::test]
async fn loading_screen_is_no_snapshot() {
    let url = serve(
        StatusCode::NOT_FOUND,
        r#"{"errorCode":"RESOURCE_NOT_FOUND"}"#,
    )
    .await;
    let client = LiveClient::new(url, None).unwrap();
    assert_eq!(client.snapshot().await, Ok(None));
}

#[tokio::test]
async fn in_game_reports_gold_and_items() {
    let url = serve(StatusCode::OK, FIXTURE).await;
    let client = LiveClient::new(url, None).unwrap();
    let s = client.snapshot().await.unwrap().unwrap();
    assert_eq!(s.gold, 1337);
    assert_eq!(s.inventory.len(), 3);
}

#[test]
fn https_needs_the_riot_certificate() {
    assert!(LiveClient::new(lcu_client::LIVE_CLIENT_URL, None).is_err());
}
