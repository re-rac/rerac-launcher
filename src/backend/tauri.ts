import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Backend } from "./api";
import type {
  AppSnapshot,
  DownloadProgress,
  EventPayload,
  ExitedPayload,
  FinishedPayload,
  GameStatus,
  LauncherUpdate,
  OfficialRelease,
  VersionInfo,
  VersionRef,
} from "./contract";

// Event names match the constants in src-tauri (extractor.rs, launch.rs, commands.rs).
const EV_EXTRACTOR = "extractor://event";
const EV_FINISHED = "extractor://finished";
const EV_EXITED = "game://exited";
const EV_DOWNLOAD = "download://progress";
const EV_UPDATE = "launcher://update-ready";

export function createTauriBackend(): Backend {
  const win = getCurrentWindow();
  return {
    mode: "tauri",
    getSnapshot: () => invoke<AppSnapshot>("get_snapshot"),
    gameStatus: (game) => invoke<GameStatus>("game_status", { game }),
    pickIso: () => invoke<string | null>("pick_iso"),
    startExtract: (game, iso) => invoke<number>("start_extract", { game, iso }),
    startVerify: (game) => invoke<number>("start_verify", { game }),
    exportTarget: (game, picked) => invoke<string>("export_target", { game, picked }),
    startExport: (game, to, what) => invoke<number>("start_export", { game, to, what }),
    cancelJob: () => invoke<boolean>("cancel_job"),
    uninstallGame: (game) => invoke<void>("uninstall_game", { game }),
    launchGame: (game) => invoke<void>("launch_game", { game }),
    openLog: (path) => invoke<void>("open_log", { path }),
    setMinimizeWhilePlaying: (value) => invoke<AppSnapshot>("set_minimize_while_playing", { value }),
    openFolder: (which) => invoke<void>("open_folder", { which }),
    openUrl: (url) => invoke<void>("open_url", { url }),
    pickFolder: (title) => invoke<string | null>("pick_folder", { title }),
    moveDataRoot: (target) => invoke<AppSnapshot>("move_data_root", { target }),
    setNtscOnly: (value) => invoke<AppSnapshot>("set_ntsc_only", { value }),
    listVersions: () => invoke<VersionInfo[]>("list_versions"),
    addDevVersion: (path) => invoke<VersionInfo>("add_dev_version", { path }),
    pickZip: () => invoke<string | null>("pick_zip"),
    installVersionZip: (path) => invoke<VersionInfo>("install_version_zip", { path }),
    uninstallVersion: (vref: VersionRef) => invoke<AppSnapshot>("uninstall_version", { vref }),
    validateVersion: (vref: VersionRef) => invoke<VersionInfo>("validate_version", { vref }),
    removeDevVersion: (path) => invoke<AppSnapshot>("remove_dev_version", { path }),
    setActiveVersion: (vref: VersionRef) => invoke<AppSnapshot>("set_active_version", { vref }),
    officialReleases: (game) => invoke<OfficialRelease[]>("official_releases", { game }),
    setOfficialConfig: (game, enabled, owner, repo) => invoke<AppSnapshot>("set_official_config", { game, enabled, owner, repo }),
    downloadOfficial: (game, version) => invoke<VersionInfo>("download_official", { game, version }),
    onExtractorEvent: (cb) => listen<EventPayload>(EV_EXTRACTOR, (e) => cb(e.payload)),
    onExtractorFinished: (cb) => listen<FinishedPayload>(EV_FINISHED, (e) => cb(e.payload)),
    onGameExited: (cb) => listen<ExitedPayload>(EV_EXITED, (e) => cb(e.payload)),
    onDownloadProgress: (cb) => listen<DownloadProgress>(EV_DOWNLOAD, (e) => cb(e.payload)),
    installLauncherUpdate: () => invoke<void>("install_launcher_update"),
    onLauncherUpdate: (cb) => listen<LauncherUpdate>(EV_UPDATE, (e) => cb(e.payload)),
    window: {
      minimize: () => win.minimize(),
      toggleMaximize: () => win.toggleMaximize(),
      close: () => win.close(),
    },
  };
}
