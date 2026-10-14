//! Scripted games: a starting state and a list of steps, written as JSON so
//! the same script drives automated tests and manual runs of the real app.
//!
//! ```json
//! { "name": "ranked mid Lux",
//!   "setup": { "ownedPageCount": 2 },
//!   "steps": [
//!     { "phase": "ChampSelect" },
//!     { "champSelect": { "champion": 99, "position": "middle" } },
//!     { "wait": 1500 },
//!     { "champSelect": { "champion": 99, "position": "middle", "locked": true } },
//!     { "phase": "InProgress" },
//!     { "live": { "gold": 500 } },
//!     { "live": { "gold": 75, "items": [[1056, 1], [2003, 2]] } },
//!     { "phase": "EndOfGame" } ] }
//! ```

use crate::state::RunePage;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Scenario {
    pub name: String,
    pub description: String,
    pub setup: Setup,
    pub steps: Vec<Step>,
}

impl Scenario {
    pub fn from_json(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|e| format!("invalid scenario: {e}"))
    }
}

/// The client's state before the first step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Setup {
    pub game_name: String,
    pub tag_line: String,
    pub summoner_id: i64,
    pub phase: String,
    pub rune_pages: Vec<RunePage>,
    pub owned_page_count: u32,
    /// An existing item-set document for the summoner (other tools' sets).
    pub item_sets: Option<Value>,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            game_name: "Tester".into(),
            tag_line: "EUW".into(),
            summoner_id: 1234,
            phase: "None".into(),
            rune_pages: vec![
                RunePage {
                    id: 1,
                    name: "My page".into(),
                    current: true,
                    selected_perk_ids: vec![8010, 9111, 9104, 8299, 8226, 8237, 5005, 5008, 5011],
                    ..RunePage::default()
                },
                // A Riot preset: listed but neither editable nor deletable.
                RunePage {
                    id: 50,
                    name: "Domination".into(),
                    is_editable: false,
                    is_deletable: false,
                    primary_style_id: 8100,
                    sub_style_id: 8300,
                    ..RunePage::default()
                },
            ],
            owned_page_count: 2,
            item_sets: None,
        }
    }
}

impl Setup {
    pub(crate) fn summoner_json(&self) -> Value {
        json!({
            "gameName": self.game_name,
            "tagLine": self.tag_line,
            "displayName": self.game_name,
            "summonerLevel": 187,
            "profileIconId": 29,
            "puuid": "mock-puuid",
            "summonerId": self.summoner_id,
            "accountId": self.summoner_id,
        })
    }

    pub(crate) fn riot_id(&self) -> String {
        format!("{}#{}", self.game_name, self.tag_line)
    }
}

/// One thing that happens. Steps apply in order; `wait` pauses between them.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Step {
    /// Gameflow phase, e.g. `Lobby`, `ChampSelect`, `InProgress`, `EndOfGame`.
    Phase(String),
    /// A champ-select session built from a few fields.
    ChampSelect(ChampSelectStep),
    /// An exact champ-select session payload (e.g. a recording).
    ChampSelectRaw(Value),
    /// Champ select ends (dodge, or the game starts).
    EndChampSelect,
    /// In-game data built from a few fields.
    Live(LiveStep),
    /// An exact `allgamedata` payload.
    LiveRaw(Value),
    /// The game has no data: loading screen, or no game at all.
    NoGame,
    /// Pause, in milliseconds.
    Wait(u64),
    /// Drops every WebSocket, as when the client restarts.
    DropConnections,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChampSelectStep {
    pub champion: i64,
    /// `top`, `jungle`, `middle`, `bottom`, `utility`, or empty (blind, ARAM).
    pub position: String,
    /// Locked in (pick completed) rather than hovered.
    pub locked: bool,
    /// ARAM: no pick actions, a bench, the champion is assigned.
    pub aram: bool,
    pub spells: [i64; 2],
}

impl Default for ChampSelectStep {
    fn default() -> Self {
        Self {
            champion: 0,
            position: String::new(),
            locked: false,
            aram: false,
            spells: [4, 14],
        }
    }
}

/// The local player's cell in generated sessions.
const MY_CELL: i64 = 2;

impl ChampSelectStep {
    pub fn session(&self, summoner_id: i64) -> Value {
        let me = json!({
            "cellId": MY_CELL,
            "assignedPosition": self.position,
            "championId": if self.aram || self.locked { self.champion } else { 0 },
            "championPickIntent": 0,
            "spell1Id": self.spells[0],
            "spell2Id": self.spells[1],
            "summonerId": summoner_id,
            "team": 1,
        });
        let ally = json!({ "cellId": 0, "assignedPosition": "top", "championId": 86, "summonerId": 0, "team": 1 });
        let actions = if self.aram {
            json!([])
        } else {
            json!([
                [{ "id": 1, "actorCellId": 0, "championId": 86, "completed": true, "isAllyAction": true, "isInProgress": false, "type": "pick" }],
                [{ "id": 2, "actorCellId": MY_CELL, "championId": self.champion, "completed": self.locked, "isAllyAction": true, "isInProgress": !self.locked, "type": "pick" }]
            ])
        };
        json!({
            "localPlayerCellId": MY_CELL,
            "benchEnabled": self.aram,
            "benchChampions": [],
            "myTeam": [ally, me],
            "theirTeam": [],
            "actions": actions,
            "timer": { "phase": if self.locked { "FINALIZATION" } else { "BAN_PICK" } },
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LiveStep {
    pub gold: f64,
    /// `[item id, count]` in inventory order.
    pub items: Vec<(i64, u32)>,
    pub champion_name: String,
    pub game_time: f64,
    pub level: u32,
    /// `ARAM` or `CLASSIC`.
    pub game_mode: String,
    /// Game events after `GameStart`, as the API lists them, e.g.
    /// `{ "EventName": "DragonKill", "EventTime": 362.4, "DragonType": "Fire",
    /// "KillerName": "Tester" }`. Every step lists all events so far.
    pub events: Vec<Value>,
}

impl Default for LiveStep {
    fn default() -> Self {
        Self {
            gold: 500.0,
            items: Vec::new(),
            champion_name: "Lux".into(),
            game_time: 0.0,
            level: 1,
            game_mode: "CLASSIC".into(),
            events: Vec::new(),
        }
    }
}

impl LiveStep {
    pub fn all_game_data(&self, riot_id: &str) -> Value {
        let items: Vec<Value> = self
            .items
            .iter()
            .enumerate()
            .map(|(slot, (id, count))| json!({ "itemID": id, "count": count, "slot": slot, "price": 0, "displayName": "" }))
            .collect();
        let events: Vec<Value> =
            std::iter::once(json!({ "EventName": "GameStart", "EventTime": 0.0 }))
                .chain(self.events.iter().cloned())
                .enumerate()
                .map(|(id, mut event)| {
                    if let Some(fields) = event.as_object_mut() {
                        fields.insert("EventID".into(), json!(id));
                    }
                    event
                })
                .collect();
        json!({
            "activePlayer": {
                "currentGold": self.gold,
                "level": self.level,
                "riotId": riot_id,
                "summonerName": riot_id,
                "championStats": {},
            },
            "allPlayers": [
                { "championName": self.champion_name, "riotId": riot_id, "summonerName": riot_id, "team": "ORDER", "level": self.level, "items": items },
                { "championName": "Garen", "riotId": "Enemy#EUW", "summonerName": "Enemy#EUW", "team": "CHAOS", "level": 1,
                  "items": [{ "itemID": 1055, "count": 1, "slot": 0 }] },
            ],
            "events": { "Events": events },
            "gameData": { "gameMode": self.game_mode, "gameTime": self.game_time },
        })
    }
}
