import { useCallback, useEffect, useRef, useState } from "react";
import { errorText } from "../backend/api";
import type { OfficialRelease, VersionInfo, VersionRef } from "../backend/contract";
import type { GameDef } from "../lib/games";
import { useLauncher } from "../state";
import { IconCheck, IconDownload, IconGear } from "./Icons";

/** How a version reads in the picker: "v0.3.6" for a release, "Dev 0.3.7-dev" for a local build. */
export function versionLabel(v: VersionInfo): string {
  const version = v.manifest?.version ?? v.ref.id;
  if (v.ref.source === "official") return version.startsWith("v") ? version : `v${version}`;
  return `Dev ${version}`;
}

/** The newest release of the feed that is not a prerelease. */
function newest(rows: OfficialRelease[] | null): OfficialRelease | null {
  return rows?.find((r) => !r.prerelease) ?? null;
}

/**
 * The game's version, picked right above Play (like a game-version selector in a store
 * launcher). Lists every installed build of this game; when the feed has a newer release that is
 * not installed, offers it as an update. Nothing downloads unless the player picks it.
 */
export function VersionPicker({ game, disabled, go }: { game: GameDef; disabled: boolean; go: (p: string) => void }) {
  const { backend, snapshot, setSnapshot, run, toast, bumpStatus, statusTick } = useLauncher();
  const [open, setOpen] = useState(false);
  const [versions, setVersions] = useState<VersionInfo[]>([]);
  const [releases, setReleases] = useState<OfficialRelease[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const active = snapshot?.actives[game.id] ?? null;
  const feed = snapshot?.feeds[game.id] ?? null;

  const load = useCallback(async () => {
    const all = await backend.listVersions().catch(() => [] as VersionInfo[]);
    setVersions(all.filter((v) => v.manifest?.game === game.id && !v.problem));
  }, [backend, game.id]);

  useEffect(() => {
    void load();
  }, [load, statusTick, active?.ref.id]);

  // The feed is read once per screen; a private or missing repository just shows no update.
  useEffect(() => {
    let alive = true;
    if (!feed?.enabled) {
      setReleases(null);
      return;
    }
    backend
      .officialReleases(game.id)
      .then((r) => alive && setReleases(r))
      .catch(() => alive && setReleases(null));
    return () => {
      alive = false;
    };
  }, [backend, game.id, feed?.enabled, feed?.owner, feed?.repo]);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const key = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", key);
    };
  }, [open]);

  const latest = newest(releases);
  const update = latest && !latest.installed && latest.asset ? latest : null;
  const current = versions.find((v) => v.ref.source === active?.ref.source && v.ref.id === active?.ref.id) ?? null;
  const isCurrent = (r: VersionRef) => active?.ref.source === r.source && active?.ref.id === r.id;

  async function choose(vref: VersionRef, label: string) {
    setOpen(false);
    if (isCurrent(vref)) return;
    setBusy(vref.id);
    const s = await run(() => backend.setActiveVersion(vref));
    setBusy(null);
    if (!s) return;
    setSnapshot(s);
    bumpStatus();
    toast(`${game.title} now runs ${label}.`, "ok");
  }

  async function install(r: OfficialRelease) {
    setOpen(false);
    setBusy(r.version);
    setProgress(0);
    const unlisten = await backend.onDownloadProgress((p) => {
      if (p.game === game.id && p.version === r.version && p.total > 0) setProgress(p.done / p.total);
    });
    try {
      const info = await backend.downloadOfficial(game.id, r.version);
      const s = await backend.setActiveVersion(info.ref);
      setSnapshot(s);
      bumpStatus();
      toast(`Installed ${r.version}. ${game.title} now runs it.`, "ok");
      await load();
      setReleases((rows) => rows?.map((x) => (x.version === r.version ? { ...x, installed: true, id: info.ref.id } : x)) ?? null);
    } catch (e) {
      toast(errorText(e), "warn");
    } finally {
      unlisten();
      setBusy(null);
      setProgress(null);
    }
  }

  const label = busy
    ? progress !== null
      ? `Downloading… ${Math.round(progress * 100)}%`
      : "Switching…"
    : current
      ? versionLabel(current)
      : active?.version
        ? `v${active.version}`
        : "Choose a version";

  return (
    <div className="vpick" ref={ref}>
      <span className="vpick-label">Version</span>
      <button
        className={`vpick-btn ${open ? "is-open" : ""} ${!active ? "is-empty" : ""}`}
        aria-haspopup="listbox"
        aria-expanded={open}
        disabled={disabled || busy !== null}
        onClick={() => setOpen((o) => !o)}
      >
        <span className="vpick-current">{label}</span>
        {update && !busy && <span className="badge is-accent">Update</span>}
        <span className="vpick-caret" aria-hidden="true" />
      </button>
      {open && (
        <ul className="menu-pop plate vpick-pop" role="listbox">
          {update && (
            <li>
              <button role="option" aria-selected={false} onClick={() => void install(update)}>
                <IconDownload size={18} />
                <span className="vpick-item">
                  <span>{update.version}</span>
                  <span className="vpick-sub">New · download and switch</span>
                </span>
              </button>
            </li>
          )}
          {versions.map((v) => {
            const sel = isCurrent(v.ref);
            return (
              <li key={`${v.ref.source}:${v.ref.id}`}>
                <button role="option" aria-selected={sel} className={sel ? "is-selected" : ""} onClick={() => void choose(v.ref, versionLabel(v))}>
                  {sel ? <IconCheck size={18} /> : <span className="vpick-gap" />}
                  <span className="vpick-item">
                    <span>{versionLabel(v)}</span>
                    {v.ref.source === "development" && <span className="vpick-sub">Local build</span>}
                  </span>
                </button>
              </li>
            );
          })}
          {versions.length === 0 && !update && <li className="vpick-none">No builds of this game installed yet.</li>}
          <li className="sep">
            <button onClick={() => go(`settings/versions/official/${game.id}`)}>
              <IconGear size={18} />
              <span>Manage versions…</span>
            </button>
          </li>
        </ul>
      )}
    </div>
  );
}
