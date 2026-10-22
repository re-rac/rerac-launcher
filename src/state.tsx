import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { errorText, type Backend } from "./backend/api";
import type { AppSnapshot, DiscGame, FinishStatus, JobKind, Stage } from "./backend/contract";

export interface DiscView {
  serial: string;
  region: string;
  version: string;
  supported: boolean;
  game?: DiscGame;
  title?: string;
}

export interface JobView {
  job: number;
  kind: JobKind;
  game: string;
  stage: Stage | null;
  done: number;
  total: number;
  file: string;
  disc: DiscView | null;
  startedAt: number;
  /** When the copy stage began and how much was done then, for the ETA. */
  copyStart: { at: number; done: number } | null;
}

export interface JobResult {
  /** `launch`: the game exited with an error (2, 3, 4: refused to start; anything else: a
   * crash; code -1: killed by a signal). */
  kind: JobKind | "launch";
  game: string;
  status: FinishStatus;
  code: number;
  message: string;
  disc: DiscView | null;
  at: number;
  /** The game's log file (launch results). */
  log?: string;
}

export interface Toast {
  id: number;
  text: string;
  tone: "ok" | "warn" | "info";
}

interface Ctx {
  backend: Backend;
  snapshot: AppSnapshot | null;
  setSnapshot(s: AppSnapshot): void;
  refresh(): Promise<void>;
  job: JobView | null;
  results: Record<string, JobResult | undefined>;
  clearResult(game: string): void;
  /** Bumped whenever install state may have changed; screens refetch on change. */
  statusTick: number;
  bumpStatus(): void;
  toast(text: string, tone?: Toast["tone"]): void;
  toasts: Toast[];
  /** Runs an action and toasts its error. */
  run<T>(fn: () => Promise<T>): Promise<T | undefined>;
}

const LauncherContext = createContext<Ctx | null>(null);

export function useLauncher(): Ctx {
  const c = useContext(LauncherContext);
  if (!c) throw new Error("useLauncher outside provider");
  return c;
}

export function LauncherProvider({ backend, children }: { backend: Backend; children: ReactNode }) {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [job, setJob] = useState<JobView | null>(null);
  const [results, setResults] = useState<Record<string, JobResult | undefined>>({});
  const [statusTick, setStatusTick] = useState(0);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const toastId = useRef(0);
  // Disc lines by job id, recorded synchronously: an error can follow its disc line before React
  // has rendered the job state.
  const discs = useRef<Record<number, DiscView>>({});

  const toast = useCallback((text: string, tone: Toast["tone"] = "info") => {
    const id = ++toastId.current;
    setToasts((t) => [...t, { id, text, tone }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 4200);
  }, []);

  const refresh = useCallback(async () => {
    try {
      setSnapshot(await backend.getSnapshot());
    } catch (e) {
      toast(errorText(e), "warn");
    }
  }, [backend, toast]);

  const bumpStatus = useCallback(() => setStatusTick((t) => t + 1), []);

  useEffect(() => {
    let alive = true;
    const unlisten: (() => void)[] = [];
    (async () => {
      const snap = await backend.getSnapshot();
      if (!alive) return;
      setSnapshot(snap);
      if (snap.job) {
        setJob({ ...snap.job, stage: null, done: 0, total: 0, file: "", disc: null, startedAt: Date.now(), copyStart: null });
      }
      unlisten.push(
        await backend.onExtractorEvent((p) => {
          const e = p.event;
          if (e.type === "disc") {
            discs.current[p.job] = { serial: e.serial, region: e.region, version: e.version, supported: e.supported, game: e.game, title: e.title };
          }
          setJob((prev) => {
            const base: JobView =
              prev && prev.job === p.job
                ? prev
                : { job: p.job, kind: p.kind, game: p.game, stage: null, done: 0, total: 0, file: "", disc: null, startedAt: Date.now(), copyStart: null };
            const ev = p.event;
            switch (ev.type) {
              case "progress": {
                const copyStart =
                  ev.stage === "copy" && base.stage !== "copy" ? { at: Date.now(), done: ev.done } : base.copyStart;
                return { ...base, stage: ev.stage, done: ev.done, total: ev.total, file: ev.file, copyStart };
              }
              case "disc":
                return { ...base, disc: discs.current[p.job] ?? null };
              default:
                return base;
            }
          });
        }),
      );
      unlisten.push(
        await backend.onExtractorFinished((p) => {
          const disc = discs.current[p.job] ?? null;
          delete discs.current[p.job];
          setJob(null);
          setResults((r) => ({
            ...r,
            [p.game]: { kind: p.kind, game: p.game, status: p.status, code: p.code, message: p.message, disc, at: Date.now() },
          }));
          if (p.status === "cancelled")
            toast(p.kind === "extract" ? "Installation cancelled." : p.kind === "export" ? "Export cancelled." : "Verification cancelled.", "info");
          if (p.status === "ok" && p.kind === "verify") toast("All game files match.", "ok");
          if (p.status === "ok" && p.kind === "extract") toast("Game data installed. Your disc image is no longer needed.", "ok");
          setStatusTick((t) => t + 1);
          void backend.getSnapshot().then(setSnapshot);
        }),
      );
      unlisten.push(
        await backend.onGameExited((p) => {
          if (p.code !== 0) {
            const code = p.code ?? -1;
            const message = p.message ?? (p.code === null ? "The game was stopped by a signal." : `The game exited with code ${p.code}.`);
            setResults((r) => ({
              ...r,
              [p.game]: { kind: "launch", game: p.game, status: "error", code, message, disc: null, at: Date.now(), log: p.log },
            }));
          }
          setStatusTick((t) => t + 1);
          void backend.getSnapshot().then(setSnapshot);
        }),
      );
      unlisten.push(await backend.onLauncherUpdate(() => void backend.getSnapshot().then(setSnapshot)));
      // Unmounted while subscribing (StrictMode runs effects twice): drop what we added.
      if (!alive) unlisten.forEach((u) => u());
    })().catch((e) => toast(errorText(e), "warn"));
    return () => {
      alive = false;
      unlisten.forEach((u) => u());
    };
  }, [backend, toast]);

  const clearResult = useCallback((game: string) => setResults((r) => ({ ...r, [game]: undefined })), []);

  const run = useCallback(
    async <T,>(fn: () => Promise<T>) => {
      try {
        return await fn();
      } catch (e) {
        toast(errorText(e), "warn");
        return undefined;
      }
    },
    [toast],
  );

  return (
    <LauncherContext.Provider
      value={{ backend, snapshot, setSnapshot, refresh, job, results, clearResult, statusTick, bumpStatus, toast, toasts, run }}
    >
      {children}
    </LauncherContext.Provider>
  );
}
