//! Port traits implemented by adapter crates. `app-core` depends only on
//! these, so every service can be tested with in-memory fakes.

use crate::builds::Build;
use crate::champ_select::{ChampSelectSession, ChampionPick};
use crate::error::PortError;
use crate::item_set::ItemSet;
use crate::objectives::ObjectiveKill;
use crate::phase::GameflowPhase;
use crate::runes::{ExistingPage, NewRunePage};
use crate::summoner::Summoner;
use async_trait::async_trait;
use std::fmt;
use std::sync::Arc;

/// Requests to the League Client API.
#[async_trait]
pub trait LcuPort: Send + Sync + fmt::Debug {
    async fn current_summoner(&self) -> Result<Summoner, PortError>;
    async fn gameflow_phase(&self) -> Result<GameflowPhase, PortError>;
    async fn champ_select_session(&self) -> Result<Option<ChampSelectSession>, PortError>;
    async fn rune_pages(&self) -> Result<Vec<ExistingPage>, PortError>;
    async fn owned_rune_page_count(&self) -> Result<u32, PortError>;
    async fn delete_rune_page(&self, id: i64) -> Result<(), PortError>;
    async fn create_rune_page(&self, page: &NewRunePage) -> Result<(), PortError>;
    async fn upsert_item_set(&self, summoner_id: i64, set: &ItemSet) -> Result<(), PortError>;
    async fn set_summoner_spells(&self, spells: [i64; 2]) -> Result<(), PortError>;
}

/// Events pushed by the LCU connection.
#[derive(Debug, Clone)]
pub enum LcuEvent {
    Connected(Arc<dyn LcuPort>),
    Disconnected,
    Phase(GameflowPhase),
    /// `None` when the session ends.
    ChampSelect(Option<ChampSelectSession>),
}

/// Where recommended builds come from (live statistics, the seed file, ...).
#[async_trait]
pub trait BuildSource: Send + Sync + fmt::Debug {
    /// Short name for logs and error messages, e.g. `OP.GG`.
    fn name(&self) -> &str;

    /// The build for a pick. `Ok(None)` means the source has no data for
    /// this champion and mode; `Err` means it could not be asked.
    async fn build(&self, pick: ChampionPick) -> Result<Option<Build>, PortError>;
}

/// What the running game says about the local player (Riot's Live Client
/// Data API, available only while a game is running).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LiveSnapshot {
    /// Current gold, rounded down.
    pub gold: u32,
    /// Items held right now: item id and count.
    pub inventory: Vec<(i64, u32)>,
    /// The game clock (`gameData.gameTime`) when this was read.
    pub game_time_ms: u64,
    /// A Summoner's Rift game with the normal objective timings (any queue
    /// or the Practice Tool); only these get objective timers.
    pub summoners_rift: bool,
    /// The API's `gameMode` (e.g. `CLASSIC`, `PRACTICETOOL`), for logs.
    pub game_mode: String,
    /// Dragons, Elders and Barons slain so far, by either team.
    pub objective_kills: Vec<ObjectiveKill>,
    /// How many events the game lists. It only grows during a game; a
    /// drop means the Practice Tool reset it.
    pub event_count: u32,
}

/// Reads the running game.
#[async_trait]
pub trait LiveGamePort: Send + Sync + fmt::Debug {
    /// `Ok(None)` while no game is running or it has not started yet.
    async fn snapshot(&self) -> Result<Option<LiveSnapshot>, PortError>;
}
