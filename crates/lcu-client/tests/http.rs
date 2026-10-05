#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code may fail loudly

mod support;

use domain::item_set::ItemSet;
use domain::ports::LcuPort;
use domain::runes::NewRunePage;
use domain::PortError;
use lcu_client::LcuHttp;
use serde_json::{json, Value};
use support::FakeLcu;

async fn client() -> (FakeLcu, LcuHttp) {
    let fake = FakeLcu::start().await;
    let http = LcuHttp::new(&fake.lockfile(), None).unwrap();
    (fake, http)
}

#[tokio::test]
async fn reads_current_summoner_tolerating_extra_fields() {
    let (fake, http) = client().await;
    fake.respond(
        "GET",
        "/lol-summoner/v1/current-summoner",
        200,
        r#"{"gameName":"Tester","tagLine":"EUW","summonerLevel":312,"profileIconId":29,"puuid":"p","summonerId":7,"unknownField":[1,2]}"#,
    );
    let s = http.current_summoner().await.unwrap();
    assert_eq!(s.riot_id(), "Tester#EUW");
    assert_eq!(s.summoner_level, 312);
}

#[tokio::test]
async fn missing_champ_select_session_is_none() {
    let (_fake, http) = client().await;
    assert_eq!(http.champ_select_session().await.unwrap(), None);
}

#[tokio::test]
async fn creates_rune_page_with_lcu_field_names() {
    let (fake, http) = client().await;
    let page = NewRunePage {
        name: "LoL Overlay: Lux".into(),
        primary_style_id: 8200,
        sub_style_id: 8000,
        selected_perk_ids: vec![8229, 8226, 8210, 8237, 8009, 8014, 5008, 5008, 5011],
        current: true,
    };
    http.create_rune_page(&page).await.unwrap();
    let req = fake.requests().pop().unwrap();
    assert_eq!(
        (req.method.as_str(), req.path.as_str()),
        ("POST", "/lol-perks/v1/pages")
    );
    let body: Value = serde_json::from_str(&req.body).unwrap();
    assert_eq!(body["primaryStyleId"], 8200);
    assert_eq!(body["selectedPerkIds"].as_array().unwrap().len(), 9);
    assert_eq!(body["current"], true);
}

#[tokio::test]
async fn http_errors_carry_status_and_body() {
    let (fake, http) = client().await;
    fake.respond(
        "POST",
        "/lol-perks/v1/pages",
        400,
        r#"{"message":"Max pages reached"}"#,
    );
    let page = NewRunePage {
        name: "x".into(),
        primary_style_id: 1,
        sub_style_id: 2,
        selected_perk_ids: vec![],
        current: true,
    };
    match http.create_rune_page(&page).await {
        Err(PortError::Http { status: 400, body }) => assert!(body.contains("Max pages")),
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn upserts_item_set_by_reading_then_writing_whole_document() {
    let (fake, http) = client().await;
    fake.respond(
        "GET",
        "/lol-item-sets/v1/item-sets/7/sets",
        200,
        r#"{"accountId":7,"itemSets":[{"uid":"mine","title":"Mine"}],"timestamp":1}"#,
    );
    let set = ItemSet {
        uid: "lol-overlay-99".into(),
        title: "LoL Overlay: Lux Middle".into(),
        champion_id: 99,
        blocks: vec![("Start".into(), vec![1056])],
    };
    http.upsert_item_set(7, &set).await.unwrap();

    let put = fake
        .requests()
        .into_iter()
        .find(|r| r.method == "PUT")
        .unwrap();
    let body: Value = serde_json::from_str(&put.body).unwrap();
    assert_eq!(body["itemSets"].as_array().unwrap().len(), 2);
    assert_eq!(body["itemSets"][1]["associatedChampions"], json!([99]));
}

#[tokio::test]
async fn sets_summoner_spells() {
    let (fake, http) = client().await;
    http.set_summoner_spells([4, 14]).await.unwrap();
    let req = fake.requests().pop().unwrap();
    assert_eq!(req.method, "PATCH");
    assert_eq!(req.path, "/lol-champ-select/v1/session/my-selection");
    assert_eq!(
        serde_json::from_str::<Value>(&req.body).unwrap(),
        json!({"spell1Id": 4, "spell2Id": 14})
    );
}

#[tokio::test]
async fn reports_not_connected_when_nothing_listens() {
    let info = lcu_client::LockfileInfo {
        pid: 1,
        port: 1,
        password: "x".into(),
        protocol: "http".into(),
    };
    let http = LcuHttp::new(&info, None).unwrap();
    assert_eq!(http.gameflow_phase().await, Err(PortError::NotConnected));
}

#[tokio::test]
async fn https_without_riot_root_is_refused() {
    let info = lcu_client::LockfileInfo {
        pid: 1,
        port: 1,
        password: "x".into(),
        protocol: "https".into(),
    };
    assert!(LcuHttp::new(&info, None).is_err());
}
