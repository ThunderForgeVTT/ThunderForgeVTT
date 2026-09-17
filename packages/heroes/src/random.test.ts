import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  HERO_PALETTES,
  HERO_RACES,
  raceProblems,
  randomHero,
  resolveHero,
  validateHero,
} from "./index.ts";

const seeds = Array.from({ length: 50 }, (_, i) => `seed-${i}`);

describe("randomHero", () => {
  it("rolls valid heroes, the same one for the same seed", () => {
    for (const seed of seeds) {
      const rolled = randomHero(seed);
      const result = validateHero({ name: "Rolled", ...rolled });
      assert.ok(result.ok, `${seed}: ${result.ok ? "" : result.problems}`);
      assert.deepEqual(randomHero(seed), rolled);
    }
    const faces = new Set(seeds.map((s) => JSON.stringify(randomHero(s))));
    assert.ok(
      faces.size > seeds.length * 0.9,
      "different seeds, different faces",
    );
  });

  it("picks colours only from the palettes", () => {
    for (const seed of seeds) {
      for (const [field, value] of Object.entries(randomHero(seed))) {
        const palette = (HERO_PALETTES as Record<string, readonly string[]>)[
          field
        ];
        if (palette) assert.ok(palette.includes(value as string), field);
      }
    }
  });

  it("leaves a locked field out, so the caller's value survives", () => {
    for (const seed of seeds) {
      const rolled = randomHero(seed, { locked: { hair: true, skin: true } });
      assert.ok(!("hair" in rolled));
      assert.ok(!("skin" in rolled));
    }
  });

  it("keeps every roll for a race inside that race's look", () => {
    for (const race of Object.keys(HERO_RACES)) {
      for (const seed of seeds) {
        const hero = resolveHero({ name: "R", ...randomHero(seed, { race }) });
        assert.deepEqual(raceProblems(hero, race), [], `${race} ${seed}`);
      }
    }
  });

  it("lets a lock beat the race", () => {
    const rolled = randomHero("x", { race: "elf", locked: { ears: true } });
    assert.ok(!("ears" in rolled));
    const hero = resolveHero({
      name: "Round-eared elf",
      ears: "round",
      ...rolled,
    });
    assert.equal(hero.ears, "round");
  });

  it("never writes the race into the hero (B5a)", () => {
    for (const race of Object.keys(HERO_RACES)) {
      const rolled = randomHero("seed", { race });
      assert.ok(!("race" in rolled));
      assert.ok(!("race" in resolveHero({ name: "x", ...rolled })));
    }
  });

  it("refuses a race it does not know", () => {
    assert.throws(() => randomHero("x", { race: "moonkin" }), /not a race/);
  });
});
