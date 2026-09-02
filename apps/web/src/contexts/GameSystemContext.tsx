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

import React, { ReactNode, useCallback, useState } from "react";

import { GameSystemContext } from "./game-system-context";
import type {
  GameSystemContextValue,
  SystemManifest,
} from "./game-system-context";

// Re-exported so consumers that only ever wanted the manifest shape keep
// importing it from here. Type-only exports do not break fast refresh.
export type {
  SystemManifest,
  SystemManifestLegal,
} from "./game-system-context";

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
