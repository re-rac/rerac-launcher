//! Launcher settings, persisted as JSON at `<data root>/settings/launcher.json`.
//!
//! Each game has its own chosen version (`active_versions`) and its own release feed (`feeds`).
//! Schema 1 had one of each for the whole launcher; [`Settings::load`] moves them to R&C1, the
//! only game schema 1 could run.

use crate::versions::{SourceId, VersionRef};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const SETTINGS_SCHEMA: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema: u32,
    /// The version each game runs (its runtime and extractor), by game id.
    pub active_versions: BTreeMap<String, VersionRef>,
    /// Each game's release feed, by game id; a game without an entry uses [`default_feed`].
    pub feeds: BTreeMap<String, OfficialSettings>,
    /// Local build folders added in Version Management → Development.
    pub dev_versions: Vec<DevVersionEntry>,
    /// Pass `--ntsc-only` to the extractor (skips PAL-only movies and scenes).
    pub ntsc_only: bool,
    /// Minimise the launcher window while the game runs (restored when it exits).
    pub minimize_while_playing: bool,
    /// Schema 1's single active version, read once and moved to R&C1.
    #[serde(skip_serializing)]
    active_version: Option<VersionRef>,
    /// Schema 1's single release feed, read once and moved to R&C1.
    #[serde(skip_serializing)]
    official: Option<OfficialSettings>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            schema: SETTINGS_SCHEMA,
            active_versions: BTreeMap::new(),
            feeds: BTreeMap::new(),
            dev_versions: Vec::new(),
            ntsc_only: false,
            minimize_while_playing: false,
            active_version: None,
            official: None,
        }
    }
}

/// A game's GitHub releases feed.
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

/// Where a game's releases are published: R&C1 on re-rac/rerac, Going Commando on re-rac/rerac2.
/// `None`: the game has no releases yet.
pub fn default_feed(game: &str) -> Option<OfficialSettings> {
    let repo = match game {
        "rac1" => "rerac",
        "rac2" => "rerac2",
        _ => return None,
    };
    Some(OfficialSettings { enabled: true, owner: "re-rac".into(), repo: repo.into() })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevVersionEntry {
    /// Absolute path of the build folder that holds `rerac-manifest.json`.
    pub path: PathBuf,
}

impl Settings {
    pub fn load(path: &Path) -> Settings {
        let mut s = match fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                eprintln!("[rerac-launcher] {} is unreadable ({e}); using defaults", path.display());
                Settings::default()
            }),
            Err(_) => Settings::default(),
        };
        s.migrate();
        s
    }

    /// Schema 1 → 2: the single active version and feed become R&C1's.
    fn migrate(&mut self) {
        if let Some(v) = self.active_version.take() {
            self.active_versions.entry("rac1".into()).or_insert(v);
        }
        if let Some(f) = self.official.take() {
            self.feeds.entry("rac1".into()).or_insert(f);
        }
        self.schema = SETTINGS_SCHEMA;
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

    /// The version `game` runs, if one is chosen.
    pub fn active(&self, game: &str) -> Option<&VersionRef> {
        self.active_versions.get(game)
    }

    /// Whether `r` is some game's chosen version.
    pub fn is_active(&self, r: &VersionRef) -> bool {
        self.active_versions.values().any(|v| v == r)
    }

    /// Forgets `r` wherever it is chosen (its folder is gone or no longer listed).
    pub fn clear_active(&mut self, r: &VersionRef) {
        self.active_versions.retain(|_, v| v != r);
    }

    /// The release feed of `game`: its own entry, else [`default_feed`].
    pub fn feed(&self, game: &str) -> Option<OfficialSettings> {
        self.feeds.get(game).cloned().or_else(|| default_feed(game))
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
        self.clear_active(&r);
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
        s.active_versions.insert("rac1".into(), r.clone());
        s.save(&file).unwrap();
        let back = Settings::load(&file);
        assert_eq!(back, s);
        assert!(back.is_active(&r));
        assert_eq!(back.active("rac1"), Some(&r));
        assert_eq!(back.active("rac2"), None);
        assert_eq!(back.feed("rac1").unwrap().repo, "rerac");
        assert_eq!(back.feed("rac2").unwrap().repo, "rerac2");
        assert_eq!(back.feed("rac3"), None);
    }

    #[test]
    fn schema_1_moves_to_rac1() {
        let t = TempDir::new("settings-v1");
        let file = t.0.join("launcher.json");
        fs::write(
            &file,
            r#"{"schema":1,"active_version":{"source":"official","id":"v0.3.6"},
                "official":{"enabled":false,"owner":"me","repo":"fork"},"ntsc_only":true}"#,
        )
        .unwrap();
        let s = Settings::load(&file);
        assert_eq!(s.schema, SETTINGS_SCHEMA);
        assert_eq!(s.active("rac1"), Some(&VersionRef { source: SourceId::Official, id: "v0.3.6".into() }));
        assert_eq!(s.feed("rac1"), Some(OfficialSettings { enabled: false, owner: "me".into(), repo: "fork".into() }));
        assert!(s.ntsc_only);
        // Written back without the old fields.
        s.save(&file).unwrap();
        let text = fs::read_to_string(&file).unwrap();
        assert!(!text.contains("\"active_version\":") && !text.contains("\"official\":"), "{text}");
        assert_eq!(Settings::load(&file), s);
    }

    #[test]
    fn missing_fields_take_defaults_and_garbage_resets() {
        let t = TempDir::new("settings-partial");
        let file = t.0.join("launcher.json");
        fs::write(&file, r#"{"ntsc_only":true}"#).unwrap();
        let s = Settings::load(&file);
        assert!(s.ntsc_only);
        assert_eq!(s.feed("rac1"), default_feed("rac1"));
        fs::write(&file, "not json").unwrap();
        assert_eq!(Settings::load(&file), Settings::default());
    }

    #[test]
    fn removing_the_active_dev_version_clears_it() {
        let mut s = Settings::default();
        let r = s.add_dev_version("/a".into());
        s.add_dev_version("/a".into());
        assert_eq!(s.dev_versions.len(), 1);
        s.active_versions.insert("rac2".into(), r);
        s.remove_dev_version(Path::new("/a"));
        assert!(s.dev_versions.is_empty());
        assert_eq!(s.active("rac2"), None);
    }
}
