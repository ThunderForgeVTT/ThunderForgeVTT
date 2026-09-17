import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  HERO_RACES,
  minimalSpec,
  PRESET_HEROES,
  presetSource,
  randomHero,
  resolveHero,
  SKIN_TONES,
  type HeroSpec,
} from "./index.ts";

const rolled: HeroSpec[] = Array.from({ length: 100 }, (_, i) => {
  const races = [null, ...Object.keys(HERO_RACES)];
  return {
    name: `R${i}`,
    ...randomHero(`m${i}`, { race: races[i % races.length] }),
  };
});

describe("minimalSpec", () => {
  it("draws the same hero for every preset and every roll", () => {
    for (const spec of [...PRESET_HEROES.map((p) => p.spec), ...rolled]) {
      assert.deepEqual(resolveHero(minimalSpec(spec)), resolveHero(spec));
    }
  });

  it("drops a field set to what it would have been anyway, and keeps the name", () => {
    const spec: HeroSpec = {
      name: "Echo",
      outfit: "#3d6fd1",
      hairColor: "#aa0000",
      beardColor: "#aa0000",
      hair: "long",
    };
    assert.deepEqual(minimalSpec(spec), {
      name: "Echo",
      hairColor: "#aa0000",
      hair: "long",
    });
    assert.deepEqual(minimalSpec({ name: "Only" }), { name: "Only" });
  });
});

describe("presetSource", () => {
  it("writes a named skin tone by name, and pastes back to the same hero", () => {
    const spec: HeroSpec = { name: "Pip", skin: SKIN_TONES.umber, hair: "bun" };
    const source = presetSource("pip", spec);
    assert.match(source, /skin: SKIN_TONES\.umber,/);
    assert.match(source, /slug: "pip",/);
    const literal = source
      .replace(/SKIN_TONES\.(\w+)/g, (_, tone: string) =>
        JSON.stringify(SKIN_TONES[tone as keyof typeof SKIN_TONES]),
      )
      .replace(/(\w+):/g, '"$1":')
      .replace(/,(\s*[}\]])/g, "$1")
      .trim()
      .replace(/,$/, "");
    const entry = JSON.parse(literal) as { slug: string; spec: HeroSpec };
    assert.deepEqual(resolveHero(entry.spec), resolveHero(spec));
  });

  it("writes a skin that is no named tone as its hex", () => {
    assert.match(
      presetSource("x", { name: "X", skin: "#123456" }),
      /skin: "#123456",/,
    );
  });
});
