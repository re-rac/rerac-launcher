import { Backdrop } from "../components/Backdrop";
import { GameLogo } from "../components/GameLogo";
import { Rivet } from "../components/Icons";
import type { GameDef } from "../lib/games";

export function ComingLater({ game }: { game: GameDef }) {
  return (
    <section className="coming">
      <Backdrop game={game} mode="blur" />
      <div className="coming-inner">
        <GameLogo game={game} className="coming-logo" />
        <div className="coming-plate plate">
          <Rivet className="rivet tl" />
          <Rivet className="rivet tr" />
          <Rivet className="rivet bl" />
          <Rivet className="rivet br" />
          <span className="coming-kicker">{game.subtitle}</span>
          <strong>Coming later</strong>
          <p>
            ReRAC is rebuilding the first game from the ground up. {game.title} support follows once Ratchet & Clank
            is complete.
          </p>
        </div>
      </div>
    </section>
  );
}
