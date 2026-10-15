import type { GraphicsBackend, Settings } from "../backend/contract";

// Graphics backends offered in a game's ⋯ menu. The launcher passes the choice to the game as
// WGPU_BACKEND; "auto" passes nothing. Keep `platforms` in step with GraphicsBackend::available_on.
export interface GraphicsBackendDef {
  id: GraphicsBackend;
  label: string;
  hint: string;
  /** `snapshot.platform` values it works on; absent: everywhere. */
  platforms?: string[];
}

export const GRAPHICS_BACKENDS: GraphicsBackendDef[] = [
  { id: "auto", label: "Automatic", hint: "the game picks, usually Vulkan" },
  { id: "vulkan", label: "Vulkan", hint: "fastest level loading", platforms: ["windows", "linux"] },
  { id: "dx12", label: "DirectX 12", hint: "try this if the game window stays black", platforms: ["windows"] },
];

export function graphicsBackendsFor(platform: string | undefined): GraphicsBackendDef[] {
  return GRAPHICS_BACKENDS.filter((b) => !b.platforms || (platform && b.platforms.includes(platform)));
}

export function graphicsBackendOf(settings: Settings | undefined, game: string): GraphicsBackend {
  return settings?.games?.[game]?.graphics_backend ?? "auto";
}

export function graphicsBackendLabel(id: GraphicsBackend): string {
  return GRAPHICS_BACKENDS.find((b) => b.id === id)?.label ?? "Automatic";
}
