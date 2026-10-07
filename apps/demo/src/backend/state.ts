/**
 * The demo world, held in the tab.
 *
 * Rows are kept in the field names the GraphQL schema gives them, so a handler
 * can hand one back as it is and the schema's own executor picks out what the
 * query asked for. There is no client-side database here and there must not
 * be one (`AGENTS.md`; spec 074 FR-012): this is one object, written to the
 * browser's own storage as JSON by the few lines at the bottom.
 */
import type { ArtAsset } from "../seed/art";
import { buildSeed, type MapListing, type Viewer } from "../seed/world";
import { forgetUploads } from "./uploads";

export type Row = Record<string, unknown>;

/**
 * Where an asset's bytes are: a static file under `maps/` for a seeded map,
 * else the browser's upload store (`uploads.ts`), with the
 * `canvas_image_assets` row the upload made.
 */
export interface HeldAsset {
  file?: string;
  byteSize: number;
  row?: Row;
}

export interface SceneExploration {
  enabled: boolean;
  epoch: number;
  /** User id → the epoch that member's own reset left them at. */
  resets: Record<string, number>;
}

export interface DemoState {
  version: 4;
  world: Row & { id: string; activeSceneId: string | null };
  scenes: Row[];
  levels: Row[];
  walls: Row[];
  tokens: Row[];
  lights: Row[];
  shapes: Row[];
  lore: Row[];
  chat: Row[];
  /** `world_roll_records`, oldest first (`handlers/dice.ts`). */
  rolls?: Row[];
  /**
   * `scenes.exploration_enabled` and `exploration_epoch`, and each member's
   * `scene_exploration_resets` row, by scene (`handlers/lighting.ts`).
   */
  exploration?: Record<string, SceneExploration>;
  /** `GraphQLWorldActor` rows, plus `castKey` for the demo's own use. */
  actors: Row[];
  /** One `GraphQLActorSystemData` row per actor that has any. */
  systemData: Row[];
  /**
   * Who asked the operation in flight: the handlers answer as this member.
   * The tab holding the world sets it per request (spec 081 R7); which tab
   * renders for whom is `currentViewer`.
   */
  viewer: Viewer;
  /** The hero the seeded player is playing (spec 023); they own both. */
  claimedActorId: string | null;
  /** Asset id → where its bytes are: a static file under `maps/`, or an upload. */
  assets: Record<string, HeldAsset>;
  /** Actor art asset id → the spec that draws it (`seed/art.ts`). */
  art: Record<string, ArtAsset>;
  nextEventId: number;
  /**
   * Spec 079: the fight, made the first time one is started. Optional, so a
   * world saved before there were fights still loads; "Start over" forgets
   * it with everything else.
   */
  fight?: Fight;
}

/** Spec 079 FR-006: everything a fight is, kept so a reload keeps it. */
export interface Fight {
  combats: Row[];
  combatants: Row[];
  attacks: Row[];
  offers: Row[];
  /** A copy's own hit points (`tokens.system_data` on the server). */
  copies: Record<string, Row>;
  /** Where the dice start, and how many throws have been made since. */
  seed: number;
  draws: number;
}

/** Versioned, so a later shape of the world does not read an earlier one. */
const STORAGE_KEY = "thunderforge-demo:v4";
const SAVE_AFTER_MS = 250;

let state: DemoState | null = null;
let saveTimer: ReturnType<typeof setTimeout> | null = null;

/** The world. Only valid once `loadState` has resolved. */
export function demoState(): DemoState {
  if (!state) {
    throw new Error("the demo world was read before it was loaded");
  }
  return state;
}

function readSaved(): DemoState | null {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const saved = JSON.parse(raw) as DemoState;
    return saved.version === 4 ? saved : null;
  } catch {
    // Storage that cannot be read is the same as storage with nothing in it.
    return null;
  }
}

/**
 * Loads what this browser last had, or the seed. `fetchStatic` is the real
 * `fetch`, kept from before the guard replaced it.
 */
export async function loadState(
  fetchStatic: typeof fetch,
  base: string,
): Promise<DemoState> {
  const saved = readSaved();
  savedViewer ??= saved?.viewer ?? "gm";
  if (saved) {
    state = saved;
    return saved;
  }
  const response = await fetchStatic(`${base}maps/maps.json`);
  if (!response.ok) {
    throw new Error(`the demo's maps could not be read (${response.status})`);
  }
  state = buildSeed((await response.json()) as MapListing[], base);
  return state;
}

function saveNow(): void {
  if (saveTimer) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
  if (!state) return;
  try {
    // `viewer` is whoever asked last; the world keeps the one it was saved
    // with, which is all a tab without a choice of its own reads (R7).
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ ...state, viewer: savedViewer ?? state.viewer }),
    );
  } catch {
    // A full or forbidden store costs the visitor their changes on reload and
    // nothing else; the world in the tab is still whole.
  }
}

/**
 * Spec 081: a tab taking over the world from one that closed reads it as
 * that tab last saved it, which is everything that tab answered.
 */
export function reloadSaved(): void {
  const saved = readSaved();
  if (saved) state = saved;
}

/** Called after anything changes. Coalesces a drag's worth of writes. */
export function markChanged(): void {
  if (saveTimer) return;
  saveTimer = setTimeout(saveNow, SAVE_AFTER_MS);
}

/** Spec 081 R7: the GM / player switch belongs to the tab, not the world. */
const VIEWER_KEY = "thunderforge-demo:viewer";

/**
 * The viewer a world saved before spec 081 was switched to, kept as it was
 * so that a tab opened without a choice of its own starts where the visitor
 * left off, whatever the tabs have asked since.
 */
let savedViewer: Viewer | null = null;

/**
 * Who this tab renders for. A tab that has not chosen starts as the saved
 * world's viewer, read once, or as the GM.
 */
export function currentViewer(): Viewer {
  try {
    const chosen = window.sessionStorage.getItem(VIEWER_KEY);
    if (chosen === "gm" || chosen === "player") return chosen;
  } catch {
    // A tab that cannot remember its choice renders for the default.
  }
  savedViewer ??= readSaved()?.viewer ?? "gm";
  return savedViewer;
}

/**
 * Who this tab renders for. Kept at once: the caller reloads the page so the
 * real client asks who it is afresh, and the answer must be waiting.
 */
export function setViewer(viewer: Viewer): void {
  try {
    window.sessionStorage.setItem(VIEWER_KEY, viewer);
  } catch {
    // Nowhere to keep it: the tab stays as it was.
  }
}

/** FR-007: forget what this browser changed. The next load is the seed. */
export function forgetSavedWorld(): void {
  if (saveTimer) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
  state = null;
  forgetUploads();
  try {
    window.localStorage.removeItem(STORAGE_KEY);
  } catch {
    // Nothing was saved, then.
  }
}

/** Writes the world now if anything is waiting to be written. */
export function flushSave(): void {
  if (saveTimer) saveNow();
}

window.addEventListener("pagehide", flushSave);
