import { Folders } from "./Folders";
import { Versions } from "./Versions";

const TABS = [
  { id: "folders", label: "Folders" },
  { id: "versions", label: "Version Management" },
];

export function Settings({ route, go }: { route: string[]; go: (p: string) => void }) {
  const tab = route[1] === "versions" ? "versions" : "folders";
  return (
    <section className="page settings">
      <header className="page-head">
        <h1>Settings</h1>
        <nav className="tabs" role="tablist">
          {TABS.map((t) => (
            <button
              key={t.id}
              role="tab"
              aria-selected={tab === t.id}
              className={`tab ${tab === t.id ? "is-active" : ""}`}
              onClick={() => go(`settings/${t.id}`)}
            >
              {t.label}
            </button>
          ))}
        </nav>
      </header>
      <div className="page-body">{tab === "folders" ? <Folders /> : <Versions sub={route[2]} game={route[3]} go={go} />}</div>
    </section>
  );
}
