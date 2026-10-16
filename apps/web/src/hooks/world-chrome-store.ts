import { GraphQLRequestError } from "@/api/graphqlClient";
import type { WorldChrome } from "./world-chrome-context";

/**
 * The pure parts of the world chrome store (spec 101), kept free of React so
 * they can be tested as plain functions.
 */

export type ChromeByWorld = Readonly<Record<string, Partial<WorldChrome>>>;

/** Equal for the store's purposes: the same value, or the same record read again. */
function same(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== "object" || typeof b !== "object" || !a || !b) return false;
  // Records here are small, plain JSON from the server.
  return JSON.stringify(a) === JSON.stringify(b);
}

/**
 * `current` with `patch` applied to `worldId`, or `current` itself when the
 * patch says nothing new, so an answer that changed nothing re-renders
 * nothing. Each key is replaced whole: a member list is never merged.
 */
export function patchChrome(
  current: ChromeByWorld,
  worldId: string,
  patch: Partial<WorldChrome>,
): ChromeByWorld {
  const known = current[worldId] ?? {};
  const keys = Object.keys(patch) as (keyof WorldChrome)[];
  if (keys.every((key) => key in known && same(known[key], patch[key]))) {
    return current;
  }
  return { ...current, [worldId]: { ...known, ...patch } };
}

/**
 * One read per key at a time: a second page asking while the first read is
 * out gets the same promise. The key is forgotten once the read settles.
 */
export function joinRead<T>(
  inFlight: Map<string, Promise<unknown>>,
  key: string,
  load: () => Promise<T>,
): Promise<T> {
  const pending = inFlight.get(key);
  if (pending) return pending as Promise<T>;
  const read = load().finally(() => inFlight.delete(key));
  inFlight.set(key, read);
  return read;
}

/**
 * The server answered and said no (a code came back), as opposed to a read
 * that never got an answer. A refusal means the world is gone for this
 * person; anything else keeps what was known (research R3).
 */
export function isRefusal(err: unknown): boolean {
  return err instanceof GraphQLRequestError && err.codes.length > 0;
}
