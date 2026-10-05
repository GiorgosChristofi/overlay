#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code may fail loudly

mod support;

use domain::phase::GameflowPhase;
use domain::ports::LcuEvent;
use lcu_client::wamp::{
    TOPIC_CHAMP_SELECT, TOPIC_GAMEFLOW_PHASE, URI_CHAMP_SELECT, URI_GAMEFLOW_PHASE,
};
use lcu_client::{run_connector, ConnectorConfig};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use support::FakeLcu;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

async fn next(rx: &mut mpsc::Receiver<LcuEvent>) -> LcuEvent {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out waiting for an LCU event")
        .expect("channel closed")
}

fn start(fake: &FakeLcu) -> (mpsc::Receiver<LcuEvent>, CancellationToken) {
    let info = fake.lockfile();
    let config = ConnectorConfig {
        riot_root_pem: None,
        retry_interval: Duration::from_millis(50),
        discover: Arc::new(move || Some(info.clone())),
    };
    let (tx, rx) = mpsc::channel(32);
    let cancel = CancellationToken::new();
    tokio::spawn(run_connector(config, tx, cancel.clone()));
    (rx, cancel)
}

#[tokio::test]
async fn connects_subscribes_and_forwards_events() {
    let fake = FakeLcu::start().await;
    fake.respond("GET", "/lol-gameflow/v1/gameflow-phase", 200, "\"Lobby\"");
    let (mut rx, cancel) = start(&fake);

    assert!(matches!(next(&mut rx).await, LcuEvent::Connected(_)));
    assert!(matches!(
        next(&mut rx).await,
        LcuEvent::Phase(GameflowPhase::Lobby)
    ));

    fake.wait_for_subscriptions(2).await;
    assert_eq!(
        fake.subscriptions(),
        vec![
            format!(r#"[5,"{TOPIC_GAMEFLOW_PHASE}"]"#),
            format!(r#"[5,"{TOPIC_CHAMP_SELECT}"]"#),
        ]
    );

    fake.push_event(
        TOPIC_GAMEFLOW_PHASE,
        URI_GAMEFLOW_PHASE,
        "Update",
        json!("ChampSelect"),
    );
    assert!(matches!(
        next(&mut rx).await,
        LcuEvent::Phase(GameflowPhase::ChampSelect)
    ));

    let session = json!({ "localPlayerCellId": 3, "myTeam": [{ "cellId": 3, "championId": 99 }], "actions": [], "someNewRiotField": true });
    fake.push_event(TOPIC_CHAMP_SELECT, URI_CHAMP_SELECT, "Update", session);
    match next(&mut rx).await {
        LcuEvent::ChampSelect(Some(s)) => assert_eq!(s.local_player_cell_id, 3),
        other => panic!("unexpected {other:?}"),
    }

    fake.push_event(TOPIC_CHAMP_SELECT, URI_CHAMP_SELECT, "Delete", json!(null));
    assert!(matches!(next(&mut rx).await, LcuEvent::ChampSelect(None)));
    cancel.cancel();
}

#[tokio::test]
async fn reports_disconnect_and_reconnects_after_client_restart() {
    let fake = FakeLcu::start().await;
    fake.respond("GET", "/lol-gameflow/v1/gameflow-phase", 200, "\"None\"");
    let (mut rx, cancel) = start(&fake);

    assert!(matches!(next(&mut rx).await, LcuEvent::Connected(_)));
    assert!(matches!(
        next(&mut rx).await,
        LcuEvent::Phase(GameflowPhase::None)
    ));
    fake.wait_for_subscriptions(2).await;

    fake.close_sockets();
    assert!(matches!(next(&mut rx).await, LcuEvent::Disconnected));
    assert!(matches!(next(&mut rx).await, LcuEvent::Connected(_)));
    cancel.cancel();
}

#[tokio::test]
async fn catches_up_when_started_mid_champ_select() {
    let fake = FakeLcu::start().await;
    fake.respond(
        "GET",
        "/lol-gameflow/v1/gameflow-phase",
        200,
        "\"ChampSelect\"",
    );
    fake.respond(
        "GET",
        "/lol-champ-select/v1/session",
        200,
        r#"{"localPlayerCellId":1,"myTeam":[],"actions":[]}"#,
    );
    let (mut rx, cancel) = start(&fake);

    assert!(matches!(next(&mut rx).await, LcuEvent::Connected(_)));
    assert!(matches!(
        next(&mut rx).await,
        LcuEvent::Phase(GameflowPhase::ChampSelect)
    ));
    assert!(matches!(
        next(&mut rx).await,
        LcuEvent::ChampSelect(Some(_))
    ));
    cancel.cancel();
}

#[tokio::test]
async fn keeps_retrying_while_client_is_not_ready() {
    let fake = FakeLcu::start().await;
    // No gameflow response yet: the client is still booting (404).
    let (mut rx, cancel) = start(&fake);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(rx.try_recv().is_err());

    fake.respond("GET", "/lol-gameflow/v1/gameflow-phase", 200, "\"None\"");
    assert!(matches!(next(&mut rx).await, LcuEvent::Connected(_)));
    cancel.cancel();
}
