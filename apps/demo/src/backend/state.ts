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

export type Row = Record<string, unknown>;

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
  /** `GraphQLWorldActor` rows, plus `castKey` for the demo's own use. */
  actors: Row[];
  /** One `GraphQLActorSystemData` row per actor that has any. */
  systemData: Row[];
  /** Whose view the page renders; the session answers with this member. */
  viewer: Viewer;
  /** The hero the seeded player is playing (spec 023); they own both. */
  claimedActorId: string | null;
  /** Background asset id → the static file under `maps/` that is its bytes. */
  assets: Record<string, { file: string; byteSize: number }>;
  /** Actor art asset id → the spec that draws it (`seed/art.ts`). */
  art: Record<string, ArtAsset>;
  nextEventId: number;
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
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // A full or forbidden store costs the visitor their changes on reload and
    // nothing else; the world in the tab is still whole.
  }
}

/** Called after anything changes. Coalesces a drag's worth of writes. */
export function markChanged(): void {
  if (saveTimer) return;
  saveTimer = setTimeout(saveNow, SAVE_AFTER_MS);
}

/**
 * Who the page is rendering for, before the world is necessarily loaded: the
 * notice bar draws on the first paint, and the saved answer is what the
 * session will report once it is.
 */
export function currentViewer(): Viewer {
  return state?.viewer ?? readSaved()?.viewer ?? "gm";
}

/**
 * Who the page is rendering for. Saved at once: the caller reloads the page
 * so the real client asks who it is afresh, and the answer must be waiting.
 */
export function setViewer(viewer: Viewer): void {
  if (!state) state = readSaved();
  if (!state) return;
  state.viewer = viewer;
  saveNow();
}

/** FR-007: forget what this browser changed. The next load is the seed. */
export function forgetSavedWorld(): void {
  if (saveTimer) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
  state = null;
  try {
    window.localStorage.removeItem(STORAGE_KEY);
  } catch {
    // Nothing was saved, then.
  }
}

window.addEventListener("pagehide", () => {
  if (saveTimer) saveNow();
});
