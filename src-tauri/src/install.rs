//! Installing a game version from a build archive (`.zip`) into `<root>/versions/<source>/<id>/`.
//!
//! One path serves "Install from zip…" (Development) and Official downloads (the downloaded
//! asset is handed to [`install_archive`] exactly like a local file):
//!
//! 1. unpack into a hidden staging folder `versions/<source>/.install-<pid>-<n>/`, keeping the
//!    executable bits (and relative symlinks) recorded in the zip;
//! 2. find the version folder: the staging root, or its single top-level folder (release zips
//!    wrap everything in `rerac-<version>-<os>-<arch>/`);
//! 3. validate the manifest and both binaries, clear the macOS quarantine flag
//!    ([`clear_quarantine`]), and ask the runtime for `--version-json`;
//! 4. rename the folder to `versions/<source>/<game>-<version>/` (`versions::managed_id`),
//!    replacing an older install of the same id.
//!
//! Nothing lands under `<id>` unless every step passed; staging is removed on any failure.

use crate::contract::{Manifest, RuntimeVersion, MANIFEST_FILE};
use crate::extractor::remove_dir_if_exists;
use crate::versions::{self, valid_managed_id, ResolvedVersion, SourceId, VersionRef};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// What an install produced.
#[derive(Debug, Clone)]
pub struct Installed {
    pub vref: VersionRef,
    pub dir: PathBuf,
    pub manifest: Manifest,
    pub runtime: RuntimeVersion,
    /// An older install with the same id was replaced.
    pub replaced: bool,
}

/// Entries the unpacker skips: macOS resource forks and Finder metadata. Release zips made with
/// `ditto`/`zip` on macOS carry an AppleDouble `._<name>` next to every file.
fn is_metadata(rel: &Path) -> bool {
    rel.components().any(|c| match c {
        Component::Normal(n) => {
            let n = n.to_string_lossy();
            n == "__MACOSX" || n == ".DS_Store" || n.starts_with("._")
        }
        _ => false,
    })
}

/// Unpacks `archive` into `dest` (which must exist). Refuses entries that would leave `dest`.
pub fn unpack_zip(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = fs::File::open(archive).map_err(|e| format!("Could not open {}: {e}", archive.display()))?;
    let mut zip = zip::ZipArchive::new(io::BufReader::new(file))
        .map_err(|e| format!("{} is not a readable zip file: {e}", archive.display()))?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| format!("Damaged zip file: {e}"))?;
        let rel = entry
            .enclosed_name()
            .ok_or_else(|| format!("The zip contains an unsafe path \"{}\"", entry.name()))?;
        if is_metadata(&rel) {
            continue;
        }
        let out = dest.join(&rel);
        let mode = entry.unix_mode();
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(|e| format!("Could not create {}: {e}", out.display()))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
        }
        let is_link = mode.is_some_and(|m| m & 0o170000 == 0o120000);
        if is_link {
            let mut target = String::new();
            io::Read::read_to_string(&mut entry, &mut target).map_err(|e| format!("Damaged zip file: {e}"))?;
            make_symlink(dest, &rel, &target)?;
            continue;
        }
        let mut f = fs::File::create(&out).map_err(|e| format!("Could not write {}: {e}", out.display()))?;
        io::copy(&mut entry, &mut f).map_err(|e| format!("Could not unpack {}: {e}", rel.display()))?;
        drop(f);
        #[cfg(unix)]
        if let Some(m) = mode {
            use std::os::unix::fs::PermissionsExt;
            // Permission bits only: no setuid/setgid/sticky from an archive.
            let _ = fs::set_permissions(&out, fs::Permissions::from_mode(m & 0o777));
        }
    }
    Ok(())
}

/// A symlink from the archive, allowed only when it is relative and stays inside `dest`.
fn make_symlink(dest: &Path, rel: &Path, target: &str) -> Result<(), String> {
    let t = Path::new(target);
    let mut depth: i32 = rel.components().count() as i32 - 1;
    let mut ok = !t.is_absolute();
    for c in t.components() {
        match c {
            Component::ParentDir => depth -= 1,
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            _ => ok = false,
        }
        if depth < 0 {
            ok = false;
        }
    }
    if !ok {
        return Err(format!("The zip contains a link that points outside it: {} -> {target}", rel.display()));
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(t, dest.join(rel)).map_err(|e| format!("Could not create link {}: {e}", rel.display()))
    }
    #[cfg(not(unix))]
    {
        Err(format!("Links in build archives are not supported on this system ({})", rel.display()))
    }
}

/// The folder in `staging` that holds the manifest: `staging` itself or its only subfolder.
pub fn find_version_root(staging: &Path) -> Result<PathBuf, String> {
    if staging.join(MANIFEST_FILE).is_file() {
        return Ok(staging.to_path_buf());
    }
    let dirs: Vec<PathBuf> = fs::read_dir(staging)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect();
    match dirs.as_slice() {
        [one] if one.join(MANIFEST_FILE).is_file() => Ok(one.clone()),
        _ => Err(format!("This archive is not a ReRAC build: there is no {MANIFEST_FILE} at its top level.")),
    }
}

/// Makes the manifest's binaries executable (archives made on Windows carry no Unix modes).
#[cfg(unix)]
fn ensure_executable(p: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let meta = fs::metadata(p).map_err(|e| e.to_string())?;
    let mode = meta.permissions().mode();
    if mode & 0o111 != 0o111 {
        fs::set_permissions(p, fs::Permissions::from_mode(mode | 0o755)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Removes macOS's `com.apple.quarantine` flag from `path` (recursively).
///
/// Why: a zip downloaded with a browser is quarantined, and unpackers that copy extended
/// attributes (Finder's Archive Utility, `ditto`) pass the flag on to every file. When a
/// quarantined, ad-hoc-signed binary like `rerac` is started, Gatekeeper blocks it ("cannot be
/// opened because the developer cannot be verified") and the launcher only sees a failed spawn.
/// The launcher's own unpacker does not copy extended attributes, but development folders
/// unpacked in Finder and future unpack paths can carry the flag, so it is cleared explicitly.
/// The user chose to install this build; this is the same as `xattr -dr com.apple.quarantine`.
/// No-op on other systems.
pub fn clear_quarantine(path: &Path) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("/usr/bin/xattr")
            .args(["-d", "-r", "com.apple.quarantine"])
            .arg(path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    #[cfg(not(target_os = "macos"))]
    let _ = path;
}

/// Checks an unpacked version folder: manifest, binaries (made executable, quarantine cleared)
/// and the runtime's `--version-json` answer.
pub fn check_version_folder(vref: VersionRef, dir: &Path) -> Result<(ResolvedVersion, RuntimeVersion), String> {
    let v = versions::resolve_dir(vref, dir.to_path_buf())?;
    #[cfg(unix)]
    for p in [&v.runtime, &v.extractor] {
        ensure_executable(p)?;
    }
    clear_quarantine(dir);
    let rv = versions::query_runtime(&v)?;
    Ok((v, rv))
}

fn staging_dir(source_dir: &Path) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    source_dir.join(format!(".install-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)))
}

/// Installs a build archive as a version of `source` under `source_dir`
/// (`<root>/versions/<source>/`), in the folder `<game>-<version>`: `tag` is the version (an
/// Official release tag); `None` uses the manifest's `version`. `known_games` limits what is
/// accepted (the games the launcher knows); `expect_game`, when given, is the only game accepted
/// (a game's release feed must hold that game's builds).
pub fn install_archive(
    archive: &Path,
    source: SourceId,
    source_dir: &Path,
    tag: Option<&str>,
    expect_game: Option<&str>,
    known_games: &[&str],
) -> Result<Installed, String> {
    let name = archive.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if !name.ends_with(".zip") {
        return Err("Choose a ReRAC build .zip file.".into());
    }
    fs::create_dir_all(source_dir).map_err(|e| format!("Could not create {}: {e}", source_dir.display()))?;
    let staging = staging_dir(source_dir);
    remove_dir_if_exists(&staging).map_err(|e| e.to_string())?;
    fs::create_dir_all(&staging).map_err(|e| format!("Could not create {}: {e}", staging.display()))?;
    let result = install_into(archive, source, source_dir, &staging, tag, expect_game, known_games);
    let _ = remove_dir_if_exists(&staging);
    result
}

fn install_into(
    archive: &Path,
    source: SourceId,
    source_dir: &Path,
    staging: &Path,
    tag: Option<&str>,
    expect_game: Option<&str>,
    known_games: &[&str],
) -> Result<Installed, String> {
    unpack_zip(archive, staging)?;
    let root = find_version_root(staging)?;
    let manifest = versions::read_manifest(&root)?;
    if !known_games.contains(&manifest.game.as_str()) {
        return Err(format!("This build is for \"{}\", a game this launcher does not know.", manifest.game));
    }
    if let Some(game) = expect_game.filter(|g| *g != manifest.game) {
        return Err(format!("This release is a build of {}, not {game}.", manifest.game));
    }
    let id = versions::managed_id(&manifest.game, tag.unwrap_or(&manifest.version));
    if !valid_managed_id(&id) {
        return Err(format!("\"{id}\" can't be used as a version folder name."));
    }
    let vref = VersionRef { source, id: id.clone() };
    let (_, runtime) = check_version_folder(vref.clone(), &root)?;

    let dest = source_dir.join(&id);
    let replaced = dest.exists();
    if replaced {
        remove_dir_if_exists(&dest).map_err(|e| format!("Could not replace {}: {e}", dest.display()))?;
    }
    fs::rename(&root, &dest).map_err(|e| format!("Could not move the build into {}: {e}", dest.display()))?;
    Ok(Installed { vref, dir: dest, manifest, runtime, replaced })
}

/// Where Official downloads are written before they are installed (hidden, never listed).
pub fn downloads_dir(official_dir: &Path) -> PathBuf {
    official_dir.join(".downloads")
}

/// Installs a downloaded Official release asset of `game` as `versions/official/<game>-<tag>/` and
/// deletes the download. The same path as a local zip; the version comes from the release tag.
pub fn install_download(file: &Path, official_dir: &Path, game: &str, tag: &str, known_games: &[&str]) -> Result<Installed, String> {
    let result = install_archive(file, SourceId::Official, official_dir, Some(tag), Some(game), known_games);
    let _ = fs::remove_file(file);
    if let Some(dir) = file.parent() {
        let _ = fs::remove_dir(dir); // only if empty
    }
    result
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::paths::tests::TempDir;
    use std::io::Write;

    pub const MANIFEST: &str = r#"{"schema":1,"name":"rerac","version":"0.1.0","game":"rac1","runtime":"rerac","extractor":"rerac-extract","supported_discs":["SCUS_971.99"],"data_format":1}"#;
    pub const RUNTIME: &str = "#!/bin/sh\n[ \"$1\" = --version-json ] && echo '{\"name\":\"rerac\",\"version\":\"0.1.0\",\"game\":\"rac1\",\"data_format\":1}'\nexit 0\n";

    /// A zip shaped like the game repo's release: one top folder, AppleDouble files, exec bits.
    pub fn release_zip(path: &Path, top: Option<&str>, runtime_mode: u32) {
        use zip::write::SimpleFileOptions;
        let mut w = zip::ZipWriter::new(fs::File::create(path).unwrap());
        let p = |n: &str| match top {
            Some(t) => format!("{t}/{n}"),
            None => n.to_string(),
        };
        let file = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        if let Some(t) = top {
            w.add_directory(format!("{t}/"), file.unix_permissions(0o755)).unwrap();
        }
        for (name, body, mode) in [
            ("rerac", RUNTIME, runtime_mode),
            ("rerac-extract", "#!/bin/sh\nexit 0\n", 0o755),
            ("rerac-manifest.json", MANIFEST, 0o644),
            ("assets/shaders/tie.wgsl", "// shader", 0o644),
            ("._rerac", "apple double", 0o644),
        ] {
            w.start_file(p(name), file.unix_permissions(mode)).unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        w.start_file("__MACOSX/._x", file).unwrap();
        w.write_all(b"x").unwrap();
        w.finish().unwrap();
    }

    #[cfg(unix)]
    fn mode(p: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    #[cfg(unix)]
    #[test]
    fn installs_a_release_zip_keeping_exec_bits() {
        let t = TempDir::new("install-zip");
        let zip_path = t.0.join("rerac-0.1.0-macos-arm64.zip");
        release_zip(&zip_path, Some("rerac-0.1.0-macos-arm64"), 0o755);
        let src = t.0.join("root/versions/development");
        let got = install_archive(&zip_path, SourceId::Development, &src, None, None, &["rac1"]).unwrap();
        assert_eq!(got.vref, VersionRef { source: SourceId::Development, id: "rac1-0.1.0".into() });
        assert_eq!(got.dir, src.join("rac1-0.1.0"));
        assert_eq!(got.runtime.version, "0.1.0");
        assert!(!got.replaced);
        assert_eq!(mode(&got.dir.join("rerac")), 0o755);
        assert_eq!(mode(&got.dir.join("rerac-extract")), 0o755);
        assert!(got.dir.join("assets/shaders/tie.wgsl").is_file());
        assert!(!got.dir.join("._rerac").exists());
        // Only the version folder is left: no staging, no __MACOSX.
        let names: Vec<_> = fs::read_dir(&src).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(names, ["rac1-0.1.0"]);

        // Reinstalling replaces it.
        let again = install_archive(&zip_path, SourceId::Development, &src, None, None, &["rac1"]).unwrap();
        assert!(again.replaced);
    }

    #[cfg(unix)]
    #[test]
    fn flat_zip_without_exec_bits_is_fixed_up() {
        let t = TempDir::new("install-flat");
        let zip_path = t.0.join("build.zip");
        release_zip(&zip_path, None, 0o644);
        let src = t.0.join("versions/official");
        let got = install_archive(&zip_path, SourceId::Official, &src, Some("v0.1.0"), None, &["rac1"]).unwrap();
        assert_eq!(got.dir, src.join("rac1-v0.1.0"));
        assert_eq!(mode(&got.dir.join("rerac")) & 0o111, 0o111);
    }

    /// The Official path: a "downloaded" asset (a local zip standing in for the GitHub download)
    /// is unpacked, validated and named after the tag; the download is removed.
    #[cfg(unix)]
    #[test]
    fn official_download_installs_under_the_tag() {
        let t = TempDir::new("install-official");
        let official = t.0.join("root/versions/official");
        let dl = downloads_dir(&official);
        fs::create_dir_all(&dl).unwrap();
        let file = dl.join("rerac-0.1.0-macos-arm64.zip");
        release_zip(&file, Some("rerac-0.1.0-macos-arm64"), 0o755);
        let got = install_download(&file, &official, "rac1", "v0.1.0", &["rac1"]).unwrap();
        assert_eq!(got.vref, VersionRef { source: SourceId::Official, id: "rac1-v0.1.0".into() });
        assert!(got.dir.join("rerac").is_file());
        assert!(!file.exists() && !dl.exists());
        // It shows up in the listing like any installed version, and github::to_rows sees it.
        let layout = crate::paths::Layout::new(t.0.join("root"));
        let listed = versions::list(&layout, &crate::settings::Settings::default());
        assert_eq!(listed.len(), 1);
        assert!(listed[0].managed && listed[0].problem.is_none());
        // A bad tag is refused and nothing is installed.
        fs::create_dir_all(&dl).unwrap();
        release_zip(&file, None, 0o755);
        assert!(install_download(&file, &official, "rac1", "../x", &["rac1"]).is_err());
        assert_eq!(versions::list_managed(&layout, SourceId::Official).len(), 1);
        // Another game's feed may not install an R&C1 build.
        fs::create_dir_all(&dl).unwrap();
        release_zip(&file, None, 0o755);
        assert!(install_download(&file, &official, "rac2", "v0.1.0", &["rac1", "rac2"]).unwrap_err().contains("not rac2"));
        assert_eq!(versions::list_managed(&layout, SourceId::Official).len(), 1);
    }

    #[test]
    fn rejects_non_builds_and_cleans_up() {
        let t = TempDir::new("install-bad");
        let src = t.0.join("versions/development");
        // Not a zip at all.
        let bogus = t.0.join("bogus.zip");
        fs::write(&bogus, "nope").unwrap();
        assert!(install_archive(&bogus, SourceId::Development, &src, None, None, &["rac1"]).unwrap_err().contains("not a readable zip"));
        // A zip with no manifest.
        let empty = t.0.join("empty.zip");
        {
            let mut w = zip::ZipWriter::new(fs::File::create(&empty).unwrap());
            w.start_file("readme.txt", zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(b"hi").unwrap();
            w.finish().unwrap();
        }
        assert!(install_archive(&empty, SourceId::Development, &src, None, None, &["rac1"]).unwrap_err().contains("not a ReRAC build"));
        // Wrong extension.
        assert!(install_archive(&t.0.join("x.tar.gz"), SourceId::Development, &src, None, None, &["rac1"]).is_err());
        // Unknown game.
        let zip_path = t.0.join("b.zip");
        release_zip(&zip_path, Some("top"), 0o755);
        assert!(install_archive(&zip_path, SourceId::Development, &src, None, None, &["rac2"]).unwrap_err().contains("does not know"));
        let left: Vec<_> = fs::read_dir(&src).unwrap().flatten().collect();
        assert!(left.is_empty(), "staging left behind: {left:?}");
    }

    #[test]
    fn refuses_escaping_entries() {
        let t = TempDir::new("install-escape");
        let zip_path = t.0.join("evil.zip");
        {
            let mut w = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
            w.start_file("../evil.txt", zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(b"x").unwrap();
            w.finish().unwrap();
        }
        let dest = t.0.join("dest");
        fs::create_dir_all(&dest).unwrap();
        assert!(unpack_zip(&zip_path, &dest).unwrap_err().contains("unsafe path"));
        assert!(!t.0.join("evil.txt").exists());
    }

    #[test]
    fn symlink_targets_must_stay_inside() {
        let t = TempDir::new("install-link");
        assert!(make_symlink(&t.0, Path::new("a/link"), "../../outside").is_err());
        assert!(make_symlink(&t.0, Path::new("a/link"), "/etc/passwd").is_err());
        fs::create_dir_all(t.0.join("a")).unwrap();
        #[cfg(unix)]
        assert!(make_symlink(&t.0, Path::new("a/link"), "../rerac").is_ok());
    }
}
