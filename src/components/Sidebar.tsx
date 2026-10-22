import { GAMES } from "../lib/games";
import { GameLogo } from "./GameLogo";
import { IconGear, IconHelp } from "./Icons";

export function Sidebar({ route, go }: { route: string[]; go: (p: string) => void }) {
  const [section, sub] = route;
  return (
    <nav className="sidebar" aria-label="Games">
      <div className="sb-games">
        {GAMES.map((g) => {
          const active = section === "game" && sub === g.id;
          return (
            <button
              key={g.id}
              className={`sb-game ${active ? "is-active" : ""} ${g.supported ? "" : "is-later"}`}
              onClick={() => go(`game/${g.id}`)}
              aria-current={active ? "page" : undefined}
              title={g.supported ? g.title : `${g.title} — coming later`}
            >
              <span className="sb-rail" aria-hidden="true" />
              <GameLogo game={g} />
            </button>
          );
        })}
      </div>
      <div className="sb-bottom">
        <button
          className={`sb-icon ${section === "settings" ? "is-active" : ""}`}
          onClick={() => go("settings/folders")}
          aria-label="Settings"
          title="Settings"
        >
          <IconGear size={22} />
        </button>
        <button
          className={`sb-icon ${section === "help" ? "is-active" : ""}`}
          onClick={() => go("help")}
          aria-label="Help"
          title="Help"
        >
          <IconHelp size={22} />
        </button>
      </div>
    </nav>
  );
}
