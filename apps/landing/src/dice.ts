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
