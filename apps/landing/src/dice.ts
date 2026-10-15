// The dice crate the server rolls with, compiled to WebAssembly. One chunk,
// loaded once, for the tray and the room alike.

type DiceModule = typeof import("@thunderforge/dice");

let loaded: Promise<DiceModule> | null = null;

export function dice(): Promise<DiceModule> {
  loaded ??= import("@thunderforge/dice").then(async (m) => {
    await m.default();
    return m;
  });
  return loaded;
}

/** A fresh seed for every roll; the crate does the rest. */
export function seed() {
  return crypto.getRandomValues(new Uint32Array(4));
}

/**
 * An id for one throw. `crypto.randomUUID` exists only on secure origins, so
 * a phone on a LAN address would throw on every roll.
 */
export function throwId() {
  return Array.from(crypto.getRandomValues(new Uint32Array(4)), (n) => n.toString(16)).join("-");
}
