import type { ReactNode } from "react";
import { useCallback, useMemo, useState } from "react";

import { WorldChromeContext, type WorldChrome } from "./world-chrome-context";

/** Holds every world's chrome for the session; see `world-chrome-context`. */
export function WorldChromeProvider({ children }: { children: ReactNode }) {
  const [chrome, setChrome] = useState<
    Readonly<Record<string, Partial<WorldChrome>>>
  >({});

  const remember = useCallback(
    (worldId: string, patch: Partial<WorldChrome>) =>
      setChrome((current) => {
        const known = current[worldId] ?? {};
        const unchanged = (Object.keys(patch) as (keyof WorldChrome)[]).every(
          (key) => known[key] === patch[key],
        );
        // The same answer again re-renders nothing.
        return unchanged
          ? current
          : { ...current, [worldId]: { ...known, ...patch } };
      }),
    [],
  );

  const value = useMemo(() => ({ chrome, remember }), [chrome, remember]);
  return (
    <WorldChromeContext.Provider value={value}>
      {children}
    </WorldChromeContext.Provider>
  );
}
