import { parseEngineEvent, type EngineEvent } from "@thunderforge/encounter";
import { guestOf, holderId, whenReady } from "./host.ts";

// Spec 091: the engine has one event callback (`sdk.rs`, a single slot), so
// the page installs it once, here. Each string is parsed once and handed to
// the frame that holds the table, and to a frame seated at it; any other
// frame hears nothing.

type Listener = (event: EngineEvent) => void;

const listeners = new Map<string, Listener>();
let installed = false;

function route(raw: string) {
  const event = parseEngineEvent(raw);
  if (!event) return;
  // A command the engine refused is a bug in the page; the console guard
  // fails the e2e on it.
  if (event.type === "sdkError") console.error(`engine refused a command: ${event.code}: ${event.message}`);
  const id = holderId();
  if (id) listeners.get(id)?.(event);
  // A frame seated at the holder's table hears it too, in its own terms.
  const guest = guestOf();
  const heard = guest && guest.id !== id ? guest.hear(event) : null;
  if (guest && heard) listeners.get(guest.id)?.(heard);
}

/** Hear the engine's events while frame `id` holds the table. */
export function listen(id: string, listener: Listener): () => void {
  listeners.set(id, listener);
  if (!installed) {
    installed = true;
    whenReady((engine) => engine.set_event_callback(route));
  }
  return () => {
    if (listeners.get(id) === listener) listeners.delete(id);
  };
}
