/**
 * The game-system manifest context, its types, and the hooks that read it.
 *
 * Separated from `GameSystemContext.tsx`, which keeps the provider. A module
 * exporting both a component and a hook cannot fast-refresh: editing either
 * one forces a full reload and loses whatever state was on screen.
 */
import React, { createContext, useContext, useState } from "react";

/**
 * Spec 016 (FR-001, contracts/manifest-legal-schema.md): a system pack's
 * required, structured legal/attribution metadata — the render-ready
 * expansion of the manifest's loose free-text `license` string. Required
 * on every manifest (FR-003/FR-007); a pack missing it fails server-side
 * validation before it ever reaches this type.
 */
export type SystemManifestLegal = {
  licenseName: string;
  attributionText: string;
  requiredNotice?: string | null;
  disclaimer?: string | null;
  trademarkRestrictions?: string[];
  requiredUiPlacement?: string | null;
  sourceUrl?: string | null;
};

/**
 * System manifest type. Each system (D&D 5e, Pathfinder 2e, etc.) exports its
 * own manifest, so beyond the four keys every pack must publish the contents
 * are whatever that pack chose. The index signature is `unknown` rather than
 * `any` on purpose: a consumer reaching for a pack-specific table
 * (`sizeCategories`, `abilityFacets`, …) has to say what shape it expects at
 * the point it reads it, instead of every such read silently type-checking.
 */
export type SystemManifest = {
  id: string;
  title: string;
  version: string;
  legal: SystemManifestLegal;
  [key: string]: unknown;
};

/**
 * Game system context value
 */
export interface GameSystemContextValue {
  /** Load a system manifest by ID, caching subsequent loads */
  loadManifest: (systemId: string) => Promise<SystemManifest>;

  /** Get a cached manifest without loading (returns null if not loaded) */
  getCachedManifest: (systemId: string) => SystemManifest | null;

  /** Clear cache for a specific system or all systems */
  clearCache: (systemId?: string) => void;

  /** Map of systemId -> loaded manifest */
  manifestCache: Map<string, SystemManifest>;
}

/**
 * Internal context for shared state
 */
export const GameSystemContext = createContext<
  GameSystemContextValue | undefined
>(undefined);
/**
 * Hook to load and access a system manifest
 *
 * @param systemId - ID of the system to load (e.g., "dnd5e", "pathfinder2e")
 * @returns { manifest, loading, error } - Current state of manifest loading
 *
 * Usage:
 * ```tsx
 * const { manifest, loading, error } = useGameSystemManifest('dnd5e');
 *
 * if (loading) return <div>Loading system...</div>;
 * if (error) return <div>Error: {error.message}</div>;
 *
 * const modifier = manifest.calculators.calculateAbilityModifier(15);
 * ```
 */
export function useGameSystemManifest(systemId: string) {
  const context = useContext(GameSystemContext);
  if (!context) {
    throw new Error(
      "useGameSystemManifest must be used within GameSystemProvider",
    );
  }

  // A cache hit is not something to copy into state from an effect: it is
  // already available while rendering, and mirroring it cost a second render
  // (and, on a systemId change, one render still showing the previous
  // system's manifest). Only the outcome of an actual load is state, and it
  // is stored with the systemId it belongs to so a late response for a
  // system we have since navigated away from can never be read as this one's.
  const cached = context.getCachedManifest(systemId);
  const [loaded, setLoaded] = useState<{
    systemId: string;
    manifest: SystemManifest | null;
    error: Error | null;
  } | null>(null);

  const settled = loaded?.systemId === systemId ? loaded : null;
  const manifest = cached ?? settled?.manifest ?? null;
  const error = settled?.error ?? null;
  const loading = !manifest && !error;

  React.useEffect(() => {
    // Already cached: `manifest` above is serving it, nothing to load.
    if (context.getCachedManifest(systemId)) {
      return;
    }

    let isMounted = true;

    context
      .loadManifest(systemId)
      .then((result) => {
        if (isMounted) {
          setLoaded({ systemId, manifest: result, error: null });
        }
      })
      .catch((err) => {
        if (isMounted) {
          setLoaded({
            systemId,
            manifest: null,
            error: err instanceof Error ? err : new Error(String(err)),
          });
        }
      });

    return () => {
      isMounted = false;
    };
  }, [systemId, context]);

  return { manifest, loading, error };
}

/**
 * Hook to access a cached manifest synchronously
 * (Returns null if not yet loaded)
 *
 * Usage:
 * ```tsx
 * const manifest = useCachedGameSystemManifest('dnd5e');
 * if (manifest) {
 *   const modifier = manifest.calculators.calculateAbilityModifier(15);
 * }
 * ```
 */
export function useCachedGameSystemManifest(
  systemId: string,
): SystemManifest | null {
  const context = useContext(GameSystemContext);
  if (!context) {
    throw new Error(
      "useCachedGameSystemManifest must be used within GameSystemProvider",
    );
  }

  return context.getCachedManifest(systemId);
}
