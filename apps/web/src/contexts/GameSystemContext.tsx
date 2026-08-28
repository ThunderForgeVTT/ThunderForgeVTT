/**
 * apps/web/src/contexts/GameSystemContext.tsx
 * Game System Manifest Context
 *
 * Phase 4.8.1: E2.3 - System Manifest Caching
 *
 * React Context to share loaded game system manifests across the component tree
 * without prop drilling. Provides:
 * - Lazy-loaded system manifests on demand
 * - In-memory caching to avoid re-imports
 * - Support for multiple systems simultaneously
 * - Type-safe system calculators and components
 *
 * Usage:
 * ```tsx
 * // At root of app:
 * <GameSystemProvider>
 *   <App />
 * </GameSystemProvider>
 *
 * // In any component:
 * const manifest = useGameSystemManifest('dnd5e');
 * const modifier = manifest.calculators.calculateAbilityModifier(15);
 * ```
 */

import React, {
  ReactNode,
  createContext,
  useContext,
  useCallback,
  useState,
} from "react";

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
interface GameSystemContextValue {
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
const GameSystemContext = createContext<GameSystemContextValue | undefined>(
  undefined,
);

/**
 * Provider component
 * Wrap your app with this to enable useGameSystemManifest hook
 */
export function GameSystemProvider({ children }: { children: ReactNode }) {
  // One Map for the provider's lifetime, mutated in place — a ref in every
  // respect except that it is handed out on the context value below and so
  // is read during consumers' renders, which `react-hooks/refs` rightly
  // rejects for a ref. A lazily-initialised state value is just as stable
  // and legal to read while rendering. `setRefresh` remains the way a fill
  // or a clear is announced, exactly as before.
  const [cache] = useState<Map<string, SystemManifest>>(() => new Map());
  const [, setRefresh] = useState(0);

  const loadManifest = useCallback(
    async (systemId: string): Promise<SystemManifest> => {
      // Check cache first
      const cached = cache.get(systemId);
      if (cached) {
        return cached;
      }

      try {
        // Dynamically import system manifest based on ID — one generic path
        // for every system, no per-systemId special-casing. Every pack's
        // `@/systems/<id>/index` (or, for packs living under
        // `packs/systems/<id>/web`, whatever local re-export bridges it
        // in) exports its manifest under a name containing "Manifest"
        // (`DnD5eSystemManifest`, `genieSystemManifest`, etc.) — sniff for
        // that key rather than hardcoding it per system.
        const module = await import(
          /* @vite-ignore */
          `@/systems/${systemId}/index`
        );

        const keys = Object.keys(module);
        const manifestKey = keys.find((k) => k.includes("Manifest"));
        if (!manifestKey) {
          throw new Error(
            `No manifest exported from @/systems/${systemId}/index`,
          );
        }

        const manifest = module[manifestKey] as SystemManifest;

        // Cache and return
        cache.set(systemId, manifest);
        setRefresh((n) => n + 1); // Trigger re-render for subscriptions
        return manifest;
      } catch (error) {
        console.error(
          `[GameSystemContext] Failed to load manifest for ${systemId}:`,
          error,
        );
        throw error;
      }
    },
    [cache],
  );

  const getCachedManifest = useCallback(
    (systemId: string): SystemManifest | null => {
      return cache.get(systemId) ?? null;
    },
    [cache],
  );

  const clearCache = useCallback(
    (systemId?: string) => {
      if (systemId) {
        cache.delete(systemId);
      } else {
        cache.clear();
      }
      setRefresh((n) => n + 1);
    },
    [cache],
  );

  const value: GameSystemContextValue = {
    loadManifest,
    getCachedManifest,
    clearCache,
    manifestCache: cache,
  };

  return (
    <GameSystemContext.Provider value={value}>
      {children}
    </GameSystemContext.Provider>
  );
}

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
