//! What the fake client and game know, and everything the app asked them.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};
use tokio::sync::broadcast;

/// A rune page as `GET /lol-perks/v1/pages` returns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RunePage {
    pub id: i64,
    pub name: String,
    pub is_editable: bool,
    pub is_deletable: bool,
    pub current: bool,
    pub primary_style_id: i64,
    pub sub_style_id: i64,
    pub selected_perk_ids: Vec<i64>,
}

impl Default for RunePage {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            is_editable: true,
            is_deletable: true,
            current: false,
            primary_style_id: 8000,
            sub_style_id: 8200,
            selected_perk_ids: Vec::new(),
        }
    }
}

/// One request the app made, in order.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recorded {
    /// `lcu` or `live`.
    pub server: &'static str,
    pub method: String,
    pub path: String,
    pub status: u16,
    /// False when the request lacked the right `riot:<token>` credentials.
    pub authorized: bool,
    /// False when the path is not one the mock implements, i.e. something
    /// the app is not expected to call.
    pub known: bool,
    pub body: Option<Value>,
}

/// The observable state, for tests and the control API.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub phase: String,
    pub champ_select: Option<Value>,
    pub rune_pages: Vec<RunePage>,
    pub owned_page_count: u32,
    /// Item-set documents by summoner id.
    pub item_sets: BTreeMap<i64, Value>,
    /// Last summoner spells the app chose, `[spell1, spell2]`.
    pub my_selection: Option<[i64; 2]>,
    pub live: Option<Value>,
    pub requests: Vec<Recorded>,
}

impl Snapshot {
    /// Requests that changed something in the client.
    pub fn writes(&self) -> Vec<&Recorded> {
        self.requests
            .iter()
            .filter(|r| r.server == "lcu" && r.method != "GET")
            .collect()
    }

    /// Our item set (`uid` starting `lol-overlay`) for a summoner, if any.
    pub fn overlay_item_set(&self, summoner_id: i64) -> Option<&Value> {
        self.item_sets
            .get(&summoner_id)?
            .get("itemSets")?
            .as_array()?
            .iter()
            .find(|s| {
                s.get("uid")
                    .and_then(Value::as_str)
                    .is_some_and(|u| u.starts_with("lol-overlay"))
            })
    }
}

#[derive(Debug)]
pub(crate) struct State {
    pub phase: String,
    pub summoner: Value,
    pub champ_select: Option<Value>,
    pub rune_pages: Vec<RunePage>,
    pub owned_page_count: u32,
    pub next_page_id: i64,
    pub item_sets: BTreeMap<i64, Value>,
    pub my_selection: Option<[i64; 2]>,
    pub live: Option<Value>,
    pub requests: Vec<Recorded>,
}

/// The app polls the game every second; keep a long manual run bounded.
const MAX_RECORDED: usize = 10_000;

impl State {
    pub fn record(&mut self, request: Recorded) {
        if self.requests.len() >= MAX_RECORDED {
            self.requests.drain(..MAX_RECORDED / 2);
        }
        self.requests.push(request);
    }

    pub fn summoner_id(&self) -> i64 {
        self.summoner
            .get("summonerId")
            .and_then(Value::as_i64)
            .unwrap_or_default()
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            phase: self.phase.clone(),
            champ_select: self.champ_select.clone(),
            rune_pages: self.rune_pages.clone(),
            owned_page_count: self.owned_page_count,
            item_sets: self.item_sets.clone(),
            my_selection: self.my_selection,
            live: self.live.clone(),
            requests: self.requests.clone(),
        }
    }
}

/// A WAMP event for subscribed sockets: `[8, topic, {uri, eventType, data}]`.
#[derive(Debug, Clone)]
pub(crate) enum Push {
    Event {
        topic: &'static str,
        uri: &'static str,
        event_type: &'static str,
        data: Value,
    },
    /// Close every socket, as the client does when it restarts.
    CloseAll,
}

#[derive(Debug)]
pub(crate) struct Shared {
    state: Mutex<State>,
    pub push: broadcast::Sender<Push>,
    pub auth: String,
}

pub(crate) const TOPIC_PHASE: &str = "OnJsonApiEvent_lol-gameflow_v1_gameflow-phase";
pub(crate) const TOPIC_CHAMP_SELECT: &str = "OnJsonApiEvent_lol-champ-select_v1_session";
pub(crate) const URI_PHASE: &str = "/lol-gameflow/v1/gameflow-phase";
pub(crate) const URI_CHAMP_SELECT: &str = "/lol-champ-select/v1/session";

impl Shared {
    pub fn new(state: State, auth: String) -> Self {
        let (push, _) = broadcast::channel(256);
        Self {
            state: Mutex::new(state),
            push,
            auth,
        }
    }

    pub fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn send(&self, push: Push) {
        // No subscriber yet is fine: the app reads the current state on connect.
        let _ = self.push.send(push);
    }

    pub fn set_phase(&self, phase: &str) {
        self.state().phase = phase.to_owned();
        self.send(Push::Event {
            topic: TOPIC_PHASE,
            uri: URI_PHASE,
            event_type: "Update",
            data: json!(phase),
        });
    }

    pub fn set_champ_select(&self, session: Option<Value>) {
        let event_type = {
            let mut state = self.state();
            let event_type = match (&state.champ_select, &session) {
                (_, None) => "Delete",
                (None, Some(_)) => "Create",
                (Some(_), Some(_)) => "Update",
            };
            state.champ_select.clone_from(&session);
            event_type
        };
        self.send(Push::Event {
            topic: TOPIC_CHAMP_SELECT,
            uri: URI_CHAMP_SELECT,
            event_type,
            data: session.unwrap_or(Value::Null),
        });
    }
}
