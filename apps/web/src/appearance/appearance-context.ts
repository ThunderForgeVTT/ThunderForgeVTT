/**
 * What a world looks like, and the hook that reads it.
 *
 * Split from the provider deliberately: a module exporting both a component
 * and a hook cannot fast-refresh, so editing either half would force a full
 * reload and lose whatever was on screen. This repo enforces that at
 * `--max-warnings=0`.
 */
import { createContext, useContext } from "react";

import type { InterfaceManifest, TokenMap } from "@/api/interfacePacks";

/** The pack that applies when nothing else does. */
export const BASE_PACK_ID = "forge";

export interface ResolvedAppearance {
  /** The pack in force — the base pack's id when falling back. */
  packId: string;
  /**
   * The pack that was asked for and could not be applied, or null.
   *
   * Set rather than thrown, because a look that cannot load must cost nothing:
   * the world still opens, every action still works, and the participant is
   * told once (FR-018).
   */
  missing: string | null;
  light: TokenMap;
  dark: TokenMap;
  canvas: Record<string, unknown> | null;
  layout: unknown[];
  /** Still resolving. The base pack's look applies meanwhile. */
  loading: boolean;
}

export const AppearanceContext = createContext<ResolvedAppearance | null>(null);

/**
 * The world's appearance.
 *
 * Outside a provider this returns null rather than throwing: appearance is
 * presentation, and a component that renders without knowing the world's look
 * should render in the default one, not fail.
 */
export function useAppearance(): ResolvedAppearance | null {
  return useContext(AppearanceContext);
}

/**
 * A pack's declarations over the base pack's.
 *
 * A pack that wants to change one token changes one token; the rest fall
 * through. The alternative — requiring a complete set — makes an author repeat
 * values they do not care about, which is how a pack ends up pinning a default
 * it never chose and never updates. `AppearanceOverride` in the engine and
 * `TokenMap` in the validator both arrived at this for the same reason.
 */
export function overlay(base: TokenMap, over: TokenMap): TokenMap {
  return { ...base, ...over };
}

/** Resolve a chosen pack against the base, tolerating a missing one. */
export function resolveAppearance(
  base: InterfaceManifest,
  chosen: InterfaceManifest | null,
  requestedId: string | null,
): ResolvedAppearance {
  const missing = requestedId !== null && chosen === null ? requestedId : null;

  return {
    packId: chosen?.id ?? base.id,
    missing,
    light: overlay(base.light, chosen?.light ?? {}),
    dark: overlay(base.dark, chosen?.dark ?? {}),
    canvas: chosen?.canvas ?? base.canvas ?? null,
    // Absent inherits the base pack's, which is generic and therefore works
    // against any system — including one that ships after the pack does.
    layout: chosen?.layout ?? base.layout ?? [],
    loading: false,
  };
}

/**
 * `background` becomes `--background`, `cardForeground` becomes
 * `--card-foreground`, and `chart1` becomes `--chart-1`.
 *
 * The chart case is the one that is not purely mechanical: the stylesheet
 * writes `--chart-1` while the token key fuses the digit to the word, so a
 * letter-to-digit boundary is a hyphen here too. Getting this wrong would set
 * a custom property nothing reads, and the failure would be silent — the chart
 * colours would simply stay whatever Forge said.
 */
export function customPropertyName(token: string): string {
  return `--${token
    .replace(/([a-z])([A-Z])/g, "$1-$2")
    .replace(/([a-z])(\d)/g, "$1-$2")
    .toLowerCase()}`;
}
