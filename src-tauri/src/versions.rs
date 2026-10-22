//! Game versions and where they come from.
//!
//! A version is a folder holding `rerac-manifest.json` (contract §Version). Versions come from
//! *sources*:
//!
//! - `official`: GitHub releases of each game's feed, downloaded and unpacked into
//!   `<root>/versions/official/<game>-<tag>/`.
//! - `development`: local build folders the user adds (they stay where they are, the id is the
//!   absolute path) and build zips installed with "Install from zip…" (unpacked into
//!   `<root>/versions/development/<game>-<version>/`, the id is the folder name).
//!
//! Every version's manifest names its game; each game runs its own chosen version
//! (`Settings::active_versions`). Folders from before per-game versions are named `<tag>` /
//! `<version>` and hold R&C1 builds ([`managed_id`]).
//!
//! Mods will be one more source (`versions/mods/<name>/`, installed like official builds from
//! another feed). Adding it means a new `SourceId` variant and its arm in `SourceId::location`;
//! nothing else keys on the source list. Installing from an archive is `crate::install`.

use crate::contract::{Manifest, RuntimeVersion, MANIFEST_FILE};
use crate::paths::Layout;
use crate::settings::Settings;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceId {
    Official,
    Development,
}

/// Where a source keeps its versions.
pub struct SourceLocation {
    /// Versions installed by the launcher live in `<root>/versions/<dir>/<id>/`.
    pub managed: &'static str,
    /// The source also lists folders the user registered; their id is the absolute path.
    pub external: bool,
}

impl SourceId {
    pub const ALL: [SourceId; 2] = [SourceId::Official, SourceId::Development];

    pub fn location(self) -> SourceLocation {
        match self {
            SourceId::Official => SourceLocation { managed: "official", external: false },
            SourceId::Development => SourceLocation { managed: "development", external: true },
        }
    }

    pub fn dir_name(self) -> &'static str {
        self.location().managed
    }
}

/// Identifies one version: `(source, id)`. For installed (managed) versions `id` is the folder
/// name under `versions/<source>/`; for registered development folders it is the absolute path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VersionRef {
    pub source: SourceId,
    pub id: String,
}

/// What the UI shows for a version.
#[derive(Debug, Clone, Serialize)]
pub struct VersionInfo {
    #[serde(rename = "ref")]
    pub vref: VersionRef,
    pub path: PathBuf,
    pub manifest: Option<Manifest>,
    pub runtime: Option<RuntimeVersion>,
    /// `None` when valid; otherwise why the version cannot be used.
    pub problem: Option<String>,
    pub active: bool,
    /// Installed by the launcher under `versions/<source>/`: removing it deletes the files.
    /// Registered development folders are only taken off the list.
    pub managed: bool,
}

/// A usable version with absolute binary paths.
#[derive(Debug, Clone)]
pub struct ResolvedVersion {
    pub vref: VersionRef,
    pub dir: PathBuf,
    pub manifest: Manifest,
    pub runtime: PathBuf,
    pub extractor: PathBuf,
}

/// A folder name usable as a managed version id: no separators, not hidden, not `.`/`..`.
/// (Hidden names are the installer's staging folders.)
pub fn valid_managed_id(id: &str) -> bool {
    !id.is_empty() && !id.contains(['/', '\\', ':']) && !id.starts_with('.') && id.len() <= 100
}

/// Whether the ref names a launcher-installed folder (as opposed to a registered path).
pub fn is_managed(r: &VersionRef) -> bool {
    !(r.source.location().external && Path::new(&r.id).is_absolute())
}

/// Folder of a version, or an error if the ref cannot name one.
pub fn version_dir(layout: &Layout, settings: &Settings, r: &VersionRef) -> Result<PathBuf, String> {
    let loc = r.source.location();
    if is_managed(r) {
        if !valid_managed_id(&r.id) {
            return Err(format!("invalid version id \"{}\"", r.id));
        }
        return Ok(layout.source_dir(loc.managed).join(&r.id));
    }
    let p = PathBuf::from(&r.id);
    if settings.dev_versions.iter().any(|d| d.path == p) {
        Ok(p)
    } else {
        Err("this development build is no longer in the list".into())
    }
}

pub fn read_manifest(dir: &Path) -> Result<Manifest, String> {
    let file = dir.join(MANIFEST_FILE);
    let text = fs::read_to_string(&file)
        .map_err(|_| format!("No {MANIFEST_FILE} in {}", dir.display()))?;
    Manifest::parse(&text).map_err(|e| e.to_string())
}

/// Manifest plus existence of both binaries. Cheap; used for listings.
pub fn resolve_dir(vref: VersionRef, dir: PathBuf) -> Result<ResolvedVersion, String> {
    let manifest = read_manifest(&dir)?;
    let runtime = dir.join(&manifest.runtime);
    let extractor = dir.join(&manifest.extractor);
    for (what, p) in [("Game runtime", &runtime), ("Extractor", &extractor)] {
        if !p.is_file() {
            return Err(format!("{what} not found at {}", p.display()));
        }
    }
    Ok(ResolvedVersion { vref, dir, manifest, runtime, extractor })
}

pub fn resolve(layout: &Layout, settings: &Settings, r: &VersionRef) -> Result<ResolvedVersion, String> {
    let dir = version_dir(layout, settings, r)?;
    resolve_dir(r.clone(), dir)
}

/// The version `game` runs; its manifest must be for that game.
pub fn resolve_active(layout: &Layout, settings: &Settings, game: &str) -> Result<ResolvedVersion, String> {
    let r = settings
        .active(game)
        .ok_or("No version is chosen for this game. Pick one above the Play button.")?;
    let v = resolve(layout, settings, r)?;
    if v.manifest.game != game {
        return Err(format!("The chosen version is for {}, not {game}.", v.manifest.game));
    }
    Ok(v)
}

/// Folder name of a launcher-installed version: `<game>-<version>` (an Official release tag or a
/// build zip's manifest version), so two games' builds of the same version never share a folder.
/// Installs from before per-game versions are plain `<version>` folders of R&C1 builds; they
/// stay valid (the manifest says which game a folder holds).
pub fn managed_id(game: &str, version: &str) -> String {
    format!("{game}-{version}")
}

/// The installed folder of release `tag` of `game` under `source_dir`, if any: `<game>-<tag>`,
/// or a plain `<tag>` folder from before per-game versions whose manifest is for `game`.
pub fn installed_id(source_dir: &Path, game: &str, tag: &str) -> Option<String> {
    let id = managed_id(game, tag);
    if source_dir.join(&id).join(MANIFEST_FILE).is_file() {
        return Some(id);
    }
    let legacy = source_dir.join(tag);
    (valid_managed_id(tag) && read_manifest(&legacy).is_ok_and(|m| m.game == game)).then(|| tag.to_string())
}

/// Runs `<runtime> --version-json` (10 s timeout) and checks it agrees with the manifest.
pub fn query_runtime(v: &ResolvedVersion) -> Result<RuntimeVersion, String> {
    let mut child = Command::new(&v.runtime)
        .arg("--version-json")
        .current_dir(&v.dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Could not start {}: {e}", v.runtime.display()))?;
    let mut stdout = child.stdout.take().expect("piped stdout");
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("The runtime did not answer --version-json within 10 s".into());
            }
        }
    }
    let out = reader.join().unwrap_or_default();
    let rv = RuntimeVersion::parse(&out)?;
    check_runtime_matches(&v.manifest, &rv)?;
    Ok(rv)
}

pub fn check_runtime_matches(m: &Manifest, rv: &RuntimeVersion) -> Result<(), String> {
    if rv.name != "rerac" {
        return Err(format!("The runtime reports name \"{}\", expected rerac", rv.name));
    }
    if rv.game != m.game {
        return Err(format!("The runtime is for {}, the manifest says {}", rv.game, m.game));
    }
    if rv.data_format != m.data_format {
        return Err(format!(
            "The runtime expects data format {}, the manifest says {}",
            rv.data_format, m.data_format
        ));
    }
    Ok(())
}

/// Full validation: manifest, binaries, and the runtime's own answer.
pub fn validate(vref: VersionRef, dir: PathBuf, active: bool) -> VersionInfo {
    match resolve_dir(vref.clone(), dir.clone()) {
        Ok(v) => match query_runtime(&v) {
            Ok(rv) => VersionInfo { managed: is_managed(&vref), vref, path: dir, manifest: Some(v.manifest), runtime: Some(rv), problem: None, active },
            Err(e) => VersionInfo { managed: is_managed(&vref), vref, path: dir, manifest: Some(v.manifest), runtime: None, problem: Some(e), active },
        },
        Err(e) => VersionInfo {
            managed: is_managed(&vref),
            vref,
            path: dir.clone(),
            manifest: read_manifest(&dir).ok(),
            runtime: None,
            problem: Some(e),
            active,
        },
    }
}

/// Launcher-installed versions of a source, newest folder name first.
pub fn list_managed(layout: &Layout, source: SourceId) -> Vec<(VersionRef, PathBuf)> {
    let base = layout.source_dir(source.dir_name());
    let mut entries: Vec<_> = fs::read_dir(&base)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| valid_managed_id(&e.file_name().to_string_lossy()) && e.path().join(MANIFEST_FILE).is_file())
        .map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            (VersionRef { source, id }, e.path())
        })
        .collect();
    entries.sort_by(|a, b| b.0.id.cmp(&a.0.id));
    entries
}

/// Cheap listing (no processes spawned) of every source.
pub fn list(layout: &Layout, settings: &Settings) -> Vec<VersionInfo> {
    let mut out = Vec::new();
    for source in SourceId::ALL {
        let mut refs = list_managed(layout, source);
        if source.location().external {
            refs.extend(
                settings
                    .dev_versions
                    .iter()
                    .map(|d| (VersionRef { source, id: d.path.to_string_lossy().into_owned() }, d.path.clone())),
            );
        }
        for (vref, dir) in refs {
            let active = settings.is_active(&vref);
            let (manifest, problem) = match resolve_dir(vref.clone(), dir.clone()) {
                Ok(v) => (Some(v.manifest), None),
                Err(e) => (read_manifest(&dir).ok(), Some(e)),
            };
            out.push(VersionInfo { managed: is_managed(&vref), vref, path: dir, manifest, runtime: None, problem, active });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::tests::TempDir;

    fn write_version(dir: &Path, runtime_body: &str) {
        fs::create_dir_all(dir.join("bin")).unwrap();
        fs::write(
            dir.join(MANIFEST_FILE),
            r#"{"schema":1,"name":"rerac","version":"0.1.0","game":"rac1","runtime":"bin/rerac","extractor":"bin/rerac-extract","supported_discs":["SCUS_971.99"],"data_format":1}"#,
        )
        .unwrap();
        for f in ["bin/rerac", "bin/rerac-extract"] {
            fs::write(dir.join(f), runtime_body).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(dir.join(f), fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
    }

    #[test]
    fn managed_ids_cannot_escape() {
        let l = Layout::new("/r");
        let s = Settings::default();
        for bad in ["", "..", ".", ".install-1", "a/b", "a\\b", "/abs"] {
            let r = VersionRef { source: SourceId::Official, id: bad.into() };
            assert!(version_dir(&l, &s, &r).is_err(), "{bad}");
        }
        let ok = VersionRef { source: SourceId::Official, id: "v0.1.0".into() };
        assert_eq!(version_dir(&l, &s, &ok).unwrap(), PathBuf::from("/r/versions/official/v0.1.0"));
        // Development: a plain id is an installed zip, an absolute path a registered folder.
        let dz = VersionRef { source: SourceId::Development, id: "0.1.0".into() };
        assert!(is_managed(&dz));
        assert_eq!(version_dir(&l, &s, &dz).unwrap(), PathBuf::from("/r/versions/development/0.1.0"));
    }

    #[test]
    fn dev_refs_must_be_registered() {
        let l = Layout::new("/r");
        let mut s = Settings::default();
        let r = VersionRef { source: SourceId::Development, id: "/b".into() };
        assert!(version_dir(&l, &s, &r).is_err());
        s.add_dev_version("/b".into());
        assert_eq!(version_dir(&l, &s, &r).unwrap(), PathBuf::from("/b"));
    }

    #[test]
    fn listing_covers_both_sources() {
        let t = TempDir::new("versions");
        let layout = Layout::new(t.0.join("root"));
        write_version(&layout.source_dir("official").join("v0.1.0"), "");
        fs::create_dir_all(layout.source_dir("official").join("junk")).unwrap();
        write_version(&layout.source_dir("official").join(".install-9"), "");
        write_version(&layout.source_dir("development").join("0.2.0"), "");
        let dev = t.0.join("devbuild");
        write_version(&dev, "");
        let mut s = Settings::default();
        let dref = s.add_dev_version(dev.clone());
        s.add_dev_version(t.0.join("missing"));
        s.active_versions.insert("rac1".into(), dref.clone());

        let all = list(&layout, &s);
        assert_eq!(all.len(), 4);
        assert_eq!(all[0].vref.source, SourceId::Official);
        assert_eq!(all[0].vref.id, "v0.1.0");
        assert!(all[0].problem.is_none() && all[0].managed);
        assert_eq!(all[1].vref, VersionRef { source: SourceId::Development, id: "0.2.0".into() });
        assert!(all[1].managed && all[1].problem.is_none());
        assert!(all[2].active && all[2].vref == dref && all[2].problem.is_none() && !all[2].managed);
        assert!(all[3].problem.as_deref().unwrap().contains("No rerac-manifest.json"));
        assert_eq!(resolve_active(&layout, &s, "rac1").unwrap().runtime, dev.join("bin/rerac"));
        assert!(resolve_active(&layout, &s, "rac2").is_err(), "no version chosen for rac2");
        s.active_versions.insert("rac2".into(), dref);
        assert!(resolve_active(&layout, &s, "rac2").unwrap_err().contains("is for rac1"));
    }

    #[test]
    fn installed_ids_are_per_game_and_old_folders_still_count() {
        let t = TempDir::new("installed-id");
        let official = t.0.join("official");
        assert_eq!(installed_id(&official, "rac1", "v0.3.7"), None);
        write_version(&official.join("rac1-v0.3.7"), "");
        assert_eq!(installed_id(&official, "rac1", "v0.3.7").as_deref(), Some("rac1-v0.3.7"));
        assert_eq!(installed_id(&official, "rac2", "v0.3.7"), None);
        // A schema-1 install: the plain tag, an R&C1 manifest.
        write_version(&official.join("v0.3.6"), "");
        assert_eq!(installed_id(&official, "rac1", "v0.3.6").as_deref(), Some("v0.3.6"));
        assert_eq!(installed_id(&official, "rac2", "v0.3.6"), None);
    }

    #[cfg(unix)]
    #[test]
    fn validate_runs_version_json() {
        let t = TempDir::new("validate");
        let good = t.0.join("good");
        write_version(
            &good,
            "#!/bin/sh\necho 'booting'\necho '{\"name\":\"rerac\",\"version\":\"0.1.0\",\"game\":\"rac1\",\"data_format\":1}'\n",
        );
        let r = VersionRef { source: SourceId::Development, id: good.to_string_lossy().into() };
        let info = validate(r, good, false);
        assert_eq!(info.problem, None);
        assert_eq!(info.runtime.unwrap().version, "0.1.0");

        let bad = t.0.join("bad");
        write_version(
            &bad,
            "#!/bin/sh\necho '{\"name\":\"rerac\",\"version\":\"0.1.0\",\"game\":\"rac1\",\"data_format\":2}'\n",
        );
        let r = VersionRef { source: SourceId::Development, id: bad.to_string_lossy().into() };
        let info = validate(r, bad, false);
        assert!(info.problem.unwrap().contains("data format"));
    }
}
