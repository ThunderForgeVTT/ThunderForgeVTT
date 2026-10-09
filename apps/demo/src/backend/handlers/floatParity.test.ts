/**
 * Spec 087 T034: the browser measures a fight to the same bit as the server.
 *
 * The server runs the combat crate on x86_64; the demo runs it compiled to
 * wasm32. `float_parity.json` records what the host computes for reach and
 * move-cost cases at awkward coordinates on every grid kind, and the crate's
 * `tests/float_parity.rs` pins the host to those values. This test pins the
 * wasm build in `dist/combat` to the same file, so a Bevy or glam upgrade
 * that moves a float on one target and not the other fails here.
 */
import { readFileSync } from "node:fs";
import { beforeAll, describe, expect, it } from "vitest";
import { initSync, measure, moveCost } from "@thunderforge/combat";

const ROOT = new URL("../../../../../", import.meta.url);

type Case<E> = { name: string; input: unknown; expected: E };
const fixtures = JSON.parse(
  readFileSync(
    new URL("crates/thunderforge-combat/tests/float_parity.json", ROOT),
    "utf8",
  ),
) as {
  measure: Case<{ distance: number; flags: string[] }>[];
  moveCost: Case<number>[];
};

beforeAll(() => {
  initSync({
    module: readFileSync(new URL("dist/combat/combat_bg.wasm", ROOT)),
  });
});

describe("the wasm combat rules agree with the host to the bit", () => {
  it.each(fixtures.measure.map((c) => [c.name, c] as const))(
    "measure: %s",
    (_name, c) => {
      const got = JSON.parse(measure(JSON.stringify(c.input)));
      // Object.is tells 0 from -0, which toBe's === would not.
      expect(Object.is(got.distance, c.expected.distance)).toBe(true);
      expect(got.flags).toEqual(c.expected.flags);
    },
  );

  it.each(fixtures.moveCost.map((c) => [c.name, c] as const))(
    "moveCost: %s",
    (_name, c) => {
      const got = moveCost(JSON.stringify(c.input));
      expect(Object.is(got, c.expected), `${got} against ${c.expected}`).toBe(
        true,
      );
    },
  );
});
