import { createContext, useContext, useEffect } from "react";

import { getWorld } from "@/api/world";
import type { WorldPlayState } from "@/api/playPause";
import type { WorldMemberDoc } from "@/db/collections/worldMembersCollection";
import type { WorldRecord } from "@/types/world";
import { isRefusal } from "./world-chrome-store";

/**
 * What a world's pages draw around themselves, the same on every one of
 * them: the scene Play resumes in, and the look the world has chosen.
 *
 * Each world page mounts its own `WorldSectionShell`, so before this every
 * page change read both again from nothing: Play said "Play" until its query
 * answered, then "Resume in …", and a world's look fell back to the base
 * pack for a moment. Held above the routes instead, the last answer shows
 * at once and the page only refreshes it.
 *
 * Spec 101 widened it to what every world page reads again on arrival: the
 * world itself, its members, the cleared claim gate and the play state.
 */
export interface WorldChrome {
  /** The active scene's name; `null` when none is active or it is hidden. */
  resumeScene: string | null;
  /** The world's interface pack; `null` for the base look. */
  interfacePackId: string | null;
  /** The world; `null` when it is missing or refused to this person. */
  world: WorldRecord | null;
  /** The roster, sorted by role, replaced whole on every answer. */
  members: WorldMemberDoc[];
  /** The claim gate let this person in, earlier in this tab. */
  claimCleared: boolean;
  /** Whether play is paused; `null` when the read was refused. */
  playState: WorldPlayState | null;
}

export interface WorldChromeStore {
  chrome: Readonly<Record<string, Partial<WorldChrome>>>;
  remember: (worldId: string, patch: Partial<WorldChrome>) => void;
  /**
   * Reads `key` with `load`, joined with a read already out for it, and
   * remembers the answer unless the signed-in person changed meanwhile.
   */
  refresh: <K extends keyof WorldChrome>(
    worldId: string,
    key: K,
    load: (worldId: string) => Promise<WorldChrome[K]>,
  ) => Promise<WorldChrome[K]>;
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
  const { chrome, refresh } = useStore();
  useEffect(() => {
    // Answers are kept by world, so one for a world the page has left is
    // still right, just not shown here. What was known stays on a failed
    // read; a page never loses its chrome to one.
    refresh(worldId, key, load).catch(() => undefined);
  }, [worldId, key, load, refresh]);
  return chrome[worldId]?.[key];
}

/** A refusal or a missing world is `null`; any other failure keeps the known one. */
function readWorld(worldId: string): Promise<WorldRecord | null> {
  return getWorld(worldId).catch((err: unknown) => {
    if (isRefusal(err)) return null;
    throw err;
  });
}

export interface KnownWorld {
  /** `null` before the first answer, and for a missing or refused world. */
  world: WorldRecord | null;
  /** False only before the first answer: the one time a page shows its loader. */
  known: boolean;
}

/** The world as last known, shown at once and refreshed on every visit. */
export function useKnownWorld(worldId: string): KnownWorld {
  const world = useWorldChrome(worldId, "world", readWorld);
  return { world: world ?? null, known: world !== undefined };
}
