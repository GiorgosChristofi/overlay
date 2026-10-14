//! Riot's Live Client Data API: the running game's local, read-only HTTPS
//! endpoint (`https://127.0.0.1:2999`). It exists only while a game runs and
//! is signed by the same Riot root certificate as the LCU.

use async_trait::async_trait;
use domain::objectives::{KillKind, ObjectiveKill, Team};
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
    events: Events,
    game_data: GameData,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Events {
    #[serde(rename = "Events")]
    events: Vec<Event>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Event {
    #[serde(rename = "EventName")]
    name: String,
    /// Game time in seconds.
    #[serde(rename = "EventTime")]
    time: f64,
    /// `DragonKill` only: `Fire`, `Water`, ..., or `Elder`.
    #[serde(rename = "DragonType")]
    dragon_type: String,
    #[serde(rename = "KillerName")]
    killer_name: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct GameData {
    game_mode: String,
    /// 11 is Summoner's Rift; 0 when the field is missing.
    map_number: u32,
    /// Seconds since the game started.
    game_time: f64,
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
    riot_id_game_name: String,
    summoner_name: String,
    team: String,
    items: Vec<Item>,
}

impl Player {
    /// Event `KillerName`s have been the summoner name, the Riot ID, or its
    /// game name alone, depending on the client version.
    fn is_named(&self, name: &str) -> bool {
        let game_name = self.riot_id.split('#').next().unwrap_or_default();
        !name.is_empty()
            && [
                self.riot_id.as_str(),
                self.riot_id_game_name.as_str(),
                self.summoner_name.as_str(),
                game_name,
            ]
            .contains(&name)
    }
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
        game_time_ms: game_ms(data.game_data.game_time),
        summoners_rift: has_rift_objectives(&data.game_data),
        game_mode: data.game_data.game_mode.chars().take(32).collect(),
        objective_kills: objective_kills(&data.events.events, &data.all_players),
        event_count: u32::try_from(data.events.events.len()).unwrap_or(u32::MAX),
    }))
}

/// Summoner's Rift's map number in the Live Client Data API.
const SUMMONERS_RIFT_MAP: u32 = 11;
/// Modes on the Rift whose objectives follow the normal timings: every
/// queue (`CLASSIC`) and the Practice Tool. Others on the same map (URF,
/// Swiftplay, ...) change the timings, so they get no timers rather than
/// wrong ones.
const TIMED_MODES: [&str; 2] = ["CLASSIC", "PRACTICETOOL"];

fn has_rift_objectives(game: &GameData) -> bool {
    let on_rift = game.map_number == SUMMONERS_RIFT_MAP || game.map_number == 0;
    on_rift && TIMED_MODES.contains(&game.game_mode.as_str())
}

/// A game lasts at most a few hours; anything beyond this is not a game time.
const MAX_GAME_SECONDS: f64 = 24.0 * 3600.0;
/// More epic kills than any real game; a bound on what the API can make us keep.
const MAX_OBJECTIVE_KILLS: usize = 256;

/// Seconds (as the API sends them) to whole milliseconds, clamped to a game.
fn game_ms(seconds: f64) -> u64 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped to 0..=24 h
    let ms = (seconds.clamp(0.0, MAX_GAME_SECONDS) * 1000.0).round() as u64;
    ms
}

/// Epic kills since the game (re)started. A game lists one `GameStart`; if
/// a Practice Tool reset appends another, kills before it are the old game's.
fn objective_kills(events: &[Event], players: &[Player]) -> Vec<ObjectiveKill> {
    let start = events
        .iter()
        .rposition(|e| e.name == "GameStart")
        .unwrap_or(0);
    events[start..]
        .iter()
        .filter_map(|e| {
            let kind = match (e.name.as_str(), e.dragon_type.as_str()) {
                ("DragonKill", "Elder") => KillKind::Elder,
                ("DragonKill", _) => KillKind::Dragon,
                ("BaronKill", _) => KillKind::Baron,
                _ => return None,
            };
            let team = players
                .iter()
                .find(|p| p.is_named(&e.killer_name))
                .and_then(|p| Team::from_api(&p.team));
            Some(ObjectiveKill {
                at_ms: game_ms(e.time),
                kind,
                team,
            })
        })
        .take(MAX_OBJECTIVE_KILLS)
        .collect()
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
    fn reads_the_game_clock_and_epic_kills_with_the_killers_team() {
        let s = parse_all_game_data(FIXTURE).unwrap().unwrap();
        assert_eq!(s.game_time_ms, 612_400);
        assert!(s.summoners_rift);
        // Champion kills and grubs are not timed; the killer's game name
        // ("Enemy") is matched to their Riot ID ("Enemy#EUW").
        assert_eq!(
            s.objective_kills,
            [
                ObjectiveKill {
                    at_ms: 362_750,
                    kind: KillKind::Dragon,
                    team: Some(Team::Order)
                },
                ObjectiveKill {
                    at_ms: 605_500,
                    kind: KillKind::Dragon,
                    team: Some(Team::Chaos)
                },
            ]
        );
    }

    #[test]
    fn elder_and_baron_kills_are_told_apart_and_unknown_killers_have_no_team() {
        let raw = r#"{"activePlayer":{"riotId":"Me#1"},
            "allPlayers":[{"riotId":"Me#1","summonerName":"Me","team":"CHAOS","items":[]}],
            "events":{"Events":[
                {"EventName":"DragonKill","EventTime":1800,"DragonType":"Elder","KillerName":"Me"},
                {"EventName":"BaronKill","EventTime":1500.5,"KillerName":"Minion_T100L1S04N0003"}]},
            "gameData":{"gameMode":"ARAM","gameTime":1801}}"#;
        let s = parse_all_game_data(raw).unwrap().unwrap();
        assert!(!s.summoners_rift);
        assert_eq!(
            s.objective_kills,
            [
                ObjectiveKill {
                    at_ms: 1_800_000,
                    kind: KillKind::Elder,
                    team: Some(Team::Chaos)
                },
                ObjectiveKill {
                    at_ms: 1_500_500,
                    kind: KillKind::Baron,
                    team: None
                },
            ]
        );
    }

    #[test]
    fn the_practice_tool_is_timed_like_a_rift_game_and_other_modes_are_not() {
        let mode = |mode: &str, map: u32| {
            let raw = format!(
                r#"{{"activePlayer":{{"riotId":"Me#1"}},"allPlayers":[{{"riotId":"Me#1"}}],
                "gameData":{{"gameMode":"{mode}","mapNumber":{map},"gameTime":10}}}}"#
            );
            parse_all_game_data(&raw).unwrap().unwrap()
        };
        assert!(mode("CLASSIC", 11).summoners_rift);
        assert!(mode("PRACTICETOOL", 11).summoners_rift);
        assert_eq!(mode("PRACTICETOOL", 11).game_mode, "PRACTICETOOL");
        assert!(!mode("ARAM", 12).summoners_rift);
        assert!(!mode("URF", 11).summoners_rift, "different timings");
        assert!(!mode("CLASSIC", 30).summoners_rift, "another map");
    }

    #[test]
    fn kills_before_a_later_game_start_belong_to_the_old_game() {
        let raw = r#"{"activePlayer":{"riotId":"Me#1"},"allPlayers":[{"riotId":"Me#1"}],
            "events":{"Events":[
                {"EventName":"GameStart","EventTime":0},
                {"EventName":"DragonKill","EventTime":362,"DragonType":"Fire"},
                {"EventName":"GameStart","EventTime":0.1},
                {"EventName":"BaronKill","EventTime":20}]},
            "gameData":{"gameMode":"PRACTICETOOL","mapNumber":11,"gameTime":30}}"#;
        let s = parse_all_game_data(raw).unwrap().unwrap();
        assert_eq!(s.event_count, 4);
        assert_eq!(
            s.objective_kills,
            [ObjectiveKill {
                at_ms: 20_000,
                kind: KillKind::Baron,
                team: None
            }]
        );
    }

    #[test]
    fn untrusted_times_and_event_floods_are_bounded() {
        let flood: Vec<String> = (0..1000)
            .map(|i| format!(r#"{{"EventName":"BaronKill","EventTime":{i}}}"#))
            .collect();
        let raw = format!(
            r#"{{"activePlayer":{{"riotId":"Me#1"}},"allPlayers":[{{"riotId":"Me#1"}}],
            "events":{{"Events":[{{"EventName":"DragonKill","EventTime":-5}},{}]}},
            "gameData":{{"gameTime":1e300}}}}"#,
            flood.join(",")
        );
        let s = parse_all_game_data(&raw).unwrap().unwrap();
        assert_eq!(s.game_time_ms, 24 * 3600 * 1000, "clamped to a day");
        assert_eq!(s.objective_kills.len(), 256);
        assert_eq!(s.objective_kills[0].at_ms, 0, "negative times clamp to 0");
    }

    #[test]
    fn missing_player_is_no_snapshot_and_garbage_is_an_error() {
        let raw = r#"{"activePlayer":{"riotId":"Me#1"},"allPlayers":[]}"#;
        assert_eq!(parse_all_game_data(raw).unwrap(), None);
        assert!(parse_all_game_data("nope").is_err());
    }
}
