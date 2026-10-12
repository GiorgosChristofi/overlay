//! A fake League of Legends for testing the app without the game.
//!
//! [`MockLeague`] runs, on `127.0.0.1` only:
//! - the League client API (LCU) over HTTPS with a WAMP WebSocket, guarded by
//!   a `riot:<token>` password like the real client;
//! - the game's Live Client Data API over HTTPS;
//! - optionally a plain-HTTP control API to drive it by hand.
//!
//! Both HTTPS servers present a certificate from a throwaway root
//! ([`MockLeague::root_pem`]); the app is told to trust that root instead of
//! Riot's. Drive it with [`Step`]s or a whole [`Scenario`], then inspect what
//! the app did with [`Driver::snapshot`].

mod control;
mod lcu;
mod live;
mod pki;
pub mod scenario;
mod serve;
mod state;

pub use scenario::{ChampSelectStep, LiveStep, Scenario, Setup, Step};
pub use state::{Recorded, RunePage, Snapshot};

use base64::Engine;
use state::{Push, Shared, State};
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// Ports to listen on; `0` picks a free one.
#[derive(Debug, Clone)]
pub struct Options {
    pub lcu_port: u16,
    /// The real app always asks `127.0.0.1:2999`; tests use `0`.
    pub live_port: u16,
    pub control_port: Option<u16>,
    /// The lockfile password.
    pub token: String,
    pub setup: Setup,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            lcu_port: 0,
            live_port: 0,
            control_port: None,
            token: random_token(),
            setup: Setup::default(),
        }
    }
}

/// Applies steps and reads state; shared by [`MockLeague`] and the control API.
#[derive(Debug)]
pub struct Driver {
    shared: Arc<Shared>,
    riot_id: String,
    summoner_id: i64,
}

impl Driver {
    pub fn summoner_id(&self) -> i64 {
        self.summoner_id
    }

    /// Applies one step. `Wait` is ignored here; [`Driver::run`] sleeps.
    pub fn apply(&self, step: &Step) {
        tracing::info!(?step, "mock step");
        match step {
            Step::Phase(phase) => self.shared.set_phase(phase),
            Step::ChampSelect(s) => self
                .shared
                .set_champ_select(Some(s.session(self.summoner_id))),
            Step::ChampSelectRaw(session) => self.shared.set_champ_select(Some(session.clone())),
            Step::EndChampSelect => self.shared.set_champ_select(None),
            Step::Live(live) => {
                self.shared.state().live = Some(live.all_game_data(&self.riot_id));
            }
            Step::LiveRaw(data) => self.shared.state().live = Some(data.clone()),
            Step::NoGame => self.shared.state().live = None,
            Step::DropConnections => self.shared.send(Push::CloseAll),
            Step::Wait(_) => {}
        }
    }

    /// Applies steps in order, sleeping on `Wait`.
    pub async fn run(&self, steps: &[Step]) {
        for step in steps {
            match step {
                Step::Wait(ms) => tokio::time::sleep(Duration::from_millis(*ms)).await,
                other => self.apply(other),
            }
        }
    }

    /// What the client and game know now, and every request so far.
    pub fn snapshot(&self) -> Snapshot {
        self.shared.state().snapshot()
    }

    /// Waits until `check` holds for the snapshot, up to `timeout`. The error
    /// carries the last snapshot, to show what happened instead.
    pub async fn wait_for(
        &self,
        timeout: Duration,
        check: impl Fn(&Snapshot) -> bool,
    ) -> Result<Snapshot, Box<Snapshot>> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let snapshot = self.snapshot();
            if check(&snapshot) {
                return Ok(snapshot);
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Box::new(snapshot));
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

/// A running fake client and game. Stops when dropped.
#[derive(Debug)]
pub struct MockLeague {
    driver: Arc<Driver>,
    token: String,
    root_pem: String,
    lcu: SocketAddr,
    live: SocketAddr,
    control: Option<SocketAddr>,
    tasks: Vec<JoinHandle<()>>,
}

impl Deref for MockLeague {
    type Target = Driver;
    fn deref(&self) -> &Driver {
        &self.driver
    }
}

impl MockLeague {
    pub async fn start(options: Options) -> Result<Self, String> {
        let pki = pki::TestPki::generate()?;
        let setup = &options.setup;
        let state = State {
            phase: setup.phase.clone(),
            summoner: setup.summoner_json(),
            champ_select: None,
            next_page_id: setup.rune_pages.iter().map(|p| p.id).max().unwrap_or(0) + 1,
            rune_pages: setup.rune_pages.clone(),
            owned_page_count: setup.owned_page_count,
            item_sets: setup
                .item_sets
                .clone()
                .map(|doc| [(setup.summoner_id, doc)].into_iter().collect())
                .unwrap_or_default(),
            my_selection: None,
            live: None,
            requests: Vec::new(),
        };
        let auth = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("riot:{}", options.token))
        );
        let shared = Arc::new(Shared::new(state, auth));
        let driver = Arc::new(Driver {
            shared: shared.clone(),
            riot_id: setup.riot_id(),
            summoner_id: setup.summoner_id,
        });

        let lcu_listener = bind(options.lcu_port).await?;
        let live_listener = bind(options.live_port).await?;
        let lcu = local_addr(&lcu_listener)?;
        let live = local_addr(&live_listener)?;
        let mut tasks = vec![
            tokio::spawn(serve::serve(
                lcu_listener,
                lcu::router(shared.clone()),
                Some(pki.server_config.clone()),
            )),
            tokio::spawn(serve::serve(
                live_listener,
                live::router(shared),
                Some(pki.server_config.clone()),
            )),
        ];
        let mut control = None;
        if let Some(port) = options.control_port {
            let listener = bind(port).await?;
            control = Some(local_addr(&listener)?);
            tasks.push(tokio::spawn(serve::serve(
                listener,
                control::router(driver.clone()),
                None,
            )));
        }
        Ok(Self {
            driver,
            token: options.token,
            root_pem: pki.root_pem,
            lcu,
            live,
            control,
            tasks,
        })
    }

    /// The `lockfile` contents the real client writes:
    /// `LeagueClient:<pid>:<port>:<password>:https`.
    pub fn lockfile(&self) -> String {
        format!(
            "LeagueClient:{}:{}:{}:https",
            std::process::id(),
            self.lcu.port(),
            self.token
        )
    }

    /// PEM of the root that signed the mock's certificates.
    pub fn root_pem(&self) -> &str {
        &self.root_pem
    }

    pub fn lcu_port(&self) -> u16 {
        self.lcu.port()
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn live_url(&self) -> String {
        format!("https://127.0.0.1:{}", self.live.port())
    }

    pub fn control_url(&self) -> Option<String> {
        self.control.map(|a| format!("http://{a}"))
    }
}

impl Drop for MockLeague {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

/// Listens on `127.0.0.1` only: the mock must never be reachable from the network.
async fn bind(port: u16) -> Result<TcpListener, String> {
    TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|e| format!("cannot listen on 127.0.0.1:{port}: {e}"))
}

fn local_addr(listener: &TcpListener) -> Result<SocketAddr, String> {
    listener.local_addr().map_err(|e| e.to_string())
}

/// A lockfile-style password. Not secret (it guards a local fake), but
/// different every run so a test cannot pass by guessing it.
fn random_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let state = std::collections::hash_map::RandomState::new();
    let mut a = state.build_hasher();
    a.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default(),
    );
    let mut b = state.build_hasher();
    b.write_u64(a.finish());
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode([a.finish().to_le_bytes(), b.finish().to_le_bytes()].concat())
}
