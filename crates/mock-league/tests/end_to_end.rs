#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code may fail loudly

//! The app's core against a fake League: the real LCU connector (TLS, WAMP
//! WebSocket, REST) and the real Live Client, talking to `mock-league` over
//! HTTPS on 127.0.0.1. Builds come from the seed file and static data from
//! the trimmed Data Dragon fixtures, so nothing here touches the internet.

use app_core::{Core, EventSink, SeedBuilds, SettingsStore};
use domain::builds::BuildCatalog;
use domain::ports::LiveGamePort;
use domain::settings::Settings;
use ipc_types::{ImportReportDto, OverlayMessage, StatusDto, StepResultDto};
use lcu_client::{run_connector, ConnectorConfig, LiveClient, LockfileInfo};
use mock_league::{
    ChampSelectStep, LiveStep, MockLeague, Options, Scenario, Setup, Snapshot, Step,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const LUX: i64 = 99;
const WAIT: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
struct RecordingSink {
    imports: Mutex<Vec<ImportReportDto>>,
    overlays: Mutex<Vec<OverlayMessage>>,
}

impl EventSink for RecordingSink {
    fn status(&self, _status: &StatusDto) {}
    fn import(&self, report: &ImportReportDto) {
        self.imports.lock().unwrap().push(report.clone());
    }
    fn overlay(&self, message: &OverlayMessage) {
        self.overlays.lock().unwrap().push(message.clone());
    }
}

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/../static-data/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The app's core wired to a mock, the way `apps/desktop` wires it to League.
struct App {
    mock: MockLeague,
    core: Arc<Core>,
    sink: Arc<RecordingSink>,
    cancel: CancellationToken,
    _dir: tempfile::TempDir,
}

impl Drop for App {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl App {
    async fn start(setup: Setup, settings: Settings) -> Self {
        let mock = MockLeague::start(Options {
            setup,
            ..Options::default()
        })
        .await
        .unwrap();
        let root = mock.root_pem().as_bytes().to_vec();
        Self::start_trusting(mock, root, settings)
    }

    /// Like `start`, but the app trusts `root` instead of the mock's own root.
    fn start_trusting(mock: MockLeague, root: Vec<u8>, settings: Settings) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("settings.json"));
        store.save(&settings).unwrap();
        let seed = BuildCatalog::from_json(include_str!("../../../data/builds/seed.json")).unwrap();
        let sink = Arc::new(RecordingSink::default());
        let core = Core::new(vec![Arc::new(SeedBuilds(seed))], store, sink.clone());
        let catalog = static_data::parse_catalog(
            "16.19.1",
            &static_data::RawFiles {
                champions: &fixture("champion.json"),
                summoners: &fixture("summoner.json"),
                runes: &fixture("runesReforged.json"),
                items: &fixture("item.json"),
            },
        )
        .unwrap();
        core.set_catalog(Arc::new(catalog));

        let cancel = CancellationToken::new();
        let lockfile = LockfileInfo::parse(&mock.lockfile()).unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::channel(64);
        tokio::spawn(run_connector(
            ConnectorConfig {
                riot_root_pem: Some(root.clone()),
                retry_interval: Duration::from_millis(100),
                discover: Arc::new(move || Some(lockfile.clone())),
            },
            tx,
            cancel.clone(),
        ));
        let events = core.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                events.handle(event).await;
            }
        });

        let tls = lcu_client::tls::riot_client_config(&root).unwrap();
        let live = LiveClient::new(mock.live_url(), Some(tls)).unwrap();
        let (poll_core, poll_cancel) = (core.clone(), cancel.clone());
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    () = poll_cancel.cancelled() => return,
                    () = tokio::time::sleep(Duration::from_millis(50)) => {}
                }
                if poll_core.in_game() {
                    if let Ok(snapshot) = live.snapshot().await {
                        poll_core.live_tick(snapshot);
                    }
                }
            }
        });
        Self {
            mock,
            core,
            sink,
            cancel,
            _dir: dir,
        }
    }

    async fn until(&self, what: &str, check: impl Fn(&Snapshot) -> bool) -> Snapshot {
        match self.mock.wait_for(WAIT, check).await {
            Ok(s) => s,
            Err(s) => panic!("timed out waiting for {what}; mock state: {s:#?}"),
        }
    }

    async fn connected(&self) {
        // The connector subscribes over the WebSocket after its first GET.
        self.until("the app to connect", |s| {
            s.requests.iter().any(|r| r.path == "/" && r.status == 101)
        })
        .await;
        // Give the subscribe frames time to land before events are pushed.
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    fn imports(&self) -> Vec<ImportReportDto> {
        self.sink.imports.lock().unwrap().clone()
    }

    fn last_view(&self) -> Option<ipc_types::OverlayViewDto> {
        match self.sink.overlays.lock().unwrap().last() {
            Some(OverlayMessage::Items(view)) => Some((**view).clone()),
            _ => None,
        }
    }

    async fn view_until(
        &self,
        what: &str,
        check: impl Fn(&ipc_types::OverlayViewDto) -> bool,
    ) -> ipc_types::OverlayViewDto {
        let deadline = tokio::time::Instant::now() + WAIT;
        loop {
            if let Some(view) = self.last_view().filter(|v| check(v)) {
                return view;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out waiting for {what}; last view: {:#?}",
                self.last_view()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

fn hover(champion: i64) -> Step {
    Step::ChampSelect(ChampSelectStep {
        champion,
        position: "middle".into(),
        ..ChampSelectStep::default()
    })
}

fn lock(champion: i64) -> Step {
    Step::ChampSelect(ChampSelectStep {
        champion,
        position: "middle".into(),
        locked: true,
        ..ChampSelectStep::default()
    })
}

fn our_pages(s: &Snapshot) -> usize {
    s.rune_pages
        .iter()
        .filter(|p| p.name.starts_with("LoL Overlay"))
        .count()
}

fn posts_of_pages(s: &Snapshot) -> usize {
    s.writes()
        .iter()
        .filter(|r| r.method == "POST" && r.path == "/lol-perks/v1/pages")
        .count()
}

#[tokio::test]
async fn a_whole_game_imports_on_hover_and_follows_purchases() {
    let app = App::start(Setup::default(), Settings::default()).await;
    app.connected().await;

    app.mock
        .run(&[Step::Phase("ChampSelect".into()), hover(LUX)])
        .await;
    let s = app
        .until("the rune page, item set and spells", |s| {
            our_pages(s) == 1 && s.overlay_item_set(1234).is_some() && s.my_selection.is_some()
        })
        .await;
    let page = s.rune_pages.iter().find(|p| p.current).unwrap();
    assert_eq!(page.name, "LoL Overlay: Lux");
    assert_eq!(page.primary_style_id, 8200);
    assert_eq!(s.my_selection, Some([4, 14]), "Flash on D by default");
    let report = &app.imports()[0];
    assert!(
        matches!(report.runes, StepResultDto::Done { .. }),
        "{report:?}"
    );

    // Locking the hovered champion must not import again.
    app.mock.apply(&lock(LUX));
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(posts_of_pages(&app.mock.snapshot()), 1);

    app.mock
        .run(&[
            Step::EndChampSelect,
            Step::Phase("InProgress".into()),
            Step::NoGame,
        ])
        .await;
    app.mock.apply(&Step::Live(LiveStep {
        gold: 500.0,
        ..LiveStep::default()
    }));
    let view = app
        .view_until("gold in the overlay", |v| v.gold == Some(500))
        .await;
    let starting: Vec<i32> = view.starting.unwrap().items.iter().map(|i| i.id).collect();
    assert_eq!(starting, [1056, 2003]);

    app.mock.apply(&Step::Live(LiveStep {
        gold: 10.0,
        items: vec![(1056, 1), (2003, 1)],
        game_time: 70.0,
        ..LiveStep::default()
    }));
    let view = app
        .view_until("the starter items to count as bought", |v| {
            v.gold == Some(10) && v.starting.is_none()
        })
        .await;
    assert!(
        view.next.is_some(),
        "a next item once starters are bought: {view:#?}"
    );

    app.mock.apply(&Step::Phase("EndOfGame".into()));
    app.until("the phase", |s| s.phase == "EndOfGame").await;
}

#[tokio::test]
async fn the_app_only_calls_what_it_needs_and_always_authenticates() {
    let app = App::start(Setup::default(), Settings::default()).await;
    app.connected().await;
    app.mock
        .run(&[
            Step::Phase("ChampSelect".into()),
            hover(LUX),
            Step::Wait(300),
            lock(LUX),
            Step::Phase("InProgress".into()),
            Step::Live(LiveStep::default()),
            Step::Wait(300),
            Step::Phase("EndOfGame".into()),
        ])
        .await;
    let s = app.until("the import", |s| our_pages(s) == 1).await;

    for r in &s.requests {
        assert!(r.authorized, "unauthenticated request: {r:?}");
        assert!(
            r.known,
            "the app called an endpoint it has no use for: {r:?}"
        );
    }
    // The client is only ever changed in these places: runes, our item set, spells.
    let allowed = |r: &&mock_league::Recorded| {
        matches!(
            (r.method.as_str(), r.path.as_str()),
            ("POST", "/lol-perks/v1/pages")
                | ("PATCH", "/lol-champ-select/v1/session/my-selection")
                | ("PUT", "/lol-item-sets/v1/item-sets/1234/sets")
        ) || (r.method == "DELETE" && r.path.starts_with("/lol-perks/v1/pages/"))
    };
    let writes = s.writes();
    let unexpected: Vec<_> = writes.iter().filter(|r| !allowed(r)).collect();
    assert!(unexpected.is_empty(), "unexpected writes: {unexpected:#?}");
    // The live API is read-only and only read with GET.
    assert!(s
        .requests
        .iter()
        .filter(|r| r.server == "live")
        .all(|r| r.method == "GET"));
}

#[tokio::test]
async fn a_client_signed_by_another_root_is_never_trusted() {
    let mock = MockLeague::start(Options::default()).await.unwrap();
    let impostor_root = MockLeague::start(Options::default())
        .await
        .unwrap()
        .root_pem()
        .as_bytes()
        .to_vec();
    let app = App::start_trusting(mock, impostor_root, Settings::default());

    tokio::time::sleep(Duration::from_secs(1)).await;
    // The TLS handshake fails, so not even the password reaches the server.
    assert!(app.mock.snapshot().requests.is_empty());
    assert_eq!(
        app.core.status().session,
        ipc_types::SessionStateDto::Disconnected
    );
}

#[tokio::test]
async fn hovering_never_touches_the_players_pages_when_all_slots_are_used() {
    let setup = Setup {
        owned_page_count: 1,
        ..Setup::default()
    };
    let app = App::start(setup, Settings::default()).await;
    app.connected().await;
    app.mock
        .run(&[Step::Phase("ChampSelect".into()), hover(LUX)])
        .await;
    app.until("the item set", |s| s.overlay_item_set(1234).is_some())
        .await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let s = app.mock.snapshot();
    assert!(s.rune_pages.iter().any(|p| p.name == "My page"), "{s:#?}");
    assert!(!s.writes().iter().any(|r| r.method == "DELETE"), "{s:#?}");
    assert!(matches!(
        app.imports()[0].runes,
        StepResultDto::Skipped { .. }
    ));
}

#[tokio::test]
async fn replaces_the_active_page_when_full_and_the_player_allows_it() {
    let setup = Setup {
        owned_page_count: 1,
        ..Setup::default()
    };
    let settings = Settings {
        replace_current_page_when_full: true,
        ..Settings::default()
    };
    let app = App::start(setup, settings).await;
    app.connected().await;
    app.mock
        .run(&[Step::Phase("ChampSelect".into()), hover(LUX)])
        .await;
    let s = app.until("our page", |s| our_pages(s) == 1).await;
    assert!(!s.rune_pages.iter().any(|p| p.name == "My page"));
    // Riot's preset pages are never deleted.
    assert!(s.rune_pages.iter().any(|p| p.name == "Domination"));
}

#[tokio::test]
async fn keeps_other_tools_item_sets() {
    let other = serde_json::json!({
        "accountId": 1234,
        "timestamp": 1,
        "someRiotField": "kept",
        "itemSets": [{ "uid": "someone-else", "title": "Theirs", "blocks": [] }],
    });
    let setup = Setup {
        item_sets: Some(other),
        ..Setup::default()
    };
    let app = App::start(setup, Settings::default()).await;
    app.connected().await;
    app.mock
        .run(&[Step::Phase("ChampSelect".into()), hover(LUX)])
        .await;
    let s = app
        .until("our item set", |s| s.overlay_item_set(1234).is_some())
        .await;
    let doc = &s.item_sets[&1234];
    assert_eq!(doc["someRiotField"], "kept");
    let uids: Vec<&str> = doc["itemSets"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["uid"].as_str())
        .collect();
    assert!(uids.contains(&"someone-else"), "{uids:?}");
}

#[tokio::test]
async fn reconnects_after_the_client_restarts_and_imports_again() {
    let app = App::start(Setup::default(), Settings::default()).await;
    app.connected().await;
    app.mock.apply(&Step::DropConnections);
    app.until("a second WebSocket", |s| {
        s.requests
            .iter()
            .filter(|r| r.path == "/" && r.status == 101)
            .count()
            >= 2
    })
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    app.mock
        .run(&[Step::Phase("ChampSelect".into()), hover(LUX)])
        .await;
    app.until("the import after reconnecting", |s| our_pages(s) == 1)
        .await;
}

#[tokio::test]
async fn aram_imports_the_assigned_champion() {
    let app = App::start(Setup::default(), Settings::default()).await;
    app.connected().await;
    app.mock
        .run(&[
            Step::Phase("ChampSelect".into()),
            Step::ChampSelect(ChampSelectStep {
                champion: 222,
                aram: true,
                ..ChampSelectStep::default()
            }),
        ])
        .await;
    let s = app.until("the import", |s| our_pages(s) == 1).await;
    assert_eq!(
        s.rune_pages.iter().find(|p| p.current).unwrap().name,
        "LoL Overlay: Jinx"
    );
}

/// Every scenario in `scenarios/` parses and plays through the app without
/// unauthenticated or unknown requests (waits shortened).
#[tokio::test]
async fn bundled_scenarios_play_through_cleanly() {
    let dir = format!("{}/scenarios", env!("CARGO_MANIFEST_DIR"));
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        count += 1;
        let scenario = Scenario::from_json(&std::fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let app = App::start(scenario.setup.clone(), Settings::default()).await;
        app.connected().await;
        let steps: Vec<Step> = scenario
            .steps
            .iter()
            .map(|s| match s {
                Step::Wait(ms) => Step::Wait((*ms).min(150)),
                other => other.clone(),
            })
            .collect();
        app.mock.run(&steps).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        for r in &app.mock.snapshot().requests {
            assert!(r.authorized && r.known, "{}: {r:?}", path.display());
        }
    }
    assert!(count > 0, "no scenarios in {dir}");
}
