import { useState } from "react";
import { IconFolder } from "../../components/Icons";
import { Modal } from "../../components/Modal";
import { useLauncher } from "../../state";

export function Folders() {
  const { backend, snapshot, setSnapshot, run, toast, bumpStatus } = useLauncher();
  const [target, setTarget] = useState<string | null>(null);
  const [moving, setMoving] = useState(false);
  if (!snapshot) return null;
  const root = snapshot.data_root;
  const isDefault = root === snapshot.default_data_root;
  const busy = !!snapshot.job || !!snapshot.game_running;
  const sep = root.includes("\\") ? "\\" : "/";

  async function chooseTarget() {
    const t = await run(() => backend.pickFolder("Choose where to keep ReRAC's data"));
    if (t) setTarget(t);
  }
  async function move() {
    if (!target) return;
    setMoving(true);
    const snap = await run(() => backend.moveDataRoot(target));
    setMoving(false);
    setTarget(null);
    if (snap) {
      setSnapshot(snap);
      bumpStatus();
      toast("Data folder moved.", "ok");
    }
  }

  const dest = target ? (target.endsWith(`${sep}rerac`) ? target : `${target}${sep}rerac`) : "";
  return (
    <div className="stack">
      <div className="card plate">
        <div className="card-head">
          <div>
            <h3>Data folder</h3>
            <p className="muted">Game builds, the data extracted from your disc (about 4.5 GB), logs and launcher settings.</p>
          </div>
          <span className={`badge ${isDefault ? "" : "is-accent"}`}>{isDefault ? "Default" : "Custom"}</span>
        </div>
        <div className="path-field mono" title={root}>
          {root}
        </div>
        <div className="row">
          <button className="btn btn-ghost btn-sm" onClick={() => void run(() => backend.openFolder("root"))}>
            <IconFolder size={16} /> Open
          </button>
          <button className="btn btn-primary btn-sm" onClick={chooseTarget} disabled={busy || moving}>
            {moving ? "Moving…" : "Move…"}
          </button>
          {busy && <span className="muted small">Moving is available when nothing is running.</span>}
        </div>
        <ul className="tree mono">
          <li>
            <b>versions{sep}</b>
            <span>game builds by source (official, development, later mods)</span>
          </li>
          <li>
            <b>games{sep}rac1{sep}data{sep}</b>
            <span>extracted from your disc</span>
          </li>
          <li>
            <b>logs{sep}</b>
            <span>extractor and game logs</span>
          </li>
          <li>
            <b>settings{sep}launcher.json</b>
            <span>these settings</span>
          </li>
        </ul>
        {!isDefault && <p className="muted small">Default location: {snapshot.default_data_root}</p>}
      </div>

      <div className="card plate">
        <h3>Extraction</h3>
        <label className="toggle">
          <input
            type="checkbox"
            checked={snapshot.settings.ntsc_only}
            onChange={(e) => void run(() => backend.setNtscOnly(e.target.checked)).then((s) => s && setSnapshot(s))}
          />
          <span className="toggle-ui" aria-hidden="true" />
          <span>
            <b>Skip PAL-only data (Ratchet &amp; Clank)</b>
            <span className="muted"> — smaller install. Anything skipped only comes back by re-extracting from the disc.</span>
          </span>
        </label>
      </div>

      <div className="card plate">
        <h3>While playing</h3>
        <label className="toggle">
          <input
            type="checkbox"
            checked={snapshot.settings.minimize_while_playing}
            onChange={(e) => void run(() => backend.setMinimizeWhilePlaying(e.target.checked)).then((s) => s && setSnapshot(s))}
          />
          <span className="toggle-ui" aria-hidden="true" />
          <span>
            <b>Minimise the launcher</b>
            <span className="muted"> — it comes back when the game exits. Off: the launcher stays open.</span>
          </span>
        </label>
      </div>

      <div className="card plate">
        <div className="card-head">
          <div>
            <h3>Logs</h3>
            <p className="muted">Every extraction and game session writes a log file.</p>
          </div>
          <button className="btn btn-ghost btn-sm" onClick={() => void run(() => backend.openFolder("logs"))}>
            <IconFolder size={16} /> Open logs
          </button>
        </div>
      </div>

      {target && (
        <Modal title="Move the data folder?" confirmLabel="Move" onConfirm={move} onClose={() => setTarget(null)}>
          <p>Everything in the data folder moves to:</p>
          <p className="path-field mono">{dest}</p>
          <p className="muted">This can take a while if the new folder is on another drive.</p>
        </Modal>
      )}
    </div>
  );
}
