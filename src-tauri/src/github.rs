//! Official releases from the GitHub releases API.
//!
//! On by default (`settings.official.enabled`), reading the game's releases on re-rac/rerac. The
//! owner and repo are configurable. A mod feed can reuse `fetch_releases` with another repo.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const USER_AGENT: &str = concat!("rerac-launcher/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone, Deserialize)]
pub struct GhRelease {
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GhAsset {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
}

/// One row of the Official table.
#[derive(Debug, Clone, Serialize)]
pub struct OfficialRelease {
    pub version: String,
    /// `YYYY-MM-DD`, empty when unknown.
    pub date: String,
    pub changes: String,
    pub url: String,
    pub prerelease: bool,
    pub asset: Option<GhAsset>,
    pub installed: bool,
}

pub fn releases_url(owner: &str, repo: &str) -> Result<String, String> {
    let ok = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    if !ok(owner) || !ok(repo) {
        return Err("Owner and repository may only contain letters, digits, '-', '_' and '.'".into());
    }
    Ok(format!("https://api.github.com/repos/{owner}/{repo}/releases?per_page=50"))
}

pub fn parse_releases(json: &str) -> Result<Vec<GhRelease>, String> {
    serde_json::from_str(json).map_err(|e| format!("Unexpected GitHub response: {e}"))
}

/// First line-ish summary of release notes for the Changes column.
pub fn summarize(body: &str) -> String {
    let text: Vec<&str> = body
        .lines()
        .map(|l| l.trim().trim_start_matches(['#', '-', '*', ' ']).trim())
        .filter(|l| !l.is_empty())
        .take(3)
        .collect();
    let mut s = text.join(" · ");
    if s.chars().count() > 160 {
        s = s.chars().take(157).collect::<String>() + "…";
    }
    s
}

/// Picks the `.zip` asset for this OS and CPU (release builds are zips, see
/// `crate::install`). Names are matched loosely (`macos`/`darwin`, `windows`/`win64`, `linux`;
/// `aarch64`/`arm64`, `x86_64`/`x64`/`amd64`).
pub fn pick_asset<'a>(assets: &'a [GhAsset], os: &str, arch: &str) -> Option<&'a GhAsset> {
    let os_keys: &[&str] = match os {
        "macos" => &["macos", "darwin", "mac", "osx"],
        "windows" => &["windows", "win64"],
        _ => &["linux"],
    };
    let arch_keys: &[&str] = match arch {
        "aarch64" => &["aarch64", "arm64"],
        _ => &["x86_64", "x64", "amd64"],
    };
    let archive = |n: &str| n.ends_with(".zip");
    let for_os: Vec<&GhAsset> = assets
        .iter()
        .filter(|a| {
            let n = a.name.to_ascii_lowercase();
            archive(&n) && os_keys.iter().any(|k| n.contains(k))
        })
        .collect();
    for_os
        .iter()
        .find(|a| {
            let n = a.name.to_ascii_lowercase();
            arch_keys.iter().any(|k| n.contains(k))
        })
        .or_else(|| {
            // An asset naming no CPU at all is taken as universal.
            for_os.iter().find(|a| {
                let n = a.name.to_ascii_lowercase();
                !["aarch64", "arm64", "x86_64", "x64", "amd64"].iter().any(|k| n.contains(k))
            })
        })
        .copied()
}

pub fn to_rows(releases: Vec<GhRelease>, installed_dir: &Path) -> Vec<OfficialRelease> {
    releases
        .into_iter()
        .filter(|r| !r.draft)
        .map(|r| {
            let asset = pick_asset(&r.assets, std::env::consts::OS, std::env::consts::ARCH).cloned();
            OfficialRelease {
                installed: installed_dir.join(&r.tag_name).join(crate::contract::MANIFEST_FILE).is_file(),
                date: r.published_at.as_deref().unwrap_or("").chars().take(10).collect(),
                changes: summarize(r.body.as_deref().unwrap_or("")),
                url: r.html_url,
                prerelease: r.prerelease,
                version: r.tag_name,
                asset,
            }
        })
        .collect()
}

pub async fn fetch_releases(owner: &str, repo: &str) -> Result<Vec<GhRelease>, String> {
    let url = releases_url(owner, repo)?;
    let resp = reqwest::Client::new()
        .get(url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Could not reach GitHub: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if status.as_u16() == 404 {
        return Err(format!("{owner}/{repo} was not found (it may still be private)"));
    }
    if !status.is_success() {
        return Err(format!("GitHub answered {status}"));
    }
    parse_releases(&text)
}

/// Downloads an asset into `dest_dir`, reporting `(done, total)` as it goes. Returns the file,
/// which the caller installs with `crate::install::install_archive` and then deletes.
pub async fn download_asset(
    asset: &GhAsset,
    dest_dir: &Path,
    mut progress: impl FnMut(u64, u64),
) -> Result<PathBuf, String> {
    if asset.name.contains(['/', '\\']) || asset.name.starts_with('.') {
        return Err(format!("refusing odd asset name \"{}\"", asset.name));
    }
    std::fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    let final_path = dest_dir.join(&asset.name);
    let part = dest_dir.join(format!("{}.part", asset.name));
    let mut resp = reqwest::Client::new()
        .get(&asset.browser_download_url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Download failed: {e}"))?;
    let total = resp.content_length().unwrap_or(asset.size);
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut done = 0u64;
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("Download failed: {e}"))? {
        std::io::Write::write_all(&mut file, &chunk).map_err(|e| e.to_string())?;
        done += chunk.len() as u64;
        progress(done, total);
    }
    drop(file);
    std::fs::rename(&part, &final_path).map_err(|e| e.to_string())?;
    Ok(final_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> GhAsset {
        GhAsset { name: name.into(), size: 1, browser_download_url: format!("https://x/{name}") }
    }

    #[test]
    fn url_validation() {
        assert_eq!(
            releases_url("re-rac", "rerac").unwrap(),
            "https://api.github.com/repos/re-rac/rerac/releases?per_page=50"
        );
        assert!(releases_url("a/b", "c").is_err());
        assert!(releases_url("", "c").is_err());
    }

    #[test]
    fn parses_github_shape() {
        let json = r###"[{"tag_name":"v0.2.0","name":"v0.2.0","published_at":"2026-10-01T12:00:00Z",
            "body":"## Changes\n- Faster loading\n- Fix Novalis","html_url":"https://github.com/o/r/releases/v0.2.0",
            "draft":false,"prerelease":false,"assets":[{"name":"rerac-0.2.0-macos-arm64.zip","size":10,
            "browser_download_url":"https://x/a","id":1}]},
            {"tag_name":"v0.3.0-draft","draft":true,"assets":[]}]"###;
        let rel = parse_releases(json).unwrap();
        assert_eq!(rel.len(), 2);
        let rows = to_rows(rel, Path::new("/nonexistent"));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].version, "v0.2.0");
        assert_eq!(rows[0].date, "2026-10-01");
        assert_eq!(rows[0].changes, "Changes · Faster loading · Fix Novalis");
        assert!(!rows[0].installed);
    }

    #[test]
    fn picks_platform_asset() {
        let assets = vec![
            asset("rerac-0.1.0-linux-x86_64.zip"),
            asset("rerac-0.1.0-macos-arm64.tar.gz"),
            asset("rerac-0.1.0-macos-arm64.zip"),
            asset("rerac-0.1.0-macos-x86_64.zip"),
            asset("rerac-v1-windows-x64.zip"),
            asset("checksums.txt"),
        ];
        // The game repo's release name (tools/package/package.sh).
        assert_eq!(pick_asset(&assets, "macos", "aarch64").unwrap().name, "rerac-0.1.0-macos-arm64.zip");
        assert_eq!(pick_asset(&assets, "macos", "x86_64").unwrap().name, "rerac-0.1.0-macos-x86_64.zip");
        assert_eq!(pick_asset(&assets, "windows", "x86_64").unwrap().name, "rerac-v1-windows-x64.zip");
        assert_eq!(pick_asset(&assets, "linux", "aarch64"), None);
        let universal = vec![asset("rerac-macos-universal.zip")];
        assert_eq!(pick_asset(&universal, "macos", "aarch64").unwrap().name, "rerac-macos-universal.zip");
    }

    #[test]
    fn summary_is_short() {
        assert_eq!(summarize(""), "");
        let long = "x".repeat(400);
        assert!(summarize(&long).chars().count() <= 158);
    }
}
