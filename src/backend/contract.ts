// Launcher <-> game contract v0 (docs/contract.md), mirrored from src-tauri/src/contract.rs,
// plus the shapes the Rust commands return. Keep in step with the Rust side.

export type GameId = "rac1" | "rac2" | "rac3" | "rac4";

export interface Manifest {
  schema: number;
  name: string;
  version: string;
  game: string;
  runtime: string;
  extractor: string;
  supported_discs: string[];
  data_format: number;
}

/** `extract` reports identify, copy (files are hashed while copied), then prepare (the engine cache); `verify` reports
 * verify; `export` (the optional Tier 2 exports, game-side clarification 18) reports export. */
export type Stage = "identify" | "copy" | "verify" | "prepare" | "export";

export type DiscGame = "rac1" | "rac2" | "rac3" | "racdl" | "unknown";

export type ExtractorEvent =
  | { type: "progress"; stage: Stage; done: number; total: number; file: string }
  | { type: "info"; message: string }
  | {
      type: "disc";
      serial: string;
      region: string;
      version: string;
      supported: boolean;
      /** Game-side extras (clarification 2 in the game repo's launcher_contract.md). */
      game?: DiscGame;
      title?: string;
      elf_sha1?: string;
    }
  | { type: "error"; code: number; message: string }
  | { type: "done"; elapsed_ms: number };

/** Extractor error codes; the process exit code equals the code. */
export const ErrorCode = {
  Ok: 0,
  CannotRead: 10,
  NotIso9660: 11,
  UnknownDisc: 20,
  UnsupportedBuild: 21,
  WriteFailure: 30,
  DiskFull: 31,
  VerifyFailed: 40,
  Internal: 99,
} as const;
export type ErrorCodeValue = (typeof ErrorCode)[keyof typeof ErrorCode];

export interface ExtractInfo {
  disc: string;
  data_format: number;
  extractor_version: string;
  ntsc_only: boolean;
  files: number;
  bytes: number;
}

export interface RuntimeVersion {
  name: string;
  version: string;
  game: string;
  data_format: number;
}

// ---- Launcher backend shapes (src-tauri/src/commands.rs etc.) ----

/** Version sources. Mods will be one more id here and one more entry in lib/sources.ts. */
export type SourceId = "official" | "development";

export interface VersionRef {
  source: SourceId;
  id: string;
}

export interface VersionInfo {
  ref: VersionRef;
  path: string;
  manifest: Manifest | null;
  runtime: RuntimeVersion | null;
  problem: string | null;
  active: boolean;
  /** Installed by the launcher under versions/<source>/ (removing deletes it); false for a
   * registered development folder (removing only takes it off the list). */
  managed: boolean;
}

/** A game's GitHub releases feed. */
export interface Feed {
  enabled: boolean;
  owner: string;
  repo: string;
}

export interface Settings {
  schema: number;
  /** Each game's chosen version, by game id. */
  active_versions: Partial<Record<string, VersionRef>>;
  /** Feeds the player changed, by game id (the snapshot's `feeds` has every game's). */
  feeds: Partial<Record<string, Feed>>;
  dev_versions: { path: string }[];
  ntsc_only: boolean;
  minimize_while_playing: boolean;
}

export type JobKind = "extract" | "verify" | "export";

/** `rerac-extract export --what` kinds (game-side clarification 18). */
export type ExportKind = "textures" | "audio" | "models" | "levels" | "collision" | "text";

export interface JobState {
  job: number;
  kind: JobKind;
  game: string;
}

export interface ActiveSummary {
  ref: VersionRef;
  version: string | null;
  game: string | null;
  data_format: number | null;
  problem: string | null;
}

export interface AppSnapshot {
  launcher_version: string;
  platform: string;
  data_root: string;
  default_data_root: string;
  settings: Settings;
  /** Each game's chosen version, by game id. */
  actives: Partial<Record<string, ActiveSummary>>;
  /** Games that can be installed and played: the supported ones, plus any game a listed build is for. */
  playable: string[];
  /** Each known game's release feed; null: the game has no releases yet. */
  feeds: Partial<Record<string, Feed | null>>;
  job: JobState | null;
  game_running: string | null;
  /** A newer launcher, downloaded and ready for "Restart to update". */
  launcher_update: LauncherUpdate | null;
}

export interface LauncherUpdate {
  version: string;
  notes: string | null;
}

export interface GameStatus {
  game: string;
  data_dir: string;
  installed: boolean;
  info: ExtractInfo | null;
  stale: boolean;
  /** The active version's data_format and version (null without a usable active version). */
  expected_format: number | null;
  active_version: string | null;
  job: JobState | null;
  running: boolean;
  /** Default destination of "Export assets…": this session's last export, else `<data>/exports`. */
  export_dir: string;
}

export interface EventPayload {
  job: number;
  kind: JobKind;
  game: string;
  event: ExtractorEvent;
}

export type FinishStatus = "ok" | "error" | "cancelled";

export interface FinishedPayload {
  job: number;
  kind: JobKind;
  game: string;
  status: FinishStatus;
  code: number;
  message: string;
  elapsed_ms: number;
}

export interface ExitedPayload {
  game: string;
  /** null: killed by a signal. */
  code: number | null;
  /** logs/<game>-<unix>.log with the runtime's stdout and stderr. */
  log: string;
  /** The runtime's `error: ...` stderr line when it could not start. */
  message: string | null;
}

/** Runtime exit codes when it cannot start (before any window opens). */
export const RuntimeExit = {
  BadArgs: 2,
  NoData: 3,
  DataMismatch: 4,
} as const;

export interface GhAsset {
  name: string;
  size: number;
  browser_download_url: string;
}

export interface OfficialRelease {
  version: string;
  date: string;
  changes: string;
  url: string;
  prerelease: boolean;
  asset: GhAsset | null;
  installed: boolean;
  /** The installed folder's version id (versions/official/<id>), when installed. */
  id: string | null;
}

export interface DownloadProgress {
  game: string;
  version: string;
  done: number;
  total: number;
}
