use domain::settings::Settings;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("could not save settings: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not encode settings: {0}")]
    Encode(#[from] serde_json::Error),
}

/// Settings persisted as JSON. Writes are atomic (temp file + rename).
#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads settings. A missing file gives defaults; a corrupt file is
    /// moved aside to `.corrupt` so the user's data is never silently lost.
    pub fn load(&self) -> Settings {
        let Ok(raw) = std::fs::read_to_string(&self.path) else {
            return Settings::default();
        };
        match serde_json::from_str(&raw) {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!(error = %e, "settings file is corrupt; using defaults");
                let backup = self.path.with_extension("json.corrupt");
                if let Err(e) = std::fs::rename(&self.path, &backup) {
                    tracing::warn!(error = %e, "could not move corrupt settings aside");
                }
                Settings::default()
            }
        }
    }

    pub fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(settings)?)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::spells::FlashKey;

    #[test]
    fn round_trips_and_defaults_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("nested/settings.json"));
        assert_eq!(store.load(), Settings::default());
        let s = Settings {
            flash_key: FlashKey::F,
            ..Settings::default()
        };
        store.save(&s).unwrap();
        assert_eq!(store.load(), s);
    }

    #[test]
    fn corrupt_file_is_kept_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(SettingsStore::new(&path).load(), Settings::default());
        assert!(dir.path().join("settings.json.corrupt").exists());
    }
}
