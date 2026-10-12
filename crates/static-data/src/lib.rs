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
/// Data Dragon's largest file (item.json) is about 1 MB; anything far bigger
/// is not Data Dragon.
const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;
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
            .redirect(same_host_redirects())
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
        let invalid = |reason: String| StaticDataError::Invalid {
            file: "versions.json".into(),
            reason,
        };
        let response = self
            .client
            .get(format!("{}/api/versions.json", self.base_url))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| StaticDataError::Network(e.to_string()))?;
        let body = read_capped(response, MAX_BODY_BYTES)
            .await
            .map_err(invalid)?;
        let versions: Vec<String> =
            serde_json::from_str(&body).map_err(|e| invalid(e.to_string()))?;
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
        // The version names a cache folder, so a hostile value such as `../..`
        // must never reach the file system.
        if !is_patch_version(version) {
            return Err(StaticDataError::Invalid {
                file: "versions.json".into(),
                reason: format!("not a patch version: {version:?}"),
            });
        }
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
        let response = self
            .client
            .get(url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| StaticDataError::Network(e.to_string()))?;
        let body =
            read_capped(response, MAX_BODY_BYTES)
                .await
                .map_err(|e| StaticDataError::Invalid {
                    file: file.into(),
                    reason: e,
                })?;
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

/// `16.19.1`: digits separated by single dots, nothing else.
fn is_patch_version(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 32
        && v.split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Follows redirects only within the host we asked, so a redirect cannot
/// send us to data from somewhere else.
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

pub fn newest_cached_version(cache_dir: &Path) -> Option<String> {
    std::fs::read_dir(cache_dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| is_patch_version(name))
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

    #[test]
    fn only_dotted_numbers_are_patch_versions() {
        for ok in ["16.19.1", "16.19", "1"] {
            assert!(is_patch_version(ok), "{ok}");
        }
        for bad in [
            "",
            "..",
            "../..",
            "16..1",
            "16.19.1/../x",
            "C:\\x",
            "/etc",
            "16.19.1 ",
            "lolpatch_7.20",
        ] {
            assert!(!is_patch_version(bad), "{bad}");
        }
    }
}
