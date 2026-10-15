import type { EngineEvent } from "@thunderforge/encounter";
import { loadEngine, type Engine, type Progress } from "./loadEngine.ts";

// Spec 091 FR-004 to FR-006: one engine and one canvas for the whole page.
//
// The engine starts once per page (`startup.rs` returns on a second start),
// so the dice tray and the room share it. Whichever frame asked last holds
// the table: the canvas moves into its stage, the frame that had it tears
// its scene down and goes `away`. The T001 probe proved a moved canvas keeps
// drawing, throwing and taking keys.
//
// The canvas is created here, outside React, so no render ever removes it.

export type Send = (command: Record<string, unknown>) => void;

export interface Holder {
  /** The frame's DOM id. */
  id: string;
  /** Where the canvas goes while this frame holds the table. */
  stage: HTMLElement;
  /** The scene, sent from scratch on every take. */
  setup: (send: Send) => void;
  /** Removes everything `setup` sent, before another frame takes the table. */
  teardown?: (send: Send) => void;
  /** Told when another frame takes the table. */
  onTaken: () => void;
}

const CANVAS_ID = "thunderforge-table";

let engine: Engine | null = null;
let loading: Promise<Engine> | null = null;
let progressTo: ((p: Progress) => void) | null = null;
let holder: Holder | null = null;
let canvas: HTMLCanvasElement | null = null;
const readyListeners: Array<(e: Engine) => void> = [];

function theCanvas(): HTMLCanvasElement {
  if (canvas) return canvas;
  const c = document.createElement("canvas");
  c.id = CANVAS_ID;
  // A picture of what the page already says in words.
  c.setAttribute("aria-hidden", "true");
  c.tabIndex = -1;
  // The engine's window setup makes the canvas focusable; put it back.
  new MutationObserver(() => {
    if (c.tabIndex !== -1) c.tabIndex = -1;
  }).observe(c, { attributes: true, attributeFilter: ["tabindex"] });
  canvas = c;
  return c;
}

/** Sends a command, if the engine runs. */
export const send: Send = (command) => {
  engine?.apply_world_command(JSON.stringify(command));
};

/** Sends for `id` only while it holds the table. */
export function sendAs(id: string): Send {
  return (command) => {
    if (holder?.id === id) send(command);
  };
}

export const holderId = () => holder?.id ?? null;
export const running = () => engine;

/**
 * Spec 098 FR-008: a frame seated at another's table. It sends through the
 * holder, presses keys on its canvas, and hears the engine's events as
 * `hear` turns them, while the holder keeps the table.
 */
export interface Guest {
  id: string;
  hear: (event: EngineEvent) => EngineEvent | null;
  /** The holder's token for the guest's. */
  token: (id: string) => string;
}

let guest: Guest | null = null;

/** Seats `next`, or stands the guest up: the board shows all again. */
export function seatGuest(next: Guest | null) {
  if (!next && guest && engine && holder) engine.set_viewer_token("");
  guest = next;
}

export const guestOf = () => (holder ? guest : null);

/** The canvas, while the engine runs and `id` holds the table or sits at it. */
export const canvasOf = (id: string) =>
  engine && (holder?.id === id || (holder && guest?.id === id)) ? canvas : null;

/** Sets whose sight the board shows, while `id` holds the table or sits at it. */
export function viewAs(id: string, tokenId: string) {
  if (engine && holder?.id === id) engine.set_viewer_token(tokenId);
  else if (engine && holder && guest?.id === id) engine.set_viewer_token(guest.token(tokenId));
}

/** Runs `fn` with the engine once it has started, now if it already has. */
export function whenReady(fn: (e: Engine) => void) {
  if (engine) fn(engine);
  else readyListeners.push(fn);
}

/** The panic hook stopped the engine: every later call would throw. */
export function stopped() {
  engine = null;
  guest = null;
  loading = null;
  holder = null;
}

function load(onProgress: (p: Progress) => void, signal?: AbortSignal): Promise<Engine> {
  progressTo = onProgress;
  if (engine) return Promise.resolve(engine);
  if (!loading) {
    loading = loadEngine(`#${CANVAS_ID}`, (p) => progressTo?.(p), signal).then(
      (e) => {
        engine = e;
        for (const fn of readyListeners.splice(0)) fn(e);
        return e;
      },
      (err) => {
        loading = null;
        throw err;
      },
    );
  }
  return loading;
}

/**
 * Give the table to `next`: the holder's scene comes down, the canvas moves,
 * the engine loads if it has not, and `next`'s scene goes up.
 */
export async function take(
  next: Holder,
  onProgress: (p: Progress) => void,
  signal?: AbortSignal,
): Promise<Engine> {
  const prev = holder;
  if (prev && prev.id !== next.id) {
    if (engine) prev.teardown?.(send);
    prev.onTaken();
  }
  holder = next;
  next.stage.append(theCanvas());
  const e = await load(onProgress, signal);
  if (holder !== next) return e;
  e.set_viewer_token("");
  next.setup(send);
  send({ type: "set_rendering", on: true });
  return e;
}
