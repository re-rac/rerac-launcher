//! The launcher updating itself (tauri-plugin-updater).
//!
//! A few seconds after start, then every six hours, the launcher reads the update feed
//! (`plugins.updater.endpoints` in tauri.conf.json: the `latest.json` the release workflow writes to
//! the repository's `updater` branch). A newer launcher is downloaded in the background and its
//! signature checked against the public key in tauri.conf.json; the page then shows "Restart to
//! update" ([`READY`], the snapshot's `launcher_update`). Installing waits for the player, and
//! never happens while a game or an extractor job runs (`commands::install_launcher_update`).
//! Failures (offline, no feed yet, a bad signature) are only logged: the launcher works as it is.

use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Emitted with an [`UpdateInfo`] once an update is downloaded and ready to install.
pub const READY: &str = "launcher://update-ready";

const FIRST_CHECK: Duration = Duration::from_secs(5);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// What the page shows about a downloaded update.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

struct Ready {
    info: UpdateInfo,
    update: Update,
    bytes: Vec<u8>,
}

/// The downloaded update waiting for "Restart to update".
#[derive(Default)]
pub struct LauncherUpdate(Mutex<Option<Ready>>);

impl LauncherUpdate {
    pub fn info(&self) -> Option<UpdateInfo> {
        self.0.lock().unwrap().as_ref().map(|r| r.info.clone())
    }

    /// Installs the downloaded update. The caller restarts the app afterwards (on Windows the
    /// installer closes it itself).
    pub fn install(&self) -> Result<(), String> {
        let ready = self.0.lock().unwrap().take().ok_or("No launcher update is ready.")?;
        ready.update.install(&ready.bytes).map_err(|e| format!("Could not install the update: {e}"))
    }
}

/// Starts the background checks.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_CHECK);
        loop {
            if let Err(e) = tauri::async_runtime::block_on(check(&app)) {
                eprintln!("[rerac-launcher] update check: {e}");
            }
            std::thread::sleep(CHECK_EVERY);
        }
    });
}

async fn check(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<LauncherUpdate>();
    if state.info().is_some() {
        return Ok(()); // already downloaded; waiting for the player
    }
    let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    let Some(update) = update else { return Ok(()) };
    let bytes = update.download(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    let info = UpdateInfo { version: update.version.clone(), notes: update.body.clone() };
    *state.0.lock().unwrap() = Some(Ready { info: info.clone(), update, bytes });
    let _ = app.emit(READY, info);
    Ok(())
}
