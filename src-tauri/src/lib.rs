//! ReRAC launcher backend. The page is a thin view; everything that touches files,
//! processes, dialogs or the network lives here.

mod commands;
pub mod contract;
pub mod extractor;
pub mod github;
pub mod install;
pub mod launch;
pub mod paths;
pub mod settings;
pub mod versions;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let default_root = commands::default_root_or_fallback(app.handle());
            let env_root: Option<std::path::PathBuf> = std::env::var_os(paths::ROOT_ENV).map(Into::into);
            let migrated = if env_root.is_none() { commands::migrate_legacy_root(&default_root) } else { None };
            let launcher = commands::Launcher::init(default_root, env_root);
            if let Some(note) = migrated {
                launcher.log_note(&note);
            }
            app.manage(launcher);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::game_status,
            commands::pick_iso,
            commands::start_extract,
            commands::start_verify,
            commands::export_target,
            commands::start_export,
            commands::cancel_job,
            commands::uninstall_game,
            commands::launch_game,
            commands::open_log,
            commands::set_minimize_while_playing,
            commands::set_graphics_backend,
            commands::open_folder,
            commands::open_url,
            commands::pick_folder,
            commands::move_data_root,
            commands::set_ntsc_only,
            commands::list_versions,
            commands::add_dev_version,
            commands::pick_zip,
            commands::install_version_zip,
            commands::uninstall_version,
            commands::validate_version,
            commands::remove_dev_version,
            commands::set_active_version,
            commands::official_releases,
            commands::set_official_config,
            commands::download_official,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the ReRAC launcher");
}
