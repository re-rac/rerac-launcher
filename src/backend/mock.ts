// In-browser mock of the Rust backend, for `npm run dev` in a normal browser and for headless
// screenshots. It follows the same contract as dev/mock/rerac-extract.
//
// URL parameters:
//   ?mock=not-installed | installed | extracting | verifying | exporting | exported | error21 | error20 | sequel | no-version | stale | playing
//   ?iso=<file name>   what the fake file dialog returns (e.g. "Ratchet & Clank (Europe).iso" -> error 21)
//   ?runtime=2|3|4|101 the fake game refuses to start with that exit code (101: a crash)
//   ?zip=<file name>   what the fake .zip dialog returns ("bad" in the name: not a build)
//   ?autoplay=1        press Play on load (for screenshots of the launch path)
//   ?export=fail31     the next export fails with that code

import type { Backend, Unlisten } from "./api";
import type {
  AppSnapshot,
  DownloadProgress,
  EventPayload,
  ExitedPayload,
  ExtractInfo,
  ExtractorEvent,
  FinishedPayload,
  GameStatus,
  JobKind,
  JobState,
  Manifest,
  OfficialRelease,
  Settings,
  VersionInfo,
  VersionRef,
} from "./contract";

type Scenario =
  | "not-installed"
  | "installed"
  | "extracting"
  | "verifying"
  | "exporting"
  | "exported"
  | "error21"
  | "error20"
  | "sequel"
  | "no-version"
  | "stale"
  | "playing";

const params = new URLSearchParams(typeof location !== "undefined" ? location.search : "");
const scenario = (params.get("mock") ?? "not-installed") as Scenario;

const HOME = "/Users/you";
const DEFAULT_ROOT = `${HOME}/Library/Application Support/rerac`;
const DEV_BUILD = `${HOME}/Repos/rerac/target/release`;

const MESSAGES: Record<number, string> = {
  10: "cannot read the image: it has fewer sectors than its volume descriptor declares (truncated)",
  11: "not an ISO 9660 image (only 2048-byte-sector .iso images are supported)",
  20: "not a Ratchet & Clank disc: boot ELF SLUS_203.12 is another game",
  21: "Ratchet & Clank build SCES_509.16 (PAL) is recognised but not supported yet",
  30: "write failure: permission denied",
  31: "not enough disk space: need 4.02 GiB, 1.10 GiB free",
  40: "verification failed: 2 files do not match (levels/05/gameplay_ntsc.bin, global/sound/vag.bin)",
  99: "internal error: unexpected end of sector table",
};

const FILES: [string, number][] = [
  ["boot/SYSTEM.CNF", 64],
  ["boot/SCUS_971.99", 3_280_000],
  ["toc.bin", 13_000],
  ["global/armor.wad", 18_400_000],
  ["global/gadgets.wad", 44_100_000],
  ["global/hud.wad", 22_900_000],
  ["global/sound/vag.bin", 312_000_000],
  ["global/sound/music.bin", 402_000_000],
  ...Array.from({ length: 20 }, (_, i) => [`levels/${String(i).padStart(2, "0")}/gameplay_ntsc.bin`, 58_000_000 + i * 3_700_000] as [string, number]),
  ...Array.from({ length: 19 }, (_, i) => [`global/mpegs/${String(i + 21).padStart(3, "0")}.pss`, 62_000_000 + i * 900_000] as [string, number]),
];
const TOTAL = FILES.reduce((a, [, s]) => a + s, 0);

function devManifest(): Manifest {
  return {
    schema: 1,
    name: "rerac",
    version: "0.1.0-dev",
    game: "rac1",
    runtime: "rerac",
    extractor: "rerac-extract",
    supported_discs: ["SCUS_971.99"],
    data_format: scenario === "stale" ? 2 : 1,
  };
}

type Disc = Extract<ExtractorEvent, { type: "disc" }>;
const disc = (serial: string, region: string, supported: boolean, game: Disc["game"], title: string): Disc => ({
  type: "disc",
  serial,
  region,
  version: "1.00",
  supported,
  game,
  title,
  elf_sha1: "0000mock",
});
const DISCS = {
  ok: disc("SCUS_971.99", "NTSC-U", true, "rac1", "Ratchet & Clank"),
  pal: disc("SCES_509.16", "PAL", false, "rac1", "Ratchet & Clank"),
  rac2: disc("SCUS_972.68", "NTSC-U", false, "rac2", "Ratchet & Clank: Going Commando"),
  rac3: disc("SCUS_973.53", "NTSC-U", false, "rac3", "Ratchet & Clank: Up Your Arsenal"),
  other: disc("SLUS_203.12", "NTSC-U", false, "unknown", ""),
};

/** Same rules as dev/mock/rerac-extract. */
function outcome(iso: string): { err: number; disc?: Disc } {
  const name = iso.split(/[\\/]/).pop()!.toLowerCase();
  const m = name.match(/err(\d\d)/);
  if (m) return { err: Number(m[1]), disc: DISCS.ok };
  if (name.endsWith(".bin")) return { err: 11 };
  if (name.includes("notrac")) return { err: 20, disc: DISCS.other };
  if (name.includes("rac2") || name.includes("commando")) return { err: 21, disc: DISCS.rac2 };
  if (name.includes("rac3") || name.includes("arsenal")) return { err: 21, disc: DISCS.rac3 };
  if (name.includes("pal") || name.includes("europe")) return { err: 21, disc: DISCS.pal };
  return { err: 0, disc: DISCS.ok };
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export function createMockBackend(): Backend {
  const noVersion = scenario === "no-version";
  const settings: Settings = {
    schema: 1,
    active_version: noVersion ? null : { source: "development", id: DEV_BUILD },
    official: { enabled: true, owner: "re-rac", repo: "rerac" },
    dev_versions: noVersion ? [] : [{ path: DEV_BUILD }, { path: `${HOME}/Downloads/rerac-old` }],
    ntsc_only: false,
    minimize_while_playing: false,
  };
  // Launcher-installed development versions (from "Install from zip…"), by folder name.
  const installedZips = new Set<string>(noVersion ? [] : ["0.1.0"]);
  let dataRoot = DEFAULT_ROOT;
  let installed: ExtractInfo | null = ["installed", "verifying", "exporting", "exported", "stale", "playing"].includes(scenario)
    ? { disc: "SCUS_971.99", data_format: 1, extractor_version: "0.1.0-dev", ntsc_only: false, files: FILES.length, bytes: TOTAL }
    : null;
  let job: (JobState & { cancelled: boolean }) | null = null;
  let running: string | null = scenario === "playing" ? "rac1" : null;
  let nextJob = 1;
  let lastExport: string | null = null;

  const evL = new Set<(p: EventPayload) => void>();
  const finL = new Set<(p: FinishedPayload) => void>();
  const exitL = new Set<(p: ExitedPayload) => void>();
  const sub = <T>(set: Set<T>, cb: T): Promise<Unlisten> => {
    set.add(cb);
    maybeAutoStart();
    return Promise.resolve(() => set.delete(cb));
  };

  const snapshot = (): AppSnapshot => ({
    launcher_version: "0.1.0",
    platform: /Mac/.test(navigator.userAgent) ? "macos" : /Win/.test(navigator.userAgent) ? "windows" : "linux",
    data_root: dataRoot,
    default_data_root: DEFAULT_ROOT,
    settings: structuredClone(settings),
    active: settings.active_version
      ? { ref: settings.active_version, version: devInfo(settings.active_version.id).manifest?.version ?? null, game: "rac1", data_format: devManifest().data_format, problem: null }
      : null,
    job: job ? { job: job.job, kind: job.kind, game: job.game } : null,
    game_running: running,
  });

  const emit = (kind: JobKind, id: number, event: ExtractorEvent) =>
    evL.forEach((cb) => cb({ job: id, kind, game: "rac1", event }));

  async function runJob(kind: JobKind, iso: string | null, opts: { from?: number; slow?: boolean; to?: string } = {}) {
    const id = nextJob++;
    job = { job: id, kind, game: "rac1", cancelled: false };
    const t0 = Date.now();
    const { err, disc: discLine } = iso ? outcome(iso) : { err: 0, disc: undefined };
    const finish = (status: FinishedPayload["status"], code: number, message: string) => {
      job = null;
      finL.forEach((cb) => cb({ job: id, kind, game: "rac1", status, code, message, elapsed_ms: Date.now() - t0 }));
    };
    const fail = (code: number) => {
      emit(kind, id, { type: "error", code, message: MESSAGES[code] ?? `error ${code}` });
      finish("error", code, MESSAGES[code] ?? `error ${code}`);
    };
    const failExport = (code: number) => {
      const m = `cannot write ${opts.to}/audio/levels/05/music/003.wav: permission denied`;
      emit(kind, id, { type: "error", code, message: m });
      finish("error", code, m);
    };
    const fail31 = () => {
      const m = `not enough free space at ${opts.to} for the export: about 3620 MiB needed, 1210 MiB available`;
      emit(kind, id, { type: "error", code: 31, message: m });
      finish("error", 31, m);
    };
    // Kick off asynchronously so the caller gets the id first, like the real backend.
    void (async () => {
      await sleep(30);
      if (kind === "extract") {
        if (err === 10 || err === 11) return fail(err);
        for (let i = 1; i <= 4; i++) {
          if (job?.cancelled) return finish("cancelled", 0, "Cancelled");
          emit(kind, id, { type: "progress", stage: "identify", done: i * 820_000, total: 3_280_000, file: "SCUS_971.99" });
          await sleep(90);
        }
        if (discLine) emit(kind, id, discLine);
        if (err === 20 || err === 21) return fail(err);
        const from = opts.from ?? 0;
        // Real extractions take 3-8 s at about 10 progress lines a second.
        const steps = opts.slow ? 2000 : 50;
        for (let s = Math.round(from * steps); s <= steps; s++) {
          if (job?.cancelled) return finish("cancelled", 0, "Cancelled");
          const done = Math.round((TOTAL * s) / steps);
          let acc = 0;
          const file = FILES.find(([, size]) => (acc += size) >= done)?.[0] ?? FILES[FILES.length - 1][0];
          emit(kind, id, { type: "progress", stage: "copy", done, total: TOTAL, file });
          if ((err === 30 || err === 31 || err === 40 || err === 99) && s > steps * 0.55) return fail(err);
          await sleep(opts.slow ? 400 : 100);
        }
      } else if (kind === "export") {
        // The real export takes about 10 s for everything (3.5 GB written), well under a second per level.
        const jobs = Array.from({ length: 19 }, (_, i) => `levels/${String(i).padStart(2, "0")}/core_data.bin`).concat(["global/sound_bank.bin", "global/all_text.bin"]);
        const total = 1_214_000_000;
        const steps = opts.slow ? 400 : 30;
        for (let s = 0; s <= steps; s++) {
          if (job?.cancelled) return finish("cancelled", 0, "Cancelled");
          const done = Math.round((total * s) / steps);
          emit(kind, id, { type: "progress", stage: "export", done, total, file: jobs[Math.min(jobs.length - 1, Math.floor((jobs.length * s) / steps))] });
          const fail = Number((params.get("export") ?? "").replace("fail", ""));
          if (fail && s > steps * 0.4) return fail === 31 ? fail31() : failExport(fail);
          await sleep(opts.slow ? 400 : 100);
        }
      } else {
        let done = 0;
        for (const [f, size] of FILES) {
          if (job?.cancelled) return finish("cancelled", 0, "Cancelled");
          done += size;
          emit(kind, id, { type: "progress", stage: "verify", done, total: TOTAL, file: f });
          await sleep(opts.slow ? 400 : 50);
        }
        if (params.get("verify") === "fail") return fail(40);
      }
      if (kind === "extract") {
        installed = { disc: "SCUS_971.99", data_format: 1, extractor_version: "0.1.0-dev", ntsc_only: settings.ntsc_only, files: FILES.length, bytes: TOTAL };
      }
      emit(kind, id, { type: "done", elapsed_ms: Date.now() - t0 });
      finish("ok", 0, kind === "extract" ? "Extraction complete." : kind === "export" ? `Exported to ${opts.to}` : "All files match.");
    })();
    return id;
  }

  let autoStarted = false;
  function maybeAutoStart() {
    if (autoStarted || evL.size === 0 || finL.size === 0) return;
    autoStarted = true;
    if (scenario === "extracting") void runJob("extract", `${HOME}/Games/Ratchet & Clank (USA).iso`, { from: 0.46, slow: true });
    if (scenario === "verifying") void runJob("verify", null, { slow: true });
    if (scenario === "exporting" || scenario === "exported") {
      lastExport = `${dataRoot}/games/rac1/data/exports`;
      void runJob("export", null, { slow: scenario === "exporting", to: lastExport });
    }
    if (scenario === "error21") void runJob("extract", `${HOME}/Games/Ratchet & Clank (Europe).iso`);
    if (scenario === "error20") void runJob("extract", `${HOME}/Games/notrac.iso`);
    if (params.get("autoplay") === "1") setTimeout(() => void self?.launchGame("rac1"), 200);
    if (scenario === "sequel") void runJob("extract", `${HOME}/Games/Ratchet & Clank 2 - Going Commando (USA).iso`);
  }

  const devInfo = (path: string): VersionInfo => {
    const ref: VersionRef = { source: "development", id: path };
    const managed = !path.startsWith("/");
    const good = managed || path === DEV_BUILD || path.includes("/dist/");
    const manifest = good ? { ...devManifest(), ...(managed ? { version: path } : {}) } : null;
    return {
      ref,
      path: managed ? `${dataRoot}/versions/development/${path}` : path,
      manifest,
      runtime: manifest ? { name: "rerac", version: manifest.version, game: "rac1", data_format: manifest.data_format } : null,
      problem: good ? null : `No rerac-manifest.json in ${path}`,
      active: settings.active_version?.id === path,
      managed,
    };
  };

  const status = (game: string): GameStatus => ({
    game,
    data_dir: `${dataRoot}/games/${game}/data`,
    installed: game === "rac1" && installed !== null,
    info: game === "rac1" ? installed : null,
    stale: scenario === "stale" && installed !== null,
    expected_format: settings.active_version ? devManifest().data_format : null,
    active_version: settings.active_version ? "0.1.0-dev" : null,
    job: job && job.game === game ? { job: job.job, kind: job.kind, game: job.game } : null,
    running: running === game,
    export_dir: lastExport ?? `${dataRoot}/games/${game}/data/exports`,
  });

  const busy = () => {
    if (job || running) throw "Wait for the running job or game to finish first.";
  };

  let self: Backend | null = null;
  return (self = {
    mode: "mock",
    getSnapshot: async () => snapshot(),
    gameStatus: async (game) => status(game),
    pickIso: async () => {
      await sleep(150);
      const name = params.get("iso") ?? "Ratchet & Clank (USA).iso";
      return `${HOME}/Games/${name}`;
    },
    startExtract: async (game, iso) => {
      if (game !== "rac1") throw "This game is not supported yet.";
      if (!settings.active_version) throw "No game version is active. Pick one in Settings → Version Management.";
      if (job) throw "Another extractor job is already running.";
      return runJob("extract", iso);
    },
    startVerify: async () => {
      if (job) throw "Another extractor job is already running.";
      return runJob("verify", null, { slow: false });
    },
    exportTarget: async (_game, picked) =>
      /\/(exports|rerac-rac1-exports)$/.test(picked) || picked.endsWith("/Empty") ? picked : `${picked}/rerac-rac1-exports`,
    startExport: async (game, to, what) => {
      if (game !== "rac1") throw "This game is not supported yet.";
      if (!installed) throw "The game data is not installed.";
      if (job) throw "Another extractor job is already running.";
      if (what.length === 0) throw "Choose at least one kind of asset to export.";
      lastExport = to;
      return runJob("export", null, { to, slow: false });
    },
    cancelJob: async () => {
      if (!job) return false;
      job.cancelled = true;
      return true;
    },
    uninstallGame: async () => {
      busy();
      installed = null;
    },
    launchGame: async (game) => {
      busy();
      running = game;
      // ?mock=installed&runtime=3|4 makes the fake game refuse to start, like the real runtime.
      const refuse = Number(params.get("runtime") ?? 0);
      const messages: Record<number, string> = {
        2: "error: --data-dir needs a value",
        101: "thread 'main' panicked at crates/rc-engine/src/main.rs: index out of bounds",
        3: `error: ${dataRoot}/games/${game}/data is not a complete ReRAC data folder (no toc.bin)`,
        4: "error: data_format 1 in extract-info.json does not match this build (2)",
      };
      setTimeout(
        () => {
          running = null;
          exitL.forEach((cb) =>
            cb({ game, code: refuse || 0, log: `${dataRoot}/logs/${game}-1790000000.log`, message: refuse && refuse !== 101 ? (messages[refuse] ?? null) : null }),
          );
        },
        refuse ? 300 : 4000,
      );
    },
    openFolder: async (which) => console.info("[mock] open folder", which),
    openLog: async (path) => console.info("[mock] reveal log", path),
    setMinimizeWhilePlaying: async (value) => {
      settings.minimize_while_playing = value;
      return snapshot();
    },
    openUrl: async (url) => console.info("[mock] open url", url),
    pickFolder: async (title) => {
      await sleep(120);
      return /build/i.test(title) ? `${HOME}/Repos/rerac/dist/dev` : /export/i.test(title) ? `${HOME}/Desktop` : "/Volumes/Games";
    },
    moveDataRoot: async (target) => {
      busy();
      dataRoot = target.endsWith("/rerac") ? target : `${target}/rerac`;
      return snapshot();
    },
    setNtscOnly: async (value) => {
      settings.ntsc_only = value;
      return snapshot();
    },
    listVersions: async () =>
      [...[...installedZips].sort().reverse(), ...settings.dev_versions.map((d) => d.path)].map((id) => ({ ...devInfo(id), runtime: null })),
    pickZip: async () => {
      await sleep(120);
      return `${HOME}/Downloads/${params.get("zip") ?? "rerac-0.2.0-macos-arm64.zip"}`;
    },
    installVersionZip: async (path) => {
      busy();
      await sleep(900);
      if (/bad/i.test(path)) throw "This archive is not a ReRAC build: there is no rerac-manifest.json at its top level.";
      const version = path.match(/rerac-([0-9][^-]*)/)?.[1] ?? "0.2.0";
      installedZips.add(version);
      return devInfo(version);
    },
    uninstallVersion: async (vref) => {
      busy();
      installedZips.delete(vref.id);
      if (settings.active_version?.id === vref.id) settings.active_version = null;
      return snapshot();
    },
    addDevVersion: async (path) => {
      await sleep(250);
      if (!settings.dev_versions.some((d) => d.path === path)) settings.dev_versions.push({ path });
      return devInfo(path);
    },
    validateVersion: async (vref) => {
      await sleep(250);
      return devInfo(vref.id);
    },
    removeDevVersion: async (path) => {
      settings.dev_versions = settings.dev_versions.filter((d) => d.path !== path);
      if (settings.active_version?.id === path) settings.active_version = null;
      return snapshot();
    },
    setActiveVersion: async (vref) => {
      busy();
      await sleep(250);
      const info = devInfo(vref.id);
      if (info.problem) throw info.problem;
      settings.active_version = vref;
      return snapshot();
    },
    officialReleases: async (): Promise<OfficialRelease[]> => {
      if (!settings.official.enabled) throw "Official releases are turned off.";
      await sleep(300);
      return [
        { version: "v0.2.0", date: "2026-11-02", changes: "Faster level loading · Hoverboard races · Fixes on Novalis", url: "", prerelease: false, asset: null, installed: false },
        { version: "v0.1.0", date: "2026-10-12", changes: "First playable build: Veldin to Kerwan", url: "", prerelease: false, asset: null, installed: true },
      ];
    },
    setOfficialConfig: async (enabled, owner, repo) => {
      settings.official = { enabled, owner, repo };
      return snapshot();
    },
    downloadOfficial: async () => {
      throw "Downloads are not available in the browser preview.";
    },
    onExtractorEvent: (cb) => sub(evL, cb),
    onExtractorFinished: (cb) => sub(finL, cb),
    onGameExited: (cb) => sub(exitL, cb),
    onDownloadProgress: (_cb: (p: DownloadProgress) => void) => Promise.resolve(() => {}),
    window: {
      minimize: async () => {},
      toggleMaximize: async () => {},
      close: async () => {},
    },
  });
}
