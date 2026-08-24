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

import React, { ReactNode, createContext, useContext, useCallback, useRef, useState } from "react";

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
 * System manifest type - extends any for flexibility
 * Each system (D&D 5e, Pathfinder 2e, etc.) exports its own manifest
 */
export type SystemManifest = {
  id: string;
  title: string;
  version: string;
  legal: SystemManifestLegal;
  [key: string]: any;
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
const GameSystemContext = createContext<GameSystemContextValue | undefined>(undefined);

/**
 * Provider component
 * Wrap your app with this to enable useGameSystemManifest hook
 */
export function GameSystemProvider({ children }: { children: ReactNode }) {
  const cacheRef = useRef<Map<string, SystemManifest>>(new Map());
  const [, setRefresh] = useState(0);

  const loadManifest = useCallback(async (systemId: string): Promise<SystemManifest> => {
    // Check cache first
    const cached = cacheRef.current.get(systemId);
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
        throw new Error(`No manifest exported from @/systems/${systemId}/index`);
      }

      const manifest = module[manifestKey] as SystemManifest;

      // Cache and return
      cacheRef.current.set(systemId, manifest);
      setRefresh((n) => n + 1); // Trigger re-render for subscriptions
      return manifest;
    } catch (error) {
      console.error(`[GameSystemContext] Failed to load manifest for ${systemId}:`, error);
      throw error;
    }
  }, []);

  const getCachedManifest = useCallback((systemId: string): SystemManifest | null => {
    return cacheRef.current.get(systemId) ?? null;
  }, []);

  const clearCache = useCallback((systemId?: string) => {
    if (systemId) {
      cacheRef.current.delete(systemId);
    } else {
      cacheRef.current.clear();
    }
    setRefresh((n) => n + 1);
  }, []);

  const value: GameSystemContextValue = {
    loadManifest,
    getCachedManifest,
    clearCache,
    manifestCache: cacheRef.current,
  };

  return <GameSystemContext.Provider value={value}>{children}</GameSystemContext.Provider>;
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
    throw new Error("useGameSystemManifest must be used within GameSystemProvider");
  }

  const [manifest, setManifest] = useState<SystemManifest | null>(
    context.getCachedManifest(systemId),
  );
  const [loading, setLoading] = useState(!manifest);
  const [error, setError] = useState<Error | null>(null);

  React.useEffect(() => {
    // If already loaded, use cached version
    const cached = context.getCachedManifest(systemId);
    if (cached) {
      setManifest(cached);
      setLoading(false);
      return;
    }

    // Load manifest
    let isMounted = true;

    (async () => {
      try {
        setLoading(true);
        setError(null);
        const loaded = await context.loadManifest(systemId);

        if (isMounted) {
          setManifest(loaded);
          setLoading(false);
        }
      } catch (err) {
        if (isMounted) {
          setError(err instanceof Error ? err : new Error(String(err)));
          setLoading(false);
        }
      }
    })();

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
export function useCachedGameSystemManifest(systemId: string): SystemManifest | null {
  const context = useContext(GameSystemContext);
  if (!context) {
    throw new Error("useCachedGameSystemManifest must be used within GameSystemProvider");
  }

  return context.getCachedManifest(systemId);
}
