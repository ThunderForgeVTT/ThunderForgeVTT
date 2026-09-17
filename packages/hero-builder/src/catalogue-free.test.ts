/**
 * Contract B1: the builder names no part, choice, colour field, flag, size or
 * race of its own. Every control is generated from the catalogue, so a key
 * written here as a string is a control that would not follow the catalogue.
 *
 * The keys are read from `@thunderforge/heroes` when the test runs, so a
 * part added there is guarded here with no edit.
 */
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  HERO_COLORS,
  HERO_FLAGS,
  HERO_PARTS,
  HERO_RACES,
  SIZES,
} from "@thunderforge/heroes";

const here = import.meta.dirname;

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((entry) => {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) return sources(path);
    return /\.tsx?$/.test(entry) && !entry.endsWith(".test.ts") ? [path] : [];
  });
}

const keys = new Set<string>([
  ...Object.keys(HERO_PARTS),
  ...Object.values(HERO_PARTS).flat(),
  ...HERO_COLORS,
  ...HERO_FLAGS,
  ...SIZES,
  ...Object.keys(HERO_RACES),
]);

test("the builder's source names no catalogue key as a string", () => {
  const found: string[] = [];
  for (const file of sources(here)) {
    const text = readFileSync(file, "utf8");
    for (const key of keys) {
      for (const quote of ['"', "'", "`"]) {
        if (text.includes(`${quote}${key}${quote}`)) {
          found.push(`${file.slice(here.length + 1)}: ${quote}${key}${quote}`);
        }
      }
    }
  }
  assert.deepEqual(found, []);
});

test("the guard can see a key", () => {
  assert.ok(keys.size > 50, "the catalogue was read");
  assert.ok(sources(here).length > 5, "the sources were found");
});
