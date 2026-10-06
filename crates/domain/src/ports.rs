//! Port traits implemented by adapter crates. `app-core` depends only on
//! these, so every service can be tested with in-memory fakes.

use crate::builds::Build;
use crate::champ_select::{ChampSelectSession, ChampionPick};
use crate::error::PortError;
use crate::item_set::ItemSet;
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
