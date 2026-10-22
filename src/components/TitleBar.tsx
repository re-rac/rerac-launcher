import { useLauncher } from "../state";
import logo from "../assets/brand/rerac-logo-480.png";
import { IconClose, IconMinus, IconSquare } from "./Icons";

export function TitleBar() {
  const { snapshot, backend } = useLauncher();
  const mac = snapshot?.platform === "macos";
  return (
    <header className={`titlebar ${mac ? "is-mac" : ""}`} data-tauri-drag-region>
      {mac && backend.mode === "mock" && (
        <span className="tb-fake-lights" aria-hidden="true">
          <i />
          <i />
          <i />
        </span>
      )}
      <div className="tb-brand" data-tauri-drag-region>
        <img className="tb-logo" src={logo} alt="ReRAC" draggable={false} data-tauri-drag-region />
        <span className="tb-name" data-tauri-drag-region>
          Launcher
        </span>
      </div>
      <div className="tb-spacer" data-tauri-drag-region />
      <div className="tb-chips" data-tauri-drag-region>
        <span className="tb-chip" data-tauri-drag-region>
          <em>Launcher</em> v{snapshot?.launcher_version ?? "…"}
        </span>
      </div>
      {!mac && (
        <div className="tb-controls">
          <button aria-label="Minimize" onClick={() => void backend.window.minimize()}>
            <IconMinus size={16} />
          </button>
          <button aria-label="Maximize" onClick={() => void backend.window.toggleMaximize()}>
            <IconSquare size={14} />
          </button>
          <button aria-label="Close" className="is-close" onClick={() => void backend.window.close()}>
            <IconClose size={16} />
          </button>
        </div>
      )}
    </header>
  );
}
