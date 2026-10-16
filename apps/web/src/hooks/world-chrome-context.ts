import { createContext, useContext, useEffect } from "react";

/**
 * What a world's pages draw around themselves, the same on every one of
 * them: the scene Play resumes in, and the look the world has chosen.
 *
 * Each world page mounts its own `WorldSectionShell`, so before this every
 * page change read both again from nothing: Play said "Play" until its query
 * answered, then "Resume in …", and a world's look fell back to the base
 * pack for a moment. Held above the routes instead, the last answer shows
 * at once and the page only refreshes it.
 */
export interface WorldChrome {
  /** The active scene's name; `null` when none is active or it is hidden. */
  resumeScene: string | null;
  /** The world's interface pack; `null` for the base look. */
  interfacePackId: string | null;
}

export interface WorldChromeStore {
  chrome: Readonly<Record<string, Partial<WorldChrome>>>;
  remember: (worldId: string, patch: Partial<WorldChrome>) => void;
}

export const WorldChromeContext = createContext<WorldChromeStore | null>(null);

function useStore(): WorldChromeStore {
  const store = useContext(WorldChromeContext);
  if (!store) {
    throw new Error("World chrome needs a WorldChromeProvider above it.");
  }
  return store;
}

/** Records a value the page learned some other way, such as a live event. */
export function useRememberWorldChrome(): WorldChromeStore["remember"] {
  return useStore().remember;
}

/**
 * The last known `key` for `worldId` (`undefined` before the first answer),
 * refreshed from `load` each time a page asks. `load` should be stable: a
 * module-level function, not one made during render.
 */
export function useWorldChrome<K extends keyof WorldChrome>(
  worldId: string,
  key: K,
  load: (worldId: string) => Promise<WorldChrome[K]>,
): WorldChrome[K] | undefined {
  const { chrome, remember } = useStore();
  useEffect(() => {
    let active = true;
    load(worldId)
      .then((value) => {
        if (active) remember(worldId, { [key]: value });
      })
      // What was known stays; a page never loses its chrome to a failed read.
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [worldId, key, load, remember]);
  return chrome[worldId]?.[key];
}
