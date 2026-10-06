#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code may fail loudly

//! Drives `Core` through a whole champ select against an in-memory fake client.

use app_core::{Core, CoreError, EventSink, SeedBuilds, SettingsStore};
use async_trait::async_trait;
use domain::builds::{Build, BuildCatalog, ItemBlock, PickStats, RunePageOption};
use domain::champ_select::{Action, ChampSelectSession, ChampionPick, Role, TeamMember};
use domain::item_set::ItemSet;
use domain::phase::GameflowPhase;
use domain::ports::{BuildSource, LcuEvent, LcuPort};
use domain::runes::{ExistingPage, NewRunePage, RuneSelection};
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

/// Stands in for OP.GG: a fixed answer for every pick.
#[derive(Debug)]
struct FakeSource {
    answer: Result<Option<Build>, PortError>,
    asked: Mutex<Vec<ChampionPick>>,
}

impl FakeSource {
    fn new(answer: Result<Option<Build>, PortError>) -> Arc<Self> {
        Arc::new(Self {
            answer,
            asked: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait]
impl BuildSource for FakeSource {
    fn name(&self) -> &str {
        "OP.GG"
    }
    async fn build(&self, pick: ChampionPick) -> Result<Option<Build>, PortError> {
        self.asked.lock().unwrap().push(pick);
        self.answer.clone()
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
                icon: String::new(),
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
                icon: String::new(),
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
    harness_with(lcu, settings, Vec::new())
}

/// `live` sources are asked before the seed builds, like OP.GG in the app.
fn harness_with(lcu: FakeLcu, settings: Settings, live: Vec<Arc<dyn BuildSource>>) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(dir.path().join("settings.json"));
    store.save(&settings).unwrap();
    let sink = Arc::new(RecordingSink::default());
    let mut sources = live;
    sources.push(Arc::new(SeedBuilds(builds())));
    let core = Core::new(sources, store, sink.clone());
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
        ..Default::default()
    }
}

/// Two valid Lux pages from the fixture trees: Sorcery/Precision and Precision/Sorcery.
fn live_lux_build() -> Build {
    let page = |primary: i64, sub: i64, perks: [i64; 9], games: u32| RunePageOption {
        runes: RuneSelection {
            primary_style_id: primary,
            sub_style_id: sub,
            perk_ids: perks.to_vec(),
        },
        stats: Some(PickStats {
            games,
            wins: games / 2,
            pick_rate: 0.5,
        }),
    };
    Build {
        champion_id: 99,
        role: Role::Middle,
        rune_pages: vec![
            page(
                8200,
                8000,
                [8229, 8226, 8210, 8237, 8009, 8014, 5008, 5008, 5011],
                900,
            ),
            page(
                8000,
                8200,
                [8008, 9101, 9104, 8017, 8233, 8236, 5005, 5008, 5011],
                100,
            ),
        ],
        summoner_spells: Some([14, 4]),
        item_blocks: vec![ItemBlock {
            title: "Starting items".into(),
            items: vec![1056, 2003, 2003],
        }],
        source: "OP.GG EUNE · Emerald+ · patch 16.19".into(),
    }
}

fn last_page_created(lcu: &FakeLcu) -> String {
    lcu.calls()
        .into_iter()
        .rev()
        .find(|c| c.starts_with("create_page"))
        .unwrap()
}

fn is_done(r: &StepResultDto) -> bool {
    matches!(r, StepResultDto::Done { .. })
}

// ---------- tests ----------

#[tokio::test]
async fn imports_on_hover_and_not_again_when_locking_the_same_champion() {
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
    assert_eq!(h.imports().len(), 1, "hovering imports");

    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", false))))
        .await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", true))))
        .await;

    let imports = h.imports();
    assert_eq!(
        imports.len(),
        1,
        "repeated updates and locking the hovered champion must not re-import"
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
async fn hovering_another_champion_imports_for_it() {
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
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(222, "middle", false))))
        .await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(222, "middle", true))))
        .await;
    let names: Vec<String> = h.imports().into_iter().map(|r| r.champion_name).collect();
    assert_eq!(names, ["Lux", "Jinx"]);
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
async fn reimport_needs_a_selected_pick() {
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

// ---------- live builds and rune page choice ----------

#[tokio::test]
async fn live_build_is_used_and_offers_every_rune_page() {
    let live = FakeSource::new(Ok(Some(live_lux_build())));
    let h = harness_with(
        FakeLcu {
            owned_pages: 2,
            ..Default::default()
        },
        Settings::default(),
        vec![live.clone()],
    );
    h.connect_and_enter_champ_select().await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", true))))
        .await;

    assert_eq!(
        *live.asked.lock().unwrap(),
        [ChampionPick {
            champion_id: 99,
            role: Role::Middle,
            mode: domain::champ_select::GameMode::SummonersRift,
        }]
    );
    let r = &h.imports()[0];
    assert_eq!(
        r.source.as_deref(),
        Some("OP.GG EUNE · Emerald+ · patch 16.19")
    );
    assert_eq!(r.rune_pages.len(), 2);
    assert_eq!(r.selected_rune_page, Some(0));
    assert_eq!(r.rune_pages[0].games, Some(900));
    assert_eq!(r.rune_pages[0].win_rate, Some(0.5));
    assert!(last_page_created(&h.lcu)
        .ends_with("[8229, 8226, 8210, 8237, 8009, 8014, 5008, 5008, 5011]"));
    // Live spells, with Flash moved to D.
    assert!(h.lcu.calls().contains(&"spells [4, 14]".to_string()));
}

#[tokio::test]
async fn choosing_another_rune_page_sends_it_and_import_again_keeps_it() {
    let h = harness_with(
        FakeLcu {
            owned_pages: 2,
            ..Default::default()
        },
        Settings::default(),
        vec![FakeSource::new(Ok(Some(live_lux_build())))],
    );
    h.connect_and_enter_champ_select().await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", true))))
        .await;

    let r = h.core.apply_rune_page(1).await.unwrap();
    assert_eq!(r.selected_rune_page, Some(1));
    assert!(is_done(&r.runes), "{r:?}");
    assert!(last_page_created(&h.lcu)
        .ends_with("[8008, 9101, 9104, 8017, 8233, 8236, 5005, 5008, 5011]"));
    assert_eq!(h.imports().last(), Some(&r), "the UI is told too");
    assert_eq!(h.core.last_import(), Some(r));

    let again = h.core.reimport().await.unwrap();
    assert_eq!(again.selected_rune_page, Some(1));
    assert!(last_page_created(&h.lcu).starts_with("create_page LoL Overlay: Lux [8008"));

    assert!(matches!(
        h.core.apply_rune_page(7).await,
        Err(CoreError::NoSuchRunePage)
    ));
}

#[tokio::test]
async fn choosing_a_rune_page_needs_an_imported_build() {
    let h = harness(
        FakeLcu {
            owned_pages: 2,
            ..Default::default()
        },
        Settings::default(),
    );
    h.connect_and_enter_champ_select().await;
    assert!(matches!(
        h.core.apply_rune_page(0).await,
        Err(CoreError::NothingToImport)
    ));
}

#[tokio::test]
async fn rune_pages_invalid_this_patch_are_not_offered() {
    let mut build = live_lux_build();
    build.rune_pages[1].runes.perk_ids[0] = 123_456;
    let h = harness_with(
        FakeLcu {
            owned_pages: 2,
            ..Default::default()
        },
        Settings::default(),
        vec![FakeSource::new(Ok(Some(build)))],
    );
    h.connect_and_enter_champ_select().await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", true))))
        .await;
    assert_eq!(h.imports()[0].rune_pages.len(), 1);
}

#[tokio::test]
async fn unreachable_live_source_falls_back_to_seed_builds() {
    let h = harness_with(
        FakeLcu {
            owned_pages: 2,
            ..Default::default()
        },
        Settings::default(),
        vec![FakeSource::new(Err(PortError::Transport(
            "timed out".into(),
        )))],
    );
    h.connect_and_enter_champ_select().await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(99, "middle", true))))
        .await;
    let r = &h.imports()[0];
    assert!(is_done(&r.runes), "{r:?}");
    assert_eq!(
        r.source.as_deref(),
        Some("Built-in build (live statistics unavailable)")
    );
}

#[tokio::test]
async fn unreachable_live_source_without_a_seed_build_is_a_failure_with_the_reason() {
    let h = harness_with(
        FakeLcu {
            owned_pages: 2,
            ..Default::default()
        },
        Settings::default(),
        vec![FakeSource::new(Err(PortError::Transport(
            "timed out".into(),
        )))],
    );
    h.connect_and_enter_champ_select().await;
    h.core
        .handle(LcuEvent::ChampSelect(Some(session(1, "middle", true))))
        .await;
    match &h.imports()[0].runes {
        StepResultDto::Failed { error } => {
            assert!(
                error.contains("OP.GG") && error.contains("timed out"),
                "{error}"
            )
        }
        other => panic!("expected failure, got {other:?}"),
    }
    assert!(h.lcu.calls().is_empty());
}

#[tokio::test]
async fn rune_catalog_describes_the_patch_trees() {
    let h = harness(FakeLcu::default(), Settings::default());
    let c = h.core.rune_catalog().unwrap();
    assert_eq!(c.trees.len(), 2);
    assert_eq!(c.trees[1].name, "Sorcery");
    assert_eq!(c.trees[1].slots[0].len(), 3);
    assert_eq!(c.stat_shard_rows.len(), 3);
}
