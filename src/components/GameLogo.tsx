import type { GameDef } from "../lib/games";

/** The game's logo art, or its title as a wordmark while there is none. */
export function GameLogo({ game, className }: { game: GameDef; className?: string }) {
  if (game.logo) return <img className={className} src={game.logo} alt={game.title} draggable={false} />;
  return (
    <span className={`game-wordmark ${className ?? ""}`} style={{ ["--accent" as string]: game.accent }}>
      {game.title}
    </span>
  );
}
