import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  HERO_PARTS,
  HERO_RACES,
  HEX_COLOR,
  matchRace,
  SIZES,
} from "./index.ts";

describe("the race looks", () => {
  it("narrow only to choices and swatches that exist", () => {
    for (const [race, look] of Object.entries(HERO_RACES)) {
      for (const [field, allowed] of Object.entries(look.choices ?? {})) {
        const options: readonly string[] =
          field === "size"
            ? SIZES
            : HERO_PARTS[field as keyof typeof HERO_PARTS];
        assert.ok(options, `${race}: ${field} is not a part`);
        assert.ok(allowed.length > 0, `${race}: ${field} allows nothing`);
        for (const choice of allowed) {
          assert.ok(options.includes(choice), `${race}: ${field}.${choice}`);
        }
      }
      for (const [field, allowed] of Object.entries(look.swatches ?? {})) {
        assert.ok(allowed.length > 0, `${race}: ${field} allows nothing`);
        for (const hex of allowed) {
          assert.match(hex, HEX_COLOR, `${race}: ${field} ${hex}`);
        }
      }
    }
  });

  it("give every alias to one race, and none to another race's key", () => {
    const owner = new Map<string, string>();
    for (const race of Object.keys(HERO_RACES)) owner.set(race, race);
    for (const [race, look] of Object.entries(HERO_RACES)) {
      for (const alias of look.aliases) {
        assert.equal(alias, alias.toLowerCase(), `${race}: ${alias}`);
        assert.ok(!owner.has(alias), `${alias} is ${owner.get(alias)}'s`);
        owner.set(alias, race);
      }
    }
  });
});

describe("matchRace", () => {
  it("finds every race by key or alias, in any case, with spaces around", () => {
    for (const [race, look] of Object.entries(HERO_RACES)) {
      for (const name of [race, ...look.aliases]) {
        assert.equal(matchRace(`  ${name.toUpperCase()} `), race, name);
      }
    }
    assert.equal(matchRace("High Elf"), "elf");
  });

  it("finds nothing for text it does not know, or no text", () => {
    assert.equal(matchRace("Moonkin"), null);
    assert.equal(matchRace(""), null);
    assert.equal(matchRace("   "), null);
    assert.equal(matchRace(null), null);
    assert.equal(matchRace(undefined), null);
  });
});
