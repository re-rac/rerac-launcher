//! Launcher settings, persisted as JSON at `<data root>/settings/launcher.json`.

use crate::versions::{SourceId, VersionRef};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const SETTINGS_SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema: u32,
    /// The version whose runtime and extractor are used.
    pub active_version: Option<VersionRef>,
    pub official: OfficialSettings,
    /// Local build folders added in Version Management → Development.
    pub dev_versions: Vec<DevVersionEntry>,
    /// Pass `--ntsc-only` to the extractor (skips PAL-only movies and scenes).
    pub ntsc_only: bool,
    /// Minimise the launcher window while the game runs (restored when it exits).
    pub minimize_while_playing: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            schema: SETTINGS_SCHEMA,
            active_version: None,
            official: OfficialSettings::default(),
            dev_versions: Vec::new(),
            ntsc_only: false,
            minimize_while_playing: false,
        }
    }
}

/// GitHub releases feed (the game's releases on re-rac/rerac); on by default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OfficialSettings {
    pub enabled: bool,
    pub owner: String,
    pub repo: String,
}

impl Default for OfficialSettings {
    fn default() -> Self {
        OfficialSettings { enabled: true, owner: "re-rac".into(), repo: "rerac".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevVersionEntry {
    /// Absolute path of the build folder that holds `rerac-manifest.json`.
    pub path: PathBuf,
}

impl Settings {
    pub fn load(path: &Path) -> Settings {
        match fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                eprintln!("[rerac-launcher] {} is unreadable ({e}); using defaults", path.display());
                Settings::default()
            }),
            Err(_) => Settings::default(),
        }
    }

    /// Atomic write: temp file + rename.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(self).map_err(io::Error::other)?)?;
        fs::rename(tmp, path)
    }

    pub fn is_active(&self, r: &VersionRef) -> bool {
        self.active_version.as_ref() == Some(r)
    }

    pub fn add_dev_version(&mut self, path: PathBuf) -> VersionRef {
        if !self.dev_versions.iter().any(|d| d.path == path) {
            self.dev_versions.push(DevVersionEntry { path: path.clone() });
        }
        VersionRef { source: SourceId::Development, id: path.to_string_lossy().into_owned() }
    }

    pub fn remove_dev_version(&mut self, path: &Path) {
        self.dev_versions.retain(|d| d.path != path);
        let r = VersionRef { source: SourceId::Development, id: path.to_string_lossy().into_owned() };
        if self.is_active(&r) {
            self.active_version = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::tests::TempDir;

    #[test]
    fn round_trip_and_defaults() {
        let t = TempDir::new("settings");
        let file = t.0.join("settings/launcher.json");
        assert_eq!(Settings::load(&file), Settings::default());
        let mut s = Settings::default();
        let r = s.add_dev_version("/builds/dev".into());
        s.active_version = Some(r.clone());
        s.save(&file).unwrap();
        let back = Settings::load(&file);
        assert_eq!(back, s);
        assert!(back.is_active(&r));
        assert!(back.official.enabled);
    }

    #[test]
    fn missing_fields_take_defaults_and_garbage_resets() {
        let t = TempDir::new("settings-partial");
        let file = t.0.join("launcher.json");
        fs::write(&file, r#"{"ntsc_only":true}"#).unwrap();
        let s = Settings::load(&file);
        assert!(s.ntsc_only);
        assert_eq!(s.official, OfficialSettings::default());
        fs::write(&file, "not json").unwrap();
        assert_eq!(Settings::load(&file), Settings::default());
    }

    #[test]
    fn removing_the_active_dev_version_clears_it() {
        let mut s = Settings::default();
        let r = s.add_dev_version("/a".into());
        s.add_dev_version("/a".into());
        assert_eq!(s.dev_versions.len(), 1);
        s.active_version = Some(r);
        s.remove_dev_version(Path::new("/a"));
        assert!(s.dev_versions.is_empty());
        assert_eq!(s.active_version, None);
    }
}
