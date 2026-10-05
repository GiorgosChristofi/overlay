#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code may fail loudly

//! Drives `Core` through a whole champ select against an in-memory fake client.

use app_core::{Core, CoreError, EventSink, SettingsStore};
use async_trait::async_trait;
use domain::builds::BuildCatalog;
use domain::champ_select::{Action, ChampSelectSession, TeamMember};
use domain::item_set::ItemSet;
use domain::phase::GameflowPhase;
use domain::ports::{LcuEvent, LcuPort};
use domain::runes::{ExistingPage, NewRunePage};
use domain::settings::Settings;
use domain::static_catalog::{RuneTree, StaticCatalog};
use domain::summoner::Summoner;
use domain::PortError;
use ipc_types::{ImportReportDto, SessionStateDto, StatusDto, StepResultDto};
use std::sync::{Arc, Mutex};

// ---------- fakes ----------

#[derive(Debug, Default)]
struct FakeLcu {
    pages: Mutex<Vec<ExistingPage>>,
    owned_pages: u32,
    calls: Mutex<Vec<String>>,
    fail_runes: bool,
}

impl FakeLcu {
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    fn log(&self, s: String) {
        self.calls.lock().unwrap().push(s);
    }
}

#[async_trait]
impl LcuPort for FakeLcu {
    async fn current_summoner(&self) -> Result<Summoner, PortError> {
        Ok(Summoner {
            game_name: "Tester".into(),
            tag_line: "EUW".into(),
            summoner_level: 30,
            profile_icon_id: 29,
            puuid: "p".into(),
            summoner_id: 1234,
        })
    }
    async fn gameflow_phase(&self) -> Result<GameflowPhase, PortError> {
        Ok(GameflowPhase::None)
    }
    async fn champ_select_session(&self) -> Result<Option<ChampSelectSession>, PortError> {
        Ok(None)
    }
    async fn rune_pages(&self) -> Result<Vec<ExistingPage>, PortError> {
        if self.fail_runes {
            return Err(PortError::Http {
                status: 500,
                body: "boom".into(),
            });
        }
        Ok(self.pages.lock().unwrap().clone())
    }
    async fn owned_rune_page_count(&self) -> Result<u32, PortError> {
        Ok(self.owned_pages)
    }
    async fn delete_rune_page(&self, id: i64) -> Result<(), PortError> {
        self.log(format!("delete_page {id}"));
        self.pages.lock().unwrap().retain(|p| p.id != id);
        Ok(())
    }
    async fn create_rune_page(&self, page: &NewRunePage) -> Result<(), PortError> {
        self.log(format!(
            "create_page {} {:?}",
            page.name, page.selected_perk_ids
        ));
        Ok(())
    }
    async fn upsert_item_set(&self, summoner_id: i64, set: &ItemSet) -> Result<(), PortError> {
        self.log(format!("item_set {summoner_id} {}", set.uid));
        Ok(())
    }
    async fn set_summoner_spells(&self, spells: [i64; 2]) -> Result<(), PortError> {
        self.log(format!("spells {spells:?}"));
        Ok(())
    }
}

#[derive(Debug, Default)]
struct RecordingSink {
    statuses: Mutex<Vec<StatusDto>>,
    imports: Mutex<Vec<ImportReportDto>>,
}

impl EventSink for RecordingSink {
    fn status(&self, status: &StatusDto) {
        self.statuses.lock().unwrap().push(status.clone());
    }
    fn import(&self, report: &ImportReportDto) {
        self.imports.lock().unwrap().push(report.clone());
    }
}

// ---------- helpers ----------

fn catalog() -> Arc<StaticCatalog> {
    let mut c = StaticCatalog {
        version: "16.19.1".into(),
        image_base_url: "https://cdn.test/img".into(),
        rune_trees: vec![
            RuneTree {
                id: 8000,
                name: "Precision".into(),
                slots: vec![
                    vec![8005, 8008, 8021, 8010],
                    vec![9101, 9111, 8009],
                    vec![9104, 9105, 9103],
                    vec![8014, 8017, 8299],
                ],
            },
            RuneTree {
                id: 8200,
                name: "Sorcery".into(),
                slots: vec![
                    vec![8214, 8229, 8230],
                    vec![8224, 8226, 8275],
                    vec![8210, 8234, 8233],
                    vec![8237, 8232, 8236],
                ],
            },
        ],
        stat_shard_rows: vec![
            vec![5008, 5005, 5007],
            vec![5008, 5010, 5001],
            vec![5011, 5013, 5001],
        ],
        ..StaticCatalog::default()
    };
    c.champions.extend([
        (99, "Lux".to_string()),
        (222, "Jinx".to_string()),
        (1, "Annie".to_string()),
    ]);
    c.summoner_spells.extend([
        (4, "Flash".to_string()),
        (14, "Ignite".to_string()),
        (7, "Heal".to_string()),
    ]);
    c.items.extend([
        (1056, "Doran's Ring".to_string()),
        (2003, "Health Potion".to_string()),
        (1055, "Doran's Blade".to_string()),
    ]);
    Arc::new(c)
}

fn builds() -> BuildCatalog {
    BuildCatalog::from_json(include_str!("../../../data/builds/seed.json")).unwrap()
}

struct Harness {
    core: Arc<Core>,
    lcu: Arc<FakeLcu>,
    sink: Arc<RecordingSink>,
    _dir: tempfile::TempDir,
}

fn harness(lcu: FakeLcu, settings: Settings) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(dir.path().join("settings.json"));
    store.save(&settings).unwrap();
    let sink = Arc::new(RecordingSink::default());
    let core = Core::new(builds(), store, sink.clone());
    core.set_catalog(catalog());
    Harness {
        core,
        lcu: Arc::new(lcu),
        sink,
        _dir: dir,
    }
}

impl Harness {
    async fn connect_and_enter_champ_select(&self) {
        self.core
            .handle(LcuEvent::Connected(self.lcu.clone()))
            .await;
        self.core
            .handle(LcuEvent::Phase(GameflowPhase::ChampSelect))
            .await;
    }
    fn imports(&self) -> Vec<ImportReportDto> {
        self.sink.imports.lock().unwrap().clone()
    }
}

fn session(champion_id: i64, position: &str, completed: bool) -> ChampSelectSession {
    ChampSelectSession {
        local_player_cell_id: 2,
        my_team: vec![TeamMember {
            cell_id: 2,
            champion_id,
            assigned_position: position.into(),
            ..Default::default()
        }],
        actions: vec![vec![Action {
            actor_cell_id: 2,
            champion_id,
            completed,
            kind: "pick".into(),
            is_in_progress: !completed,
        }]],
    }
}

fn is_done(r: &StepResultDto) -> bool {
    matches!(r, StepResultDto::Done { .. })
}

// ---------- tests ----------

#[tokio::test]
async fn imports_once_when_the_player_locks_in() {
    let h = harness(
        FakeLcu {
            owned_pages: 2,
            ..Default::default()
        },
        Settings::default(),
    );
    h.connect_and_enter_champ_select().await;

    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", false))))
        .await;
    assert!(h.imports().is_empty(), "hovering must not import");

    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", true))))
        .await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", true))))
        .await;

    let imports = h.imports();
    assert_eq!(
        imports.len(),
        1,
        "repeated session updates must not re-import"
    );
    let r = &imports[0];
    assert_eq!(
        (r.champion_name.as_str(), r.role.as_str()),
        ("Lux", "Middle")
    );
    assert!(
        is_done(&r.runes) && is_done(&r.item_set) && is_done(&r.summoner_spells),
        "{r:?}"
    );
    assert_eq!(
        h.lcu.calls(),
        vec![
            "create_page LoL Overlay: Lux [8229, 8226, 8210, 8237, 8009, 8014, 5008, 5008, 5011]"
                .to_string(),
            "item_set 1234 lol-overlay-99".to_string(),
            "spells [4, 14]".to_string(),
        ]
    );
}

#[tokio::test]
async fn puts_flash_on_the_key_the_player_chose() {
    let h = harness(
        FakeLcu {
            owned_pages: 2,
            ..Default::default()
        },
        Settings::default(),
    );
    h.connect_and_enter_champ_select().await;
    // Jinx's seed build lists Heal first; Flash must still land on D.
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(222, "bottom", true))))
        .await;
    assert!(h.lcu.calls().contains(&"spells [4, 7]".to_string()));
    assert_eq!(
        h.imports()[0].summoner_spells,
        StepResultDto::Done {
            detail: "Flash on D, Heal on F".into()
        }
    );
}

#[tokio::test]
async fn replaces_our_old_page_but_never_the_players_own_unless_allowed() {
    let pages = vec![
        ExistingPage {
            id: 1,
            name: "My page".into(),
            is_editable: true,
            is_deletable: true,
            current: true,
        },
        ExistingPage {
            id: 2,
            name: "LoL Overlay: Jinx".into(),
            is_editable: true,
            is_deletable: true,
            current: false,
        },
    ];
    let h = harness(
        FakeLcu {
            owned_pages: 2,
            pages: Mutex::new(pages),
            ..Default::default()
        },
        Settings::default(),
    );
    h.connect_and_enter_champ_select().await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", true))))
        .await;
    assert_eq!(h.lcu.calls()[0], "delete_page 2");
}

#[tokio::test]
async fn full_page_slots_skip_runes_with_a_reason_and_still_do_the_rest() {
    let pages = vec![ExistingPage {
        id: 1,
        name: "My page".into(),
        is_editable: true,
        is_deletable: true,
        current: false,
    }];
    let h = harness(
        FakeLcu {
            owned_pages: 1,
            pages: Mutex::new(pages),
            ..Default::default()
        },
        Settings::default(),
    );
    h.connect_and_enter_champ_select().await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", true))))
        .await;

    let r = &h.imports()[0];
    match &r.runes {
        StepResultDto::Skipped { reason } => assert!(reason.contains("Settings")),
        other => panic!("expected skipped, got {other:?}"),
    }
    assert!(is_done(&r.item_set) && is_done(&r.summoner_spells));
    assert!(!h.lcu.calls().iter().any(|c| c.starts_with("delete_page")));
}

#[tokio::test]
async fn overwrites_active_page_when_one_page_exists() {
    let pages = vec![ExistingPage {
        id: 1,
        name: "My page".into(),
        is_editable: true,
        is_deletable: true,
        current: true,
    }];
    let h = harness(
        FakeLcu {
            owned_pages: 1,
            pages: Mutex::new(pages),
            ..Default::default()
        },
        Settings::default(),
    );
    h.connect_and_enter_champ_select().await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", true))))
        .await;

    let r = &h.imports()[0];
    assert!(is_done(&r.runes), "{r:?}");
    assert_eq!(h.lcu.calls()[0], "delete_page 1");
    assert!(h.lcu.calls()[1].starts_with("create_page LoL Overlay: Lux"));
}

#[tokio::test]
async fn a_failing_step_does_not_block_the_others() {
    let h = harness(
        FakeLcu {
            owned_pages: 2,
            fail_runes: true,
            ..Default::default()
        },
        Settings::default(),
    );
    h.connect_and_enter_champ_select().await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", true))))
        .await;
    let r = &h.imports()[0];
    assert!(matches!(r.runes, StepResultDto::Failed { .. }));
    assert!(is_done(&r.item_set) && is_done(&r.summoner_spells));
}

#[tokio::test]
async fn disabled_steps_are_skipped() {
    let settings = Settings {
        import_runes: false,
        import_item_sets: false,
        ..Settings::default()
    };
    let h = harness(
        FakeLcu {
            owned_pages: 2,
            ..Default::default()
        },
        settings,
    );
    h.connect_and_enter_champ_select().await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", true))))
        .await;
    assert_eq!(h.lcu.calls(), vec!["spells [4, 14]".to_string()]);
}

#[tokio::test]
async fn champion_without_a_build_is_reported_not_imported() {
    let h = harness(
        FakeLcu {
            owned_pages: 2,
            ..Default::default()
        },
        Settings::default(),
    );
    h.connect_and_enter_champ_select().await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(1, "middle", true))))
        .await;
    assert_eq!(
        h.imports()[0].runes,
        StepResultDto::Skipped {
            reason: "No build for Annie yet".into()
        }
    );
    assert!(h.lcu.calls().is_empty());
}

#[tokio::test]
async fn a_new_champ_select_imports_again() {
    let h = harness(
        FakeLcu {
            owned_pages: 2,
            ..Default::default()
        },
        Settings::default(),
    );
    h.connect_and_enter_champ_select().await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", true))))
        .await;
    h.core.handle(LcuEvent::Phase(GameflowPhase::Lobby)).await; // someone dodged
    h.core
        .handle(LcuEvent::Phase(GameflowPhase::ChampSelect))
        .await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", true))))
        .await;
    assert_eq!(h.imports().len(), 2);
}

#[tokio::test]
async fn status_reflects_connection_profile_and_disconnect() {
    let h = harness(FakeLcu::default(), Settings::default());
    h.core.handle(LcuEvent::Connected(h.lcu.clone())).await;
    let s = h.core.status();
    assert_eq!(s.session, SessionStateDto::ClientReady);
    let summoner = s.summoner.unwrap();
    assert_eq!(summoner.riot_id, "Tester#EUW");
    assert_eq!(
        summoner.profile_icon_url.as_deref(),
        Some("https://cdn.test/img/profileicon/29.png")
    );
    assert_eq!(s.patch.as_deref(), Some("16.19.1"));

    h.core.handle(LcuEvent::Disconnected).await;
    let s = h.core.status();
    assert_eq!(
        (s.session, s.summoner),
        (SessionStateDto::Disconnected, None)
    );
    assert!(h.sink.statuses.lock().unwrap().len() >= 3);
}

#[tokio::test]
async fn reimport_needs_a_locked_pick() {
    let h = harness(
        FakeLcu {
            owned_pages: 2,
            ..Default::default()
        },
        Settings::default(),
    );
    h.connect_and_enter_champ_select().await;
    assert!(matches!(
        h.core.reimport().await,
        Err(CoreError::NothingToImport)
    ));
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", true))))
        .await;
    assert!(h.core.reimport().await.is_ok());
    assert_eq!(h.imports().len(), 2);
}

#[tokio::test]
async fn saved_settings_apply_to_the_next_import() {
    let h = harness(
        FakeLcu {
            owned_pages: 2,
            ..Default::default()
        },
        Settings::default(),
    );
    h.core
        .save_settings(Settings {
            import_summoner_spells: false,
            ..Settings::default()
        })
        .unwrap();
    h.connect_and_enter_champ_select().await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", true))))
        .await;
    assert!(matches!(
        h.imports()[0].summoner_spells,
        StepResultDto::Skipped { .. }
    ));
}
