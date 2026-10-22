import { useState } from "react";
import { useLauncher } from "../state";
import { IconDownload } from "./Icons";

/** "A new launcher is ready" strip under the title bar; the update installs only when the player restarts. */
export function UpdateBanner() {
  const { backend, snapshot, run } = useLauncher();
  const [restarting, setRestarting] = useState(false);
  const update = snapshot?.launcher_update;
  if (!update) return null;
  const busy = !!snapshot?.game_running || !!snapshot?.job;

  async function restart() {
    setRestarting(true);
    await run(() => backend.installLauncherUpdate());
    setRestarting(false);
  }

  return (
    <div className="update-banner" role="status">
      <IconDownload size={16} />
      <span>
        Launcher <strong>v{update.version}</strong> is ready.
        {busy && <span className="muted"> It installs once the game or job has finished.</span>}
      </span>
      <button className="btn btn-primary btn-sm" disabled={busy || restarting} onClick={() => void restart()}>
        {restarting ? "Restarting…" : "Restart to update"}
      </button>
    </div>
  );
}
