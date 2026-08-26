/**
 * A read-only window onto the world store, for when the UI says nothing.
 *
 * Every hard bug this codebase has hit recently failed *silently*: the
 * engine drew nothing because no renderer was compiled in, keys stopped
 * working because focus moved, a map switch froze for seconds inside a
 * smoothed average, and `TokenTool` renders empty when a selected token's
 * id is not a key in the store. None of those produce an error. What they
 * have in common is that the state which would explain them lives inside a
 * closure — React state, or an `App::run()` that never returns — where
 * neither a person nor a test can look at it.
 *
 * `plugins/render_probe.rs` and `plugins/frame_trace.rs` solve exactly this
 * on the engine side. This is the same idea for the store: publish what the
 * store believes, plus the recent command traffic that got it there, so
 * "the panel is blank" can be answered with the id it is actually holding
 * rather than by reading the reducer and guessing.
 *
 * Development only. `import.meta.env.DEV` is compiled to a constant, so the
 * whole module is dropped from a production bundle — the probe cannot leak
 * world state to a player because it does not exist in their build. It is
 * available under Playwright, which runs against the dev server.
 *
 * Deliberately read-only: it reports, it never dispatches. A debugging
 * surface that can also mutate state becomes a way to write tests that pass
 * against situations the app cannot actually reach.
 */

import type { WorldCommand } from "./types";
import type { WorldStore } from "./store";

/** Commands retained. Enough to cover a scene load plus an interaction. */
const COMMAND_LOG_CAPACITY = 200;

interface LoggedCommand {
  type: WorldCommand["type"];
  /** Which layer dispatched it — "bevy", "sync", or "ui". */
  source: string;
  /** The subject's id, when the command has one. */
  id?: string;
}

export interface WorldProbe {
  /** What the store currently believes, in the smallest useful form. */
  state: () => {
    worldId: string | null;
    selectedTokenId: string | null;
    tokenIds: string[];
    /** Position and size of each token, which is what a missed canvas
     *  hit-test comes down to. */
    tokens: { id: string; x: number; y: number; scale?: number }[];
    /** The question `TokenTool` silently answers "no" to when it renders
     *  nothing: is the selected id actually a token the store knows? */
    selectionResolves: boolean;
    counts: Record<string, number>;
  };
  /** Recent command traffic, oldest first. */
  commands: () => LoggedCommand[];
}

/** The subject id of a command, for the log. Commands are a wide union and
 *  most carry an id under one of a few shapes. */
function commandId(command: WorldCommand): string | undefined {
  const record = command as unknown as Record<string, unknown>;
  for (const key of ["tokenId", "wallId", "lightId", "shapeId", "assetId", "worldId"]) {
    const value = record[key];
    if (typeof value === "string") return value;
  }
  const subject = record.token ?? record.wall ?? record.light ?? record.shape;
  if (subject && typeof subject === "object" && "id" in subject) {
    const id = (subject as { id: unknown }).id;
    if (typeof id === "string") return id;
  }
  return undefined;
}

/**
 * Publishes the probe at `window.__worldProbe` and returns an unsubscribe.
 * A no-op outside development.
 */
export function installWorldProbe(store: WorldStore): () => void {
  if (!import.meta.env.DEV || typeof window === "undefined") {
    return () => {};
  }

  const log: LoggedCommand[] = [];

  const unsubscribe = store.subscribe((event) => {
    if (log.length === COMMAND_LOG_CAPACITY) {
      log.shift();
    }
    log.push({
      type: event.command.type,
      source: event.source,
      id: commandId(event.command),
    });
  });

  const probe: WorldProbe = {
    state: () => {
      const state = store.getState();
      const tokenIds = Object.keys(state.tokens);
      return {
        worldId: state.worldId,
        selectedTokenId: state.selectedTokenId,
        tokenIds,
        tokens: Object.values(state.tokens).map((token) => ({
          id: token.id,
          x: token.x,
          y: token.y,
          scale: token.scale,
        })),
        selectionResolves:
          state.selectedTokenId !== null && tokenIds.includes(state.selectedTokenId),
        counts: {
          tokens: tokenIds.length,
          walls: Object.keys(state.walls).length,
          lights: Object.keys(state.lights).length,
          shapes: Object.keys(state.shapes).length,
        },
      };
    },
    commands: () => [...log],
  };

  (window as unknown as Record<string, unknown>).__worldProbe = probe;

  return () => {
    unsubscribe();
    delete (window as unknown as Record<string, unknown>).__worldProbe;
  };
}
