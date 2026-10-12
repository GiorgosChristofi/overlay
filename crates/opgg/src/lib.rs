//! Builds from OP.GG's champion statistics: rune pages, summoner spells and
//! items for a champion in a role (or ARAM).
//!
//! This is the JSON API behind the op.gg website. It is not documented or
//! versioned by OP.GG, so everything that depends on its shape lives in
//! [`parse`], and a failure here falls back to the seed builds.

mod parse;

pub use parse::{parse, position, role_from_position, Parsed, MAX_RUNE_PAGES};

use async_trait::async_trait;
use domain::builds::Build;
use domain::champ_select::{ChampionPick, GameMode, Role};
use domain::ports::BuildSource;
use domain::PortError;
use reqwest::StatusCode;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const OPGG_API_URL: &str = "https://lol-api-champion.op.gg";
/// OP.GG's region code for statistics from every server combined.
pub const GLOBAL_REGION: &str = "global";

const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
/// OP.GG refreshes statistics a few times a day; a champ select only needs
/// them to stay stable while the player looks at them.
const CACHE_TTL: Duration = Duration::from_secs(30 * 60);
/// Rank bracket the statistics come from.
const TIER: &str = "emerald_plus";
const TIER_LABEL: &str = "Emerald+";
/// Asked first when the player has no assigned role; the response tells us
/// the champion's real main role.
const ANY_ROLE_FIRST_GUESS: Role = Role::Middle;

type CacheKey = (i64, GameMode, Role);

#[derive(Debug)]
pub struct OpggClient {
    client: reqwest::Client,
    base_url: String,
    /// OP.GG region code, e.g. `EUNE`.
    region: String,
    cache: Mutex<HashMap<CacheKey, (Instant, Option<Build>)>>,
}

impl OpggClient {
    pub fn new(base_url: impl Into<String>, region: impl Into<String>) -> Result<Self, PortError> {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(concat!("lol-overlay/", env!("CARGO_PKG_VERSION")))
            .redirect(same_host_redirects())
            .build()
            .map_err(|e| PortError::Transport(e.to_string()))?;
        Ok(Self {
            client,
            base_url: base_url.into(),
            region: region.into(),
            cache: Mutex::new(HashMap::new()),
        })
    }

    fn cached(&self, key: &CacheKey) -> Option<Option<Build>> {
        let cache = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache
            .get(key)
            .filter(|(at, _)| at.elapsed() < CACHE_TTL)
            .map(|(_, build)| build.clone())
    }

    fn remember(&self, key: CacheKey, build: Option<Build>) {
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key, (Instant::now(), build));
    }

    /// GETs one champion page. `Ok(None)` when OP.GG has no data for it.
    async fn get(&self, path: &str) -> Result<Option<String>, PortError> {
        let url = format!("{}/api/{}/champions/{path}", self.base_url, self.region);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| PortError::Transport(e.to_string()))?;
        let status = response.status();
        // OP.GG answers 404 for unknown champions and 422 for positions it
        // has no statistics for.
        if status == StatusCode::NOT_FOUND || status == StatusCode::UNPROCESSABLE_ENTITY {
            tracing::info!(%url, %status, "OP.GG has no data");
            return Ok(None);
        }
        let body = read_capped(response, MAX_BODY_BYTES)
            .await
            .map_err(PortError::Transport)?;
        if !status.is_success() {
            return Err(PortError::Http {
                status: status.as_u16(),
                body: body.chars().take(200).collect(),
            });
        }
        Ok(Some(body))
    }

    async fn fetch(
        &self,
        path: &str,
        champion_id: i64,
        role: Role,
        source: &str,
    ) -> Result<Parsed, PortError> {
        match self.get(path).await? {
            Some(body) => parse(&body, champion_id, role, source)
                .map_err(|e| PortError::Decode(format!("OP.GG response: {e}"))),
            None => Ok(Parsed {
                build: None,
                most_played: None,
            }),
        }
    }

    /// How the region reads in the UI, e.g. `OP.GG all regions`.
    fn source_prefix(&self) -> String {
        if self.region.eq_ignore_ascii_case(GLOBAL_REGION) {
            "OP.GG all regions".into()
        } else {
            format!("OP.GG {}", self.region)
        }
    }

    async fn ranked(&self, champion_id: i64, role: Role) -> Result<Parsed, PortError> {
        let pos = position(role).unwrap_or("MID");
        let source = format!("{} · {TIER_LABEL}", self.source_prefix());
        self.fetch(
            &format!("ranked/{champion_id}/{pos}?tier={TIER}"),
            champion_id,
            role,
            &source,
        )
        .await
    }

    async fn fetch_build(&self, pick: ChampionPick) -> Result<Option<Build>, PortError> {
        let id = pick.champion_id;
        match pick.mode {
            GameMode::Aram => {
                let source = format!("{} · ARAM", self.source_prefix());
                let parsed = self
                    .fetch(&format!("aram/{id}/none"), id, Role::Any, &source)
                    .await?;
                Ok(parsed.build)
            }
            GameMode::SummonersRift if pick.role != Role::Any => {
                Ok(self.ranked(id, pick.role).await?.build)
            }
            GameMode::SummonersRift => {
                // No assigned role (blind pick): use the champion's main role.
                let guess = self.ranked(id, ANY_ROLE_FIRST_GUESS).await?;
                match guess.most_played {
                    Some(main) if main != ANY_ROLE_FIRST_GUESS => {
                        Ok(self.ranked(id, main).await?.build)
                    }
                    _ => Ok(guess.build),
                }
            }
        }
    }
}

#[async_trait]
impl BuildSource for OpggClient {
    fn name(&self) -> &str {
        "OP.GG"
    }

    async fn build(&self, pick: ChampionPick) -> Result<Option<Build>, PortError> {
        let key = (pick.champion_id, pick.mode, pick.role);
        if let Some(hit) = self.cached(&key) {
            return Ok(hit);
        }
        let started = Instant::now();
        let build = self.fetch_build(pick).await?;
        tracing::info!(
            champion = pick.champion_id,
            role = ?pick.role,
            mode = ?pick.mode,
            found = build.is_some(),
            ms = started.elapsed().as_millis() as u64,
            "OP.GG build fetched"
        );
        self.remember(key, build.clone());
        Ok(build)
    }
}

/// OP.GG champion pages are a few hundred KB; this bounds a misbehaving server.
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// Follows redirects only within the host we asked, so a redirect cannot
/// send us to builds from somewhere else.
fn same_host_redirects() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        let same_host = attempt
            .previous()
            .first()
            .is_some_and(|first| first.host_str() == attempt.url().host_str());
        if same_host && attempt.previous().len() < 5 {
            attempt.follow()
        } else {
            attempt.stop()
        }
    })
}

/// Reads a body as UTF-8 text, giving up once it passes `max` bytes.
async fn read_capped(mut response: reqwest::Response, max: usize) -> Result<String, String> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if body.len() + chunk.len() > max {
            return Err(format!("response is larger than {max} bytes"));
        }
        body.extend_from_slice(&chunk);
    }
    String::from_utf8(body).map_err(|e| e.to_string())
}
