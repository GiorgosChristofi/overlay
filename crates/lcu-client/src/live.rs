//! Riot's Live Client Data API: the running game's local, read-only HTTPS
//! endpoint (`https://127.0.0.1:2999`). It exists only while a game runs and
//! is signed by the same Riot root certificate as the LCU.

use async_trait::async_trait;
use domain::ports::{LiveGamePort, LiveSnapshot};
use domain::PortError;
use reqwest::StatusCode;
use rustls::ClientConfig;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

pub const LIVE_CLIENT_URL: &str = "https://127.0.0.1:2999";
const PATH: &str = "/liveclientdata/allgamedata";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
/// Windows retries a refused local connection for about 2 s; give up sooner
/// so "no game running" is answered quickly and reads as a connect error.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Clone)]
pub struct LiveClient {
    client: reqwest::Client,
    base: String,
}

impl LiveClient {
    /// `tls` is required for the real `https` endpoint; tests pass `None`
    /// with an `http` base URL.
    pub fn new(base: impl Into<String>, tls: Option<Arc<ClientConfig>>) -> Result<Self, PortError> {
        let base = base.into();
        let mut builder = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT);
        if base.starts_with("https://") {
            let config = tls.ok_or_else(|| {
                PortError::Transport("the Riot root certificate is required for https".into())
            })?;
            builder = builder.use_preconfigured_tls((*config).clone());
        }
        let client = builder
            .build()
            .map_err(|e| PortError::Transport(e.to_string()))?;
        Ok(Self { client, base })
    }
}

#[async_trait]
impl LiveGamePort for LiveClient {
    async fn snapshot(&self) -> Result<Option<LiveSnapshot>, PortError> {
        let response = match self.client.get(format!("{}{PATH}", self.base)).send().await {
            Ok(r) => r,
            // Nothing listens on 2999 outside a game.
            Err(e) if e.is_connect() => return Ok(None),
            Err(e) => return Err(PortError::Transport(e.to_string())),
        };
        let status = response.status();
        // The game answers 404 during the loading screen.
        if status == StatusCode::NOT_FOUND || status == StatusCode::SERVICE_UNAVAILABLE {
            return Ok(None);
        }
        let body = response
            .text()
            .await
            .map_err(|e| PortError::Transport(e.to_string()))?;
        if !status.is_success() {
            return Err(PortError::Http {
                status: status.as_u16(),
                body: body.chars().take(200).collect(),
            });
        }
        parse_all_game_data(&body)
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct AllGameData {
    active_player: ActivePlayer,
    all_players: Vec<Player>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ActivePlayer {
    current_gold: f64,
    riot_id: String,
    summoner_name: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Player {
    riot_id: String,
    summoner_name: String,
    items: Vec<Item>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Item {
    #[serde(rename = "itemID")]
    item_id: i64,
    count: u32,
}

/// Finds the local player in `allgamedata`. `Ok(None)` when they are not
/// listed (spectating, or the game is still starting).
pub fn parse_all_game_data(raw: &str) -> Result<Option<LiveSnapshot>, PortError> {
    let data: AllGameData = serde_json::from_str(raw)
        .map_err(|e| PortError::Decode(format!("live client data: {e}")))?;
    let me = &data.active_player;
    let is_me = |p: &&Player| {
        (!me.riot_id.is_empty() && p.riot_id == me.riot_id)
            || (!me.summoner_name.is_empty() && p.summoner_name == me.summoner_name)
    };
    let Some(player) = data.all_players.iter().find(is_me) else {
        return Ok(None);
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // gold is small and positive
    let gold = me.current_gold.max(0.0).floor() as u32;
    Ok(Some(LiveSnapshot {
        gold,
        inventory: player
            .items
            .iter()
            .filter(|i| i.item_id > 0)
            .map(|i| (i.item_id, i.count.max(1)))
            .collect(),
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    const FIXTURE: &str = include_str!("../../../fixtures/synthetic/live-allgamedata.json");

    #[test]
    fn finds_the_local_player_among_all_players() {
        let s = parse_all_game_data(FIXTURE).unwrap().unwrap();
        assert_eq!(s.gold, 1337);
        assert_eq!(s.inventory, [(3802, 1), (2003, 2), (3340, 1)]);
    }

    #[test]
    fn falls_back_to_the_summoner_name() {
        let raw = r#"{"activePlayer":{"currentGold":5,"summonerName":"Old"},"allPlayers":[{"summonerName":"Old","items":[{"itemID":1001,"count":1}]}]}"#;
        let s = parse_all_game_data(raw).unwrap().unwrap();
        assert_eq!(s.inventory, [(1001, 1)]);
    }

    #[test]
    fn missing_player_is_no_snapshot_and_garbage_is_an_error() {
        let raw = r#"{"activePlayer":{"riotId":"Me#1"},"allPlayers":[]}"#;
        assert_eq!(parse_all_game_data(raw).unwrap(), None);
        assert!(parse_all_game_data("nope").is_err());
    }
}
