import { useCallback, useEffect, useState } from "react";
import { errorText, type Backend } from "../../backend/api";
import type { OfficialRelease, VersionInfo, VersionRef } from "../../backend/contract";
import { IconCheck, IconDownload, IconFolder, IconInfo, IconPlus, IconRefresh, IconTrash, IconWarn } from "../../components/Icons";
import { Modal } from "../../components/Modal";
import { SOURCES } from "../../lib/sources";
import { useLauncher, type Toast } from "../../state";

/** After switching versions: warn when the installed game data has another data_format. */
async function warnIfDataMismatch(backend: Backend, toast: (text: string, tone?: Toast["tone"]) => void) {
  try {
    const st = await backend.gameStatus("rac1");
    if (st.installed && st.stale && st.info) {
      toast(
        `The installed game data is format ${st.info.data_format}, but ReRAC ${st.active_version ?? ""} needs format ${st.expected_format}. Re-extract before playing.`,
        "warn",
      );
    }
  } catch {
    // The game screen shows the same state.
  }
}

export function Versions({ sub, go }: { sub?: string; go: (p: string) => void }) {
  const current = SOURCES.find((s) => s.id === sub && !s.disabledNote) ?? SOURCES[0];
  return (
    <div className="stack">
      <nav className="subtabs" role="tablist">
        {SOURCES.map((s) => (
          <button
            key={s.id}
            role="tab"
            aria-selected={current.id === s.id}
            disabled={!!s.disabledNote}
            className={`subtab ${current.id === s.id ? "is-active" : ""}`}
            onClick={() => go(`settings/versions/${s.id}`)}
            title={s.disabledNote ? `${s.label}: ${s.disabledNote.toLowerCase()}` : s.blurb}
          >
            {s.label}
            {s.disabledNote && <span className="soon-tag">{s.disabledNote}</span>}
          </button>
        ))}
      </nav>
      {current.id === "official" ? <Official /> : <Development />}
    </div>
  );
}

function ActiveBadge() {
  return (
    <span className="badge is-ok">
      <IconCheck size={13} /> Active
    </span>
  );
}

// ---------------------------------------------------------------------------------------------

function Official() {
  const { backend, snapshot, setSnapshot, run, toast, bumpStatus } = useLauncher();
  const [busy, setBusy] = useState<string | null>(null);
  const cfg = snapshot?.settings.official;
  const [rows, setRows] = useState<OfficialRelease[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [owner, setOwner] = useState(cfg?.owner ?? "");
  const [repo, setRepo] = useState(cfg?.repo ?? "");
  const enabled = !!cfg?.enabled;

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setRows(await backend.officialReleases());
    } catch (e) {
      setError(errorText(e));
    }
    setLoading(false);
  }, [backend]);

  useEffect(() => {
    if (enabled) void load();
  }, [enabled, load]);

  async function save(nextEnabled: boolean) {
    const s = await run(() => backend.setOfficialConfig(nextEnabled, owner.trim(), repo.trim()));
    if (s) setSnapshot(s);
  }

  async function download(version: string) {
    setBusy(version);
    const info = await run(() => backend.downloadOfficial(version));
    setBusy(null);
    if (info) {
      toast(`Installed ReRAC ${info.manifest?.version ?? version}.`, "ok");
      await load();
    }
  }

  async function activate(version: string) {
    setBusy(version);
    const s = await run(() => backend.setActiveVersion({ source: "official", id: version }));
    setBusy(null);
    if (s) {
      setSnapshot(s);
      bumpStatus();
      toast("Active version changed.", "ok");
      await warnIfDataMismatch(backend, toast);
    }
  }

  const activeOfficial = snapshot?.settings.active_version?.source === "official" ? snapshot.settings.active_version.id : null;

  return (
    <>
      {!enabled && (
        <div className="notice plate">
          <IconInfo size={22} />
          <div>
            <strong>Official releases are turned off.</strong>
            <p className="muted">
              Turn them on under Release feed below, or use a build from the Development tab.
            </p>
          </div>
        </div>
      )}
      <div className={`card plate ${enabled ? "" : "is-disabled"}`} aria-disabled={!enabled}>
        <div className="toolbar">
          <span className="muted small mono">
            github.com/{cfg?.owner}/{cfg?.repo}
          </span>
          <div className="row">
            <button className="btn btn-ghost btn-sm" disabled={!enabled || loading} onClick={() => void load()}>
              <IconRefresh size={16} /> {loading ? "Refreshing…" : "Refresh"}
            </button>
            <button className="btn btn-ghost btn-sm" disabled={!enabled} onClick={() => void run(() => backend.openFolder("source:official"))}>
              <IconFolder size={16} /> Open folder
            </button>
          </div>
        </div>
        <table className="vtable">
          <thead>
            <tr>
              <th style={{ width: "18%" }}>Version</th>
              <th style={{ width: "16%" }}>Date</th>
              <th>Changes</th>
              <th style={{ width: "1%" }} />
            </tr>
          </thead>
          <tbody>
            {enabled && error && (
              <tr>
                <td colSpan={4} className="vt-empty is-warn">
                  <IconWarn size={16} /> {error}
                </td>
              </tr>
            )}
            {enabled && !error && rows?.length === 0 && (
              <tr>
                <td colSpan={4} className="vt-empty">
                  No releases published yet.
                </td>
              </tr>
            )}
            {!enabled &&
              [0, 1, 2].map((i) => (
                <tr key={i} className="vt-ghost">
                  <td>
                    <i style={{ width: "60%" }} />
                  </td>
                  <td>
                    <i style={{ width: "70%" }} />
                  </td>
                  <td>
                    <i style={{ width: `${80 - i * 15}%` }} />
                  </td>
                  <td />
                </tr>
              ))}
            {enabled &&
              rows?.map((r) => (
                <tr key={r.version}>
                  <td className="mono">
                    {r.version}
                    {r.prerelease && <span className="badge">pre</span>}
                  </td>
                  <td className="mono muted">{r.date}</td>
                  <td>{r.changes}</td>
                  <td className="vt-actions">
                    {r.installed ? (
                      activeOfficial === r.version ? (
                        <ActiveBadge />
                      ) : (
                        <button className="btn btn-ghost btn-sm" disabled={busy !== null} onClick={() => void activate(r.version)}>
                          {busy === r.version ? "Checking…" : "Set active"}
                        </button>
                      )
                    ) : (
                      <button className="btn btn-primary btn-sm btn-icon" disabled={!r.asset || busy !== null} title={r.asset ? `Download and install ${r.asset.name}` : "No download for this system"} onClick={() => void download(r.version)}>
                        {busy === r.version ? <IconRefresh size={16} /> : <IconDownload size={16} />}
                      </button>
                    )}
                  </td>
                </tr>
              ))}
          </tbody>
        </table>
      </div>
      <details className="card plate advanced">
        <summary>Release feed</summary>
        <p className="muted small">GitHub repository the Official list reads from.</p>
        <div className="form-row">
          <label>
            Owner
            <input value={owner} onChange={(e) => setOwner(e.target.value)} spellCheck={false} />
          </label>
          <label>
            Repository
            <input value={repo} onChange={(e) => setRepo(e.target.value)} spellCheck={false} />
          </label>
          <label className="toggle">
            <input type="checkbox" checked={enabled} onChange={(e) => void save(e.target.checked)} />
            <span className="toggle-ui" aria-hidden="true" />
            <span>Enabled</span>
          </label>
          <button className="btn btn-ghost btn-sm" onClick={() => void save(enabled)}>
            Save
          </button>
        </div>
      </details>
    </>
  );
}

// ---------------------------------------------------------------------------------------------

function Development() {
  const { backend, snapshot, setSnapshot, run, toast, bumpStatus } = useLauncher();
  const [list, setList] = useState<VersionInfo[]>([]);
  const [checked, setChecked] = useState<Record<string, VersionInfo>>({});
  const [pending, setPending] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState<VersionInfo | null>(null);

  const validate = useCallback(
    async (vref: VersionRef) => {
      setPending(vref.id);
      try {
        const info = await backend.validateVersion(vref);
        setChecked((c) => ({ ...c, [vref.id]: info }));
      } catch (e) {
        toast(errorText(e), "warn");
      }
      setPending(null);
    },
    [backend, toast],
  );

  const reload = useCallback(async () => {
    const all = await backend.listVersions();
    const dev = all.filter((v) => v.ref.source === "development");
    setList(dev);
    return dev;
  }, [backend]);

  useEffect(() => {
    void reload().then(async (dev) => {
      for (const v of dev) if (!v.problem) await validate(v.ref);
    });
  }, [reload, validate, snapshot?.settings.active_version?.id]);

  async function add() {
    const path = await run(() => backend.pickFolder("Choose a ReRAC build folder (with rerac-manifest.json)"));
    if (!path) return;
    setPending(path);
    const info = await run(() => backend.addDevVersion(path));
    setPending(null);
    if (!info) return;
    setChecked((c) => ({ ...c, [info.ref.id]: info }));
    await reload();
    if (info.problem) toast("Added, but the build did not validate. See the details below.", "warn");
    else toast(`Added ReRAC ${info.manifest?.version}.`, "ok");
  }

  async function installZip() {
    const path = await run(() => backend.pickZip());
    if (!path) return;
    setInstalling(true);
    const info = await run(() => backend.installVersionZip(path));
    setInstalling(false);
    if (!info) return;
    setChecked((c) => ({ ...c, [info.ref.id]: info }));
    await reload();
    toast(`Installed ReRAC ${info.manifest?.version}.${info.active ? "" : " Set it active to use it."}`, "ok");
  }

  async function activate(vref: VersionRef) {
    setPending(vref.id);
    const s = await run(() => backend.setActiveVersion(vref));
    setPending(null);
    if (s) {
      setSnapshot(s);
      bumpStatus();
      toast("Active version changed.", "ok");
      await warnIfDataMismatch(backend, toast);
    }
  }

  async function remove(v: VersionInfo) {
    setConfirmDelete(null);
    const s = await run(() => (v.managed ? backend.uninstallVersion(v.ref) : backend.removeDevVersion(v.path)));
    if (s) {
      setSnapshot(s);
      bumpStatus();
      await reload();
      if (v.managed) toast(`Deleted ReRAC ${v.manifest?.version ?? v.ref.id}.`, "info");
    }
  }

  const activeId = snapshot?.settings.active_version?.source === "development" ? snapshot.settings.active_version.id : null;

  return (
    <>
      <div className="card plate">
        <div className="card-head">
          <div>
            <h3>Local builds</h3>
            <p className="muted">
              Install a build <code>.zip</code> (it is unpacked into the data folder), or add a build folder that contains{" "}
              <code>rerac-manifest.json</code> (used in place). The launcher checks the manifest, finds the runtime and
              extractor, and asks the runtime for <code>--version-json</code>.
            </p>
          </div>
          <div className="row">
            <button className="btn btn-primary btn-sm" onClick={installZip} disabled={pending !== null || installing}>
              <IconDownload size={16} /> {installing ? "Installing…" : "Install from zip…"}
            </button>
            <button className="btn btn-ghost btn-sm" onClick={add} disabled={pending !== null || installing}>
              <IconPlus size={16} /> Add build folder…
            </button>
          </div>
        </div>
      </div>
      {list.length === 0 ? (
        <div className="empty plate">
          <p>No development builds yet.</p>
          <p className="muted small">
            Tip: <code>dev/mock</code> in the launcher repo is a ready-made mock build for trying the whole flow.
          </p>
        </div>
      ) : (
        <ul className="dev-list">
          {list.map((v) => {
            const info = checked[v.ref.id] ?? v;
            const isActive = activeId === v.ref.id;
            const problem = info.problem;
            const busy = pending === v.ref.id;
            return (
              <li key={v.ref.id} className={`dev-row plate ${isActive ? "is-active" : ""} ${problem ? "is-bad" : ""}`}>
                <div className="dev-main">
                  <div className="dev-title">
                    <span>{info.manifest ? `ReRAC ${info.manifest.version}` : "Unknown build"}</span>
                    {info.manifest && <span className="badge">{info.manifest.game.toUpperCase()}</span>}
                    {v.managed && <span className="badge">Installed</span>}
                    {isActive && <ActiveBadge />}
                  </div>
                  <div className="dev-path mono" title={v.path}>
                    {v.path}
                  </div>
                  <div className={`dev-status ${problem ? "is-warn" : info.runtime ? "is-ok" : ""}`}>
                    {busy ? (
                      "Checking…"
                    ) : problem ? (
                      <>
                        <IconWarn size={15} /> {problem}
                      </>
                    ) : info.runtime ? (
                      <>
                        <IconCheck size={15} /> Runtime answered v{info.runtime.version} · data format {info.runtime.data_format}
                      </>
                    ) : (
                      "Not checked yet"
                    )}
                  </div>
                </div>
                <div className="dev-actions">
                  {!isActive && (
                    <button className="btn btn-primary btn-sm" disabled={!!problem || busy} onClick={() => void activate(v.ref)}>
                      Set active
                    </button>
                  )}
                  <button className="btn btn-ghost btn-sm btn-icon" title="Check again" disabled={busy} onClick={() => void validate(v.ref)}>
                    <IconRefresh size={16} />
                  </button>
                  <button className="btn btn-ghost btn-sm btn-icon" title="Open folder" onClick={() => void run(() => backend.openFolder(`path:${v.path}`))}>
                    <IconFolder size={16} />
                  </button>
                  {v.managed ? (
                    <button className="btn btn-ghost btn-sm btn-icon is-danger" title="Delete this installed build" onClick={() => setConfirmDelete(info)}>
                      <IconTrash size={16} />
                    </button>
                  ) : (
                    <button className="btn btn-ghost btn-sm btn-icon is-danger" title="Remove from list (files stay)" onClick={() => void remove(info)}>
                      <IconTrash size={16} />
                    </button>
                  )}
                </div>
              </li>
            );
          })}
        </ul>
      )}
      {confirmDelete && (
        <Modal
          title={`Delete ReRAC ${confirmDelete.manifest?.version ?? confirmDelete.ref.id}?`}
          confirmLabel="Delete"
          danger
          onConfirm={() => void remove(confirmDelete)}
          onClose={() => setConfirmDelete(null)}
        >
          <p>
            This deletes the installed build in <code>{confirmDelete.path}</code>. Your game data, saves and settings stay.
            You can install it again from its zip.
          </p>
        </Modal>
      )}
    </>
  );
}
