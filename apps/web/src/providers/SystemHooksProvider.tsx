/**
 * SystemHooksProvider
 *
 * This provider manages dynamic loading of game system modules
 * and makes hooks available to child components via React context.
 */

import React, { createContext, useEffect, useState, ReactNode } from "react";
import { SystemHooksContract } from "../hooks/useSystemHooks";

/**
 * System context value
 */
export interface SystemContextValue {
  systemId?: string;
  hooks: SystemHooksContract;
  loading: boolean;
  error?: string;
  reload: () => Promise<void>;
}

/**
 * Create the hooks context
 */
export const SystemHooksContext = createContext<SystemContextValue | undefined>(
  undefined,
);

interface SystemHooksProviderProps {
  worldId: string;
  systemId?: string;
  children: ReactNode;
}

/**
 * Provider component
 */
export const SystemHooksProvider: React.FC<SystemHooksProviderProps> = ({
  systemId,
  children,
}) => {
  const [hooks, setHooks] = useState<SystemHooksContract>({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();

  const loadSystemModules = async () => {
    if (!systemId) {
      setHooks({});
      setError(undefined);
      return;
    }

    setLoading(true);
    setError(undefined);

    try {
      // 1. Fetch system manifest
      const manifestResponse = await fetch(
        `/api/systems/${systemId}/manifest.json`,
      );
      if (!manifestResponse.ok) {
        throw new Error(
          `Failed to load system manifest: ${manifestResponse.statusText}`,
        );
      }
      const manifest = await manifestResponse.json();

      // 2. Load CSS files
      if (manifest.styles && Array.isArray(manifest.styles)) {
        for (const stylePath of manifest.styles) {
          const link = document.createElement("link");
          link.rel = "stylesheet";
          link.href = `/api/systems/${systemId}/${stylePath}`;
          link.id = `system-style-${systemId}-${stylePath.replace(/\//g, "-")}`;
          document.head.appendChild(link);
        }
      }

      // 3. Load JavaScript ESM modules
      const loadedHooks: SystemHooksContract = {};

      if (manifest.esmodules && Array.isArray(manifest.esmodules)) {
        for (const modulePath of manifest.esmodules) {
          try {
            const moduleUrl = `/api/systems/${systemId}/${modulePath}`;
            // Dynamic import for ESM modules
            const module = await import(/* webpackIgnore: true */ moduleUrl);

            // Merge exported hooks into the context
            if (module.default) {
              Object.assign(loadedHooks, module.default);
            } else if (module.hooks) {
              Object.assign(loadedHooks, module.hooks);
            } else {
              // Try to assign the module itself if it's not wrapped
              Object.assign(loadedHooks, module);
            }
          } catch (err) {
            console.warn(`Failed to load system module ${modulePath}:`, err);
            // Don't fail entire provider if one module fails
            // Just skip it and continue with other modules
          }
        }
      }

      setHooks(loadedHooks);
    } catch (err) {
      const errorMsg =
        err instanceof Error ? err.message : "Unknown error loading system";
      setError(errorMsg);
      console.error("Error loading system modules:", err);
      setHooks({});
    } finally {
      setLoading(false);
    }
  };

  // Load modules when systemId changes
  useEffect(() => {
    loadSystemModules();
  }, [systemId]);

  // Cleanup CSS when unmounting or system changes
  useEffect(() => {
    return () => {
      if (systemId) {
        const links = document.querySelectorAll(
          `link[id^="system-style-${systemId}"]`,
        );
        links.forEach((link) => link.remove());
      }
    };
  }, [systemId]);

  const value: SystemContextValue = {
    systemId,
    hooks,
    loading,
    error,
    reload: loadSystemModules,
  };

  return (
    <SystemHooksContext.Provider value={value}>
      {children}
    </SystemHooksContext.Provider>
  );
};
