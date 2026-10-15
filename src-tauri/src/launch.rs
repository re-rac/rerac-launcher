//! Starts the game: `<runtime> --data-dir <game_data_dir>` with `RC_DATA_DIR` set too
//! (contract §Runtime), plus `WGPU_BACKEND` when the game's graphics backend is not automatic.
//! stdout and stderr go to `logs/<game>-<unix time>.log`.
//!
//! [`plan`] decides what to start from the active version and the installed data (and refuses
//! stale data before the runtime would exit 4); [`GameProcess`] runs it and reports the exit.

use crate::contract::DATA_DIR_ENV;
use crate::extractor;
use crate::paths::Layout;
use crate::settings::{GraphicsBackend, Settings, GRAPHICS_BACKEND_ENV};
use crate::versions;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const EXITED: &str = "game://exited";

#[derive(Debug, Clone, Serialize)]
pub struct ExitedPayload {
    pub game: String,
    pub code: Option<i32>,
    pub log: PathBuf,
    /// The runtime's `error: ...` line (stderr) when it could not start.
    pub message: Option<String>,
}

/// Runtime exit codes when it cannot start (contract §Runtime).
pub const EXIT_BAD_ARGS: i32 = 2;
pub const EXIT_NO_DATA: i32 = 3;
pub const EXIT_DATA_MISMATCH: i32 = 4;

/// Everything needed to start the game.
#[derive(Debug, Clone)]
pub struct LaunchPlan {
    pub game: String,
    pub version: String,
    pub runtime: PathBuf,
    /// The version folder (working directory of the runtime).
    pub workdir: PathBuf,
    pub data_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub graphics_backend: GraphicsBackend,
}

/// Resolves the active version and checks the installed data against it.
pub fn plan(layout: &Layout, settings: &Settings, game: &str) -> Result<LaunchPlan, String> {
    let v = versions::resolve_active(layout, settings)?;
    if v.manifest.game != game {
        return Err(format!("The active version is for {}, not {game}.", v.manifest.game));
    }
    let data_dir = layout.game_data_dir(game);
    let info = extractor::read_install(&data_dir).ok_or("The game data is not installed.")?;
    if info.data_format != v.manifest.data_format {
        return Err(format!(
            "The installed data is format {}, but ReRAC {} needs format {}. Re-extract from your disc image.",
            info.data_format, v.manifest.version, v.manifest.data_format
        ));
    }
    Ok(LaunchPlan {
        game: game.to_string(),
        version: v.manifest.version,
        runtime: v.runtime,
        workdir: v.dir,
        data_dir,
        logs_dir: layout.logs_dir(),
        graphics_backend: usable_backend(settings.graphics_backend(game), std::env::consts::OS),
    })
}

/// A choice this system cannot use (settings copied from another OS: DirectX 12 on Linux) falls back
/// to automatic instead of a game that finds no graphics card.
fn usable_backend(b: GraphicsBackend, os: &str) -> GraphicsBackend {
    if b.available_on(os) {
        b
    } else {
        GraphicsBackend::Auto
    }
}

/// Last `error: ...` line of a runtime log.
pub fn error_line(log: &str) -> Option<String> {
    log.lines().rev().find(|l| l.starts_with("error:")).map(|l| l.trim().to_string())
}

/// For a crash (a Rust panic exits 101): the last `panicked at` line and the message after it.
pub fn panic_lines(log: &str) -> Option<String> {
    let lines: Vec<&str> = log.lines().collect();
    let i = lines.iter().rposition(|l| l.contains("panicked at"))?;
    let mut s = lines[i].trim().to_string();
    if let Some(next) = lines.get(i + 1).filter(|l| !l.trim().is_empty() && !l.starts_with("note:")) {
        s.push('\n');
        s.push_str(next.trim());
    }
    Some(s)
}

#[derive(Default)]
pub struct GameProcess {
    running: Arc<Mutex<Option<String>>>,
}

pub fn command(runtime: &Path, workdir: &Path, data_dir: &Path) -> Command {
    let mut c = Command::new(runtime);
    c.arg("--data-dir").arg(data_dir).env(DATA_DIR_ENV, data_dir).current_dir(workdir);
    c
}

/// [`command`] plus the plan's options. `Auto` leaves `WGPU_BACKEND` as the launcher inherited it.
fn game_command(p: &LaunchPlan) -> Command {
    let mut c = command(&p.runtime, &p.workdir, &p.data_dir);
    if let Some(b) = p.graphics_backend.env_value() {
        c.env(GRAPHICS_BACKEND_ENV, b);
    }
    c
}

impl GameProcess {
    pub fn running(&self) -> Option<String> {
        self.running.lock().unwrap().clone()
    }

    /// Starts the game. `extra_env` is for tests (e.g. `RC_SCREENSHOT_FRAME`); the app passes
    /// none. Returns the log file path.
    pub fn launch(
        &self,
        p: &LaunchPlan,
        extra_env: &[(&str, &str)],
        on_exit: impl FnOnce(ExitedPayload) + Send + 'static,
    ) -> Result<PathBuf, String> {
        let game = p.game.as_str();
        let mut slot = self.running.lock().unwrap();
        if slot.is_some() {
            return Err("The game is already running.".into());
        }
        fs::create_dir_all(&p.logs_dir).map_err(|e| e.to_string())?;
        let unix = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        // Two launches in the same second (a quick exit 3/4, then Play again) get their own file.
        let mut log_path = p.logs_dir.join(format!("{game}-{unix}.log"));
        let mut n = 1;
        while log_path.exists() {
            n += 1;
            log_path = p.logs_dir.join(format!("{game}-{unix}-{n}.log"));
        }
        let mut log = fs::File::create(&log_path).map_err(|e| e.to_string())?;
        let backend = p.graphics_backend.env_value();
        {
            use std::io::Write;
            let _ = writeln!(
                log,
                "$ {}{} --data-dir {}   (ReRAC {}, from {})",
                backend.map_or(String::new(), |b| format!("{GRAPHICS_BACKEND_ENV}={b} ")),
                p.runtime.display(),
                p.data_dir.display(),
                p.version,
                p.workdir.display()
            );
        }
        let log_err = log.try_clone().map_err(|e| e.to_string())?;
        let mut cmd = game_command(p);
        cmd.envs(extra_env.iter().copied());
        let child: Child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err))
            .spawn()
            .map_err(|e| format!("Could not start the game ({}): {e}", p.runtime.display()))?;
        *slot = Some(game.to_string());
        drop(slot);

        let running = self.running.clone();
        let game = game.to_string();
        let returned = log_path.clone();
        std::thread::spawn(move || {
            let mut child = child;
            let code = loop {
                match child.try_wait() {
                    Ok(Some(s)) => break s.code(),
                    Ok(None) => std::thread::sleep(Duration::from_millis(200)),
                    Err(_) => break None,
                }
            };
            *running.lock().unwrap() = None;
            let message = if code == Some(0) {
                None
            } else {
                let text = fs::read_to_string(&log_path).unwrap_or_default();
                error_line(&text).or_else(|| panic_lines(&text))
            };
            if let Ok(mut f) = fs::OpenOptions::new().append(true).open(&log_path) {
                use std::io::Write;
                let _ = writeln!(f, "--- exit: {} ---", code.map_or("killed by a signal".to_string(), |c| c.to_string()));
            }
            on_exit(ExitedPayload { game, code, log: log_path, message });
        });
        Ok(returned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_passes_data_dir_both_ways() {
        let c = command(Path::new("/v/bin/rerac"), Path::new("/v"), Path::new("/r/games/rac1/data"));
        let args: Vec<_> = c.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args, ["--data-dir", "/r/games/rac1/data"]);
        let env: Vec<_> = c
            .get_envs()
            .map(|(k, v)| (k.to_string_lossy().into_owned(), v.map(|v| v.to_string_lossy().into_owned())))
            .collect();
        assert_eq!(env, [("RC_DATA_DIR".to_string(), Some("/r/games/rac1/data".to_string()))]);
        assert_eq!(c.get_current_dir(), Some(Path::new("/v")));
    }

    #[test]
    fn graphics_backend_sets_wgpu_backend_unless_auto() {
        let mut p = LaunchPlan {
            game: "rac1".into(),
            version: "0.1.0".into(),
            runtime: "/v/bin/rerac".into(),
            workdir: "/v".into(),
            data_dir: "/r/games/rac1/data".into(),
            logs_dir: "/r/logs".into(),
            graphics_backend: GraphicsBackend::Auto,
        };
        let backend_env = |p: &LaunchPlan| {
            game_command(p)
                .get_envs()
                .find(|(k, _)| *k == GRAPHICS_BACKEND_ENV)
                .map(|(_, v)| v.map(|v| v.to_string_lossy().into_owned()))
        };
        assert_eq!(backend_env(&p), None);
        p.graphics_backend = GraphicsBackend::Dx12;
        assert_eq!(backend_env(&p), Some(Some("dx12".into())));
        p.graphics_backend = GraphicsBackend::Vulkan;
        assert_eq!(backend_env(&p), Some(Some("vulkan".into())));
    }

    #[test]
    fn finds_the_runtime_error_line() {
        assert_eq!(
            error_line("booting\nerror: data_format 2 does not match 1\n"),
            Some("error: data_format 2 does not match 1".to_string())
        );
        assert_eq!(error_line("all good\n"), None);
        let panic = "fps: 60\nthread 'main' panicked at crates/rc-engine/src/main.rs:10:5:\nindex out of bounds\nnote: run with `RUST_BACKTRACE=1`\n";
        assert_eq!(
            panic_lines(panic).as_deref(),
            Some("thread 'main' panicked at crates/rc-engine/src/main.rs:10:5:\nindex out of bounds")
        );
        assert_eq!(panic_lines("fine\n"), None);
    }

    #[cfg(unix)]
    #[test]
    fn early_exit_reports_code_and_error_line() {
        use std::os::unix::fs::PermissionsExt;
        let t = crate::paths::tests::TempDir::new("launch");
        let rt = t.0.join("rt");
        fs::write(&rt, "#!/bin/sh\necho 'error: the data folder is incomplete' >&2\nexit 3\n").unwrap();
        fs::set_permissions(&rt, fs::Permissions::from_mode(0o755)).unwrap();
        let gp = GameProcess::default();
        let plan = LaunchPlan {
            game: "rac1".into(),
            version: "0.1.0".into(),
            runtime: rt,
            workdir: t.0.clone(),
            data_dir: t.0.join("data"),
            logs_dir: t.0.join("logs"),
            graphics_backend: GraphicsBackend::Auto,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let log = gp.launch(&plan, &[], move |p| tx.send(p).unwrap()).unwrap();
        let p = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(p.code, Some(EXIT_NO_DATA));
        assert_eq!(p.message.as_deref(), Some("error: the data folder is incomplete"));
        assert_eq!(p.log, log);
        assert!(log.file_name().unwrap().to_string_lossy().starts_with("rac1-"));
        let text = fs::read_to_string(&log).unwrap();
        assert!(text.contains("--data-dir") && text.contains("--- exit: 3 ---"), "{text}");
        assert!(gp.running().is_none());

        // A second launch in the same second gets its own log.
        let (tx, rx) = std::sync::mpsc::channel();
        let log2 = gp.launch(&plan, &[("RC_EXTRA", "1")], move |p| tx.send(p).unwrap()).unwrap();
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_ne!(log, log2);
    }

    #[test]
    fn plan_checks_version_and_data_format() {
        let t = crate::paths::tests::TempDir::new("plan");
        let layout = Layout::new(t.0.join("root"));
        let mut s = Settings::default();
        assert!(plan(&layout, &s, "rac1").unwrap_err().contains("No game version is active"));

        let v = layout.source_dir("development").join("0.1.0");
        fs::create_dir_all(&v).unwrap();
        fs::write(v.join(crate::contract::MANIFEST_FILE), crate::install::tests::MANIFEST).unwrap();
        fs::write(v.join("rerac"), "").unwrap();
        fs::write(v.join("rerac-extract"), "").unwrap();
        s.active_version = Some(versions::VersionRef { source: versions::SourceId::Development, id: "0.1.0".into() });
        assert!(plan(&layout, &s, "rac1").unwrap_err().contains("not installed"));

        let data = layout.game_data_dir("rac1");
        fs::create_dir_all(&data).unwrap();
        let info = |f: u32| format!(r#"{{"disc":"SCUS_971.99","data_format":{f},"extractor_version":"0.1.0","ntsc_only":false,"files":1,"bytes":1}}"#);
        fs::write(data.join("extract-info.json"), info(2)).unwrap();
        assert!(plan(&layout, &s, "rac1").unwrap_err().contains("format 2"));
        fs::write(data.join("extract-info.json"), info(1)).unwrap();
        let p = plan(&layout, &s, "rac1").unwrap();
        assert_eq!(p.runtime, v.join("rerac"));
        assert_eq!(p.workdir, v);
        assert_eq!(p.data_dir, data);
        assert_eq!(p.logs_dir, layout.logs_dir());
        assert_eq!(p.graphics_backend, GraphicsBackend::Auto);
        s.set_graphics_backend("rac1", GraphicsBackend::Dx12);
        let want = if cfg!(windows) { GraphicsBackend::Dx12 } else { GraphicsBackend::Auto };
        assert_eq!(plan(&layout, &s, "rac1").unwrap().graphics_backend, want);
    }

    #[test]
    fn a_backend_from_another_system_falls_back_to_auto() {
        assert_eq!(usable_backend(GraphicsBackend::Dx12, "windows"), GraphicsBackend::Dx12);
        assert_eq!(usable_backend(GraphicsBackend::Dx12, "linux"), GraphicsBackend::Auto);
        assert_eq!(usable_backend(GraphicsBackend::Vulkan, "linux"), GraphicsBackend::Vulkan);
        assert_eq!(usable_backend(GraphicsBackend::Vulkan, "macos"), GraphicsBackend::Auto);
        assert_eq!(usable_backend(GraphicsBackend::Auto, "macos"), GraphicsBackend::Auto);
    }
}
