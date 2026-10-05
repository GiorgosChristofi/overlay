//! REST calls to the LCU. Every endpoint path the app uses lives in this file.

use crate::lockfile::LockfileInfo;
use async_trait::async_trait;
use base64::Engine;
use domain::champ_select::ChampSelectSession;
use domain::item_set::ItemSet;
use domain::phase::GameflowPhase;
use domain::ports::LcuPort;
use domain::runes::{ExistingPage, NewRunePage};
use domain::summoner::Summoner;
use domain::PortError;
use reqwest::{Method, StatusCode};
use rustls::ClientConfig;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// LCU REST client. Implements [`LcuPort`].
#[derive(Clone)]
pub struct LcuHttp {
    client: reqwest::Client,
    base: String,
    auth: String,
}

impl fmt::Debug for LcuHttp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LcuHttp")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}

pub fn basic_auth(password: &str) -> String {
    let token = base64::engine::general_purpose::STANDARD.encode(format!("riot:{password}"));
    format!("Basic {token}")
}

impl LcuHttp {
    /// `tls` is required when the lockfile says `https`.
    pub fn new(info: &LockfileInfo, tls: Option<Arc<ClientConfig>>) -> Result<Self, PortError> {
        let mut builder = reqwest::Client::builder().timeout(REQUEST_TIMEOUT);
        if info.is_tls() {
            let config = tls.ok_or_else(|| {
                PortError::Transport("the Riot root certificate is required for https".into())
            })?;
            builder = builder.use_preconfigured_tls((*config).clone());
        }
        let client = builder
            .build()
            .map_err(|e| PortError::Transport(e.to_string()))?;
        Ok(Self {
            client,
            base: info.base_url(),
            auth: basic_auth(&info.password),
        })
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<reqwest::Response, PortError> {
        let mut req = self
            .client
            .request(method, format!("{}{path}", self.base))
            .header(reqwest::header::AUTHORIZATION, &self.auth)
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(body) = body {
            req = req.json(body);
        }
        let resp = req.send().await.map_err(|e| {
            if e.is_connect() {
                PortError::NotConnected
            } else {
                PortError::Transport(e.to_string())
            }
        })?;
        if resp.status().is_success() {
            Ok(resp)
        } else {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            Err(PortError::Http {
                status,
                body: truncate(&body, 300),
            })
        }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, PortError> {
        let resp = self.send(Method::GET, path, None).await?;
        resp.json()
            .await
            .map_err(|e| PortError::Decode(format!("{path}: {e}")))
    }

    async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<(), PortError> {
        self.send(method, path, body).await.map(|_| ())
    }
}

fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn encode<T: serde::Serialize>(value: &T) -> Result<Value, PortError> {
    serde_json::to_value(value).map_err(|e| PortError::Decode(e.to_string()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PerkInventory {
    #[serde(default)]
    owned_page_count: u32,
}

#[async_trait]
impl LcuPort for LcuHttp {
    async fn current_summoner(&self) -> Result<Summoner, PortError> {
        self.get("/lol-summoner/v1/current-summoner").await
    }

    async fn gameflow_phase(&self) -> Result<GameflowPhase, PortError> {
        self.get("/lol-gameflow/v1/gameflow-phase").await
    }

    async fn champ_select_session(&self) -> Result<Option<ChampSelectSession>, PortError> {
        match self.get("/lol-champ-select/v1/session").await {
            Ok(session) => Ok(Some(session)),
            Err(PortError::Http { status, .. }) if status == StatusCode::NOT_FOUND.as_u16() => {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    async fn rune_pages(&self) -> Result<Vec<ExistingPage>, PortError> {
        self.get("/lol-perks/v1/pages").await
    }

    async fn owned_rune_page_count(&self) -> Result<u32, PortError> {
        let inv: PerkInventory = self.get("/lol-perks/v1/inventory").await?;
        Ok(inv.owned_page_count)
    }

    async fn delete_rune_page(&self, id: i64) -> Result<(), PortError> {
        self.call(Method::DELETE, &format!("/lol-perks/v1/pages/{id}"), None)
            .await
    }

    async fn create_rune_page(&self, page: &NewRunePage) -> Result<(), PortError> {
        self.call(Method::POST, "/lol-perks/v1/pages", Some(&encode(page)?))
            .await
    }

    async fn upsert_item_set(&self, summoner_id: i64, set: &ItemSet) -> Result<(), PortError> {
        let path = format!("/lol-item-sets/v1/item-sets/{summoner_id}/sets");
        let existing: Value = self.get(&path).await?;
        let merged = merge_item_set(existing, set);
        self.call(Method::PUT, &path, Some(&merged)).await
    }

    async fn set_summoner_spells(&self, spells: [i64; 2]) -> Result<(), PortError> {
        let body = json!({ "spell1Id": spells[0], "spell2Id": spells[1] });
        self.call(
            Method::PATCH,
            "/lol-champ-select/v1/session/my-selection",
            Some(&body),
        )
        .await
    }
}

/// Converts our item set into the client's JSON format.
pub fn item_set_json(set: &ItemSet) -> Value {
    let blocks: Vec<Value> = set
        .blocks
        .iter()
        .map(|(title, items)| {
            json!({
                "type": title,
                "hideIfSummonerSpell": "",
                "showIfSummonerSpell": "",
                "items": items.iter().map(|id| json!({ "id": id.to_string(), "count": 1 })).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "uid": set.uid,
        "title": set.title,
        "type": "custom",
        "map": "any",
        "mode": "any",
        "priority": false,
        "sortrank": 0,
        "startedFrom": "blank",
        "associatedChampions": [set.champion_id],
        "associatedMaps": [],
        "preferredItemSlots": [],
        "blocks": blocks,
    })
}

/// Replaces our set (matched by uid) inside the client's full item-set
/// document and keeps everything else, including fields we do not know.
pub fn merge_item_set(mut existing: Value, set: &ItemSet) -> Value {
    if !existing.is_object() {
        existing = json!({});
    }
    let mut sets: Vec<Value> = existing
        .get("itemSets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.get("uid").and_then(Value::as_str) != Some(set.uid.as_str()))
        .collect();
    sets.push(item_set_json(set));
    if let Some(obj) = existing.as_object_mut() {
        obj.insert("itemSets".into(), Value::Array(sets));
    }
    existing
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set() -> ItemSet {
        ItemSet {
            uid: "lol-overlay-99".into(),
            title: "LoL Overlay: Lux Middle".into(),
            champion_id: 99,
            blocks: vec![("Start".into(), vec![1056, 2003])],
        }
    }

    #[test]
    fn merge_replaces_only_our_set_and_keeps_unknown_fields() {
        let existing = json!({
            "accountId": 1,
            "timestamp": 5,
            "itemSets": [
                { "uid": "user-made", "title": "Mine" },
                { "uid": "lol-overlay-99", "title": "old" }
            ]
        });
        let merged = merge_item_set(existing, &set());
        let sets = merged["itemSets"].as_array().unwrap();
        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0]["uid"], "user-made");
        assert_eq!(sets[1]["title"], "LoL Overlay: Lux Middle");
        assert_eq!(sets[1]["blocks"][0]["items"][0]["id"], "1056");
        assert_eq!(merged["accountId"], 1);
    }

    #[test]
    fn merge_handles_empty_document() {
        let merged = merge_item_set(Value::Null, &set());
        assert_eq!(merged["itemSets"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn basic_auth_uses_riot_user() {
        // base64("riot:pw")
        assert_eq!(basic_auth("pw"), "Basic cmlvdDpwdw==");
    }
}
