import type { ReactNode } from "react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useAuth } from "@/hooks/useAuth";
import {
  WorldChromeContext,
  type WorldChrome,
  type WorldChromeStore,
} from "./world-chrome-context";
import {
  joinRead,
  patchChrome,
  type ChromeByWorld,
} from "./world-chrome-store";

/**
 * Holds every world's chrome for the session; see `world-chrome-context`.
 *
 * What one person was shown is never shown to the next: the store belongs
 * to whoever is signed in, starts empty when that changes, and an answer
 * that was asked for the previous person is dropped.
 */
export function WorldChromeProvider({ children }: { children: ReactNode }) {
  const who = useAuth().user?.id ?? null;
  const [held, setHeld] = useState<{
    who: string | null;
    chrome: ChromeByWorld;
  }>({ who, chrome: {} });
  const current = useRef(who);
  const inFlight = useRef(new Map<string, Promise<unknown>>());

  useEffect(() => {
    current.current = who;
  }, [who]);

  const write = useCallback(
    (asker: string | null, worldId: string, patch: Partial<WorldChrome>) =>
      setHeld((state) => {
        if (asker !== current.current) return state;
        const base = state.who === asker ? state.chrome : {};
        const chrome = patchChrome(base, worldId, patch);
        // The same answer again re-renders nothing.
        return chrome === state.chrome ? state : { who: asker, chrome };
      }),
    [],
  );

  const remember = useCallback<WorldChromeStore["remember"]>(
    (worldId, patch) => write(who, worldId, patch),
    [who, write],
  );

  const refresh = useCallback<WorldChromeStore["refresh"]>(
    // A new `refresh` for a new person re-runs every page's read.
    (worldId, key, load) => {
      const asker = who;
      return joinRead(inFlight.current, `${asker}:${worldId}:${key}`, () =>
        load(worldId),
      ).then((value) => {
        write(asker, worldId, { [key]: value });
        return value;
      });
    },
    [who, write],
  );

  // Until the first write for a new person, the previous person's store is
  // still held; it is never shown.
  const chrome = held.who === who ? held.chrome : EMPTY;
  const value = useMemo(
    () => ({ chrome, remember, refresh }),
    [chrome, remember, refresh],
  );
  return (
    <WorldChromeContext.Provider value={value}>
      {children}
    </WorldChromeContext.Provider>
  );
}

const EMPTY: ChromeByWorld = {};
