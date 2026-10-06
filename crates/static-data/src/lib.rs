//! Static game data from Data Dragon.
//!
//! On startup the app asks for the latest version, downloads that patch's
//! data once and caches it on disk. If the network is down it falls back to
//! the newest cached patch, so the app still works offline.

mod parse;

pub use parse::{parse_catalog, RawFiles, STAT_SHARD_ROWS};

use domain::static_catalog::StaticCatalog;
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

pub const DATA_DRAGON_URL: &str = "https://ddragon.leagueoflegends.com";
const FILES: [&str; 4] = [
    "champion.json",
    "summoner.json",
    "runesReforged.json",
    "item.json",
];

#[derive(Debug, Error)]
pub enum StaticDataError {
    #[error("network error: {0}")]
    Network(String),
    #[error("data for {file} is invalid: {reason}")]
    Invalid { file: String, reason: String },
    #[error("cache error: {0}")]
    Cache(#[from] std::io::Error),
    #[error("no network and no cached data yet")]
    NothingAvailable,
}

#[derive(Debug, Clone)]
pub struct DataDragon {
    client: reqwest::Client,
    base_url: String,
    cache_dir: PathBuf,
    locale: String,
}

impl DataDragon {
    pub fn new(
        base_url: impl Into<String>,
        cache_dir: impl Into<PathBuf>,
        locale: impl Into<String>,
    ) -> Result<Self, StaticDataError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| StaticDataError::Network(e.to_string()))?;
        Ok(Self {
            client,
            base_url: base_url.into(),
            cache_dir: cache_dir.into(),
            locale: locale.into(),
        })
    }

    /// Latest patch from the network, or the newest cached patch when offline.
    pub async fn load(&self) -> Result<StaticCatalog, StaticDataError> {
        match self.latest_version().await {
            Ok(version) => match self.load_version(&version).await {
                Ok(catalog) => return Ok(catalog),
                Err(e) => {
                    tracing::warn!(%version, error = %e, "could not load latest patch; trying cache")
                }
            },
            Err(e) => tracing::warn!(error = %e, "could not reach Data Dragon; trying cache"),
        }
        let version =
            newest_cached_version(&self.cache_dir).ok_or(StaticDataError::NothingAvailable)?;
        tracing::info!(%version, "using cached static data");
        self.load_version(&version).await
    }

    pub async fn latest_version(&self) -> Result<String, StaticDataError> {
        let versions: Vec<String> = self
            .client
            .get(format!("{}/api/versions.json", self.base_url))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| StaticDataError::Network(e.to_string()))?
            .json()
            .await
            .map_err(|e| StaticDataError::Invalid {
                file: "versions.json".into(),
                reason: e.to_string(),
            })?;
        versions
            .into_iter()
            .next()
            .ok_or_else(|| StaticDataError::Invalid {
                file: "versions.json".into(),
                reason: "empty list".into(),
            })
    }

    /// Loads one patch, from cache if present, otherwise downloading it.
    pub async fn load_version(&self, version: &str) -> Result<StaticCatalog, StaticDataError> {
        let dir = self.cache_dir.join(version).join(&self.locale);
        let mut contents = Vec::with_capacity(FILES.len());
        for file in FILES {
            contents.push(self.cached_or_download(version, &dir, file).await?);
        }
        let [champions, summoners, runes, items] =
            <[String; 4]>::try_from(contents).map_err(|_| StaticDataError::Invalid {
                file: "all".into(),
                reason: "missing file".into(),
            })?;
        let mut catalog = parse_catalog(
            version,
            &RawFiles {
                champions: &champions,
                summoners: &summoners,
                runes: &runes,
                items: &items,
            },
        )?;
        catalog.image_base_url = format!("{}/cdn/{version}/img", self.base_url);
        catalog.rune_image_base_url = format!("{}/cdn/img", self.base_url);
        Ok(catalog)
    }

    async fn cached_or_download(
        &self,
        version: &str,
        dir: &Path,
        file: &str,
    ) -> Result<String, StaticDataError> {
        let path = dir.join(file);
        if let Ok(cached) = tokio::fs::read_to_string(&path).await {
            return Ok(cached);
        }
        let url = format!(
            "{}/cdn/{version}/data/{}/{file}",
            self.base_url, self.locale
        );
        let body = self
            .client
            .get(url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| StaticDataError::Network(e.to_string()))?
            .text()
            .await
            .map_err(|e| StaticDataError::Network(e.to_string()))?;
        // Validate before caching so a broken download is never persisted.
        serde_json::from_str::<serde_json::Value>(&body).map_err(|e| StaticDataError::Invalid {
            file: file.into(),
            reason: e.to_string(),
        })?;
        tokio::fs::create_dir_all(dir).await?;
        let tmp = path.with_extension("json.tmp");
        tokio::fs::write(&tmp, &body).await?;
        tokio::fs::rename(&tmp, &path).await?;
        Ok(body)
    }
}

/// Orders `16.19.1` style versions numerically, not as strings.
fn version_key(v: &str) -> Vec<u64> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

pub fn newest_cached_version(cache_dir: &Path) -> Option<String> {
    std::fs::read_dir(cache_dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| name.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .max_by_key(|v| version_key(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_version_is_numeric_not_lexical() {
        let dir = tempfile::tempdir().unwrap();
        for v in ["16.9.1", "16.19.1", "16.10.2", "notes"] {
            std::fs::create_dir_all(dir.path().join(v)).unwrap();
        }
        assert_eq!(
            newest_cached_version(dir.path()).as_deref(),
            Some("16.19.1")
        );
    }
}
