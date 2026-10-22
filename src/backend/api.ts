// The page talks to the backend only through this interface. In the Tauri app it is the Rust
// commands (tauri.ts); in a plain browser (vite dev) it is an in-memory mock (mock.ts) so the
// UI can be developed and screenshot without the app or the game.

import type {
  AppSnapshot,
  DownloadProgress,
  EventPayload,
  ExitedPayload,
  ExportKind,
  FinishedPayload,
  LauncherUpdate,
  GameStatus,
  OfficialRelease,
  VersionInfo,
  VersionRef,
} from "./contract";

export type Unlisten = () => void;

/** `root`, `logs`, `versions`, `game:<id>`, `export:<id>` (last export), `source:<id>`, `path:<dev build folder>`. */
export type FolderId = string;

export interface Backend {
  mode: "tauri" | "mock";

  getSnapshot(): Promise<AppSnapshot>;
  gameStatus(game: string): Promise<GameStatus>;

  pickIso(): Promise<string | null>;
  startExtract(game: string, iso: string): Promise<number>;
  startVerify(game: string): Promise<number>;
  /** Where an export into the picked folder goes (the folder itself if empty or an earlier export,
   * else `rerac-<game>-exports` inside it). */
  exportTarget(game: string, picked: string): Promise<string>;
  /** Runs `rerac-extract export` on the installed data into `to`. */
  startExport(game: string, to: string, what: ExportKind[]): Promise<number>;
  cancelJob(): Promise<boolean>;
  uninstallGame(game: string): Promise<void>;
  launchGame(game: string): Promise<void>;
  /** Shows a log file (inside the logs folder) in the file manager. */
  openLog(path: string): Promise<void>;
  setMinimizeWhilePlaying(value: boolean): Promise<AppSnapshot>;

  openFolder(which: FolderId): Promise<void>;
  openUrl(url: string): Promise<void>;
  pickFolder(title: string): Promise<string | null>;
  moveDataRoot(target: string): Promise<AppSnapshot>;
  setNtscOnly(value: boolean): Promise<AppSnapshot>;

  listVersions(): Promise<VersionInfo[]>;
  addDevVersion(path: string): Promise<VersionInfo>;
  pickZip(): Promise<string | null>;
  /** Unpacks a build zip into versions/development/<game>-<version>/ and validates it. */
  installVersionZip(path: string): Promise<VersionInfo>;
  /** Deletes a launcher-installed version (managed: true). */
  uninstallVersion(vref: VersionRef): Promise<AppSnapshot>;
  validateVersion(vref: VersionRef): Promise<VersionInfo>;
  removeDevVersion(path: string): Promise<AppSnapshot>;
  /** Makes the version the chosen one of the game its manifest names. */
  setActiveVersion(vref: VersionRef): Promise<AppSnapshot>;

  /** The releases of the game's feed. */
  officialReleases(game: string): Promise<OfficialRelease[]>;
  setOfficialConfig(game: string, enabled: boolean, owner: string, repo: string): Promise<AppSnapshot>;
  /** Downloads and installs a release of the game into versions/official/<game>-<tag>/. */
  downloadOfficial(game: string, version: string): Promise<VersionInfo>;

  onExtractorEvent(cb: (p: EventPayload) => void): Promise<Unlisten>;
  onExtractorFinished(cb: (p: FinishedPayload) => void): Promise<Unlisten>;
  onGameExited(cb: (p: ExitedPayload) => void): Promise<Unlisten>;
  onDownloadProgress(cb: (p: DownloadProgress) => void): Promise<Unlisten>;

  /** Installs the downloaded launcher update and restarts into it (refused while a game or job runs). */
  installLauncherUpdate(): Promise<void>;
  /** A launcher update finished downloading. */
  onLauncherUpdate(cb: (u: LauncherUpdate) => void): Promise<Unlisten>;

  window: {
    minimize(): Promise<void>;
    toggleMaximize(): Promise<void>;
    close(): Promise<void>;
  };
}

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

let backend: Backend | null = null;

export async function loadBackend(): Promise<Backend> {
  if (backend) return backend;
  backend = isTauri() ? (await import("./tauri")).createTauriBackend() : (await import("./mock")).createMockBackend();
  return backend;
}

/** Errors from Rust commands arrive as plain strings. */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
