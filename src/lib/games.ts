import type { GameId } from "../backend/contract";

// Art is optional: any src/assets/games/<id>-logo.* and <id>-bg.{webp,png,jpg,jpeg,avif} is
// picked up at build time. A missing background falls back to the drawn backdrop
// (components/Backdrop.tsx), a missing logo to the title as a wordmark (GameLogo).
const art = import.meta.glob("../assets/games/*-{logo,bg}.{webp,png,jpg,jpeg,avif,svg}", {
  eager: true,
  import: "default",
}) as Record<string, string>;

function artFor(id: GameId, kind: "logo" | "bg"): string | null {
  const key = Object.keys(art).find((k) => k.split("/").pop()!.startsWith(`${id}-${kind}.`));
  return key ? art[key] : null;
}

export interface GameDef {
  id: GameId;
  title: string;
  subtitle: string;
  year: number;
  logo: string | null;
  bg: string | null;
  /** Hue used by the drawn backdrop. */
  accent: string;
  supported: boolean;
}

export const GAMES: GameDef[] = [
  { id: "rac1", title: "Ratchet & Clank", subtitle: "Ratchet & Clank", year: 2002, logo: artFor("rac1", "logo"), bg: artFor("rac1", "bg"), accent: "#f28c28", supported: true },
  { id: "rac2", title: "Going Commando", subtitle: "Ratchet & Clank 2", year: 2003, logo: artFor("rac2", "logo"), bg: artFor("rac2", "bg"), accent: "#4fb8ff", supported: true },
  { id: "rac3", title: "Up Your Arsenal", subtitle: "Ratchet & Clank 3", year: 2004, logo: artFor("rac3", "logo"), bg: artFor("rac3", "bg"), accent: "#ffcf3a", supported: false },
  { id: "rac4", title: "Deadlocked", subtitle: "Ratchet: Deadlocked", year: 2005, logo: artFor("rac4", "logo"), bg: artFor("rac4", "bg"), accent: "#e8433a", supported: false },
];

export const gameById = (id: string) => GAMES.find((g) => g.id === id);
