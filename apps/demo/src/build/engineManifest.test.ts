import { describe, expect, it } from "vitest";
import { engineManifest } from "./engineManifest.ts";

const wasm = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]);

describe("engineManifest", () => {
  it("names the hashed glue and wasm, and counts the wasm", () => {
    const manifest = engineManifest({
      "assets/index-a1.js": { type: "chunk", fileName: "assets/index-a1.js", isDynamicEntry: false },
      "assets/engine-B7MfRaL8.js": {
        type: "chunk",
        fileName: "assets/engine-B7MfRaL8.js",
        isDynamicEntry: true,
      },
      "assets/engine_bg-uJt7OZaQ.js": {
        type: "chunk",
        fileName: "assets/engine_bg-uJt7OZaQ.js",
        isDynamicEntry: true,
      },
      "assets/engine_bg-BlbLpOM6.wasm": {
        type: "asset",
        fileName: "assets/engine_bg-BlbLpOM6.wasm",
        source: wasm,
      },
      "assets/dice_bg-DHIgDmOu.wasm": {
        type: "asset",
        fileName: "assets/dice_bg-DHIgDmOu.wasm",
        source: new Uint8Array(3),
      },
    });
    expect(manifest).toEqual({
      js: "assets/engine-B7MfRaL8.js",
      wasm: "assets/engine_bg-BlbLpOM6.wasm",
      bytes: { raw: 8 },
      version: expect.stringMatching(/^[0-9a-f]{16}$/),
    });
  });

  it("fails the build when the engine is missing", () => {
    expect(() => engineManifest({})).toThrow(/expected one engine glue chunk/);
  });
});
