import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import {
  HERO_COLORS,
  HERO_FLAGS,
  HERO_PARTS,
  HERO_SPEC_SCHEMA,
  HERO_TEXT_LIMITS,
  heroSpecSchemaText,
  PRESET_HEROES,
  randomHero,
  SIZES,
  validateHero,
} from "./index.ts";
import { SERVER_SCHEMA_PATH } from "./schemaPath.ts";

type Schema = Record<string, unknown>;

/** The keywords this test's validator understands. The schema may use no
 * others, so a keyword added to `schema.ts` without teaching this validator
 * fails here instead of being silently ignored. */
const KEYWORDS = new Set([
  "$schema",
  "title",
  "description",
  "type",
  "required",
  "additionalProperties",
  "properties",
  "enum",
  "pattern",
  "maxLength",
]);

function keywordsIn(schema: Schema, at: string, out: string[]): void {
  for (const [key, value] of Object.entries(schema)) {
    if (!KEYWORDS.has(key)) out.push(`${at}${key}`);
    if (key === "properties") {
      for (const [name, sub] of Object.entries(
        value as Record<string, Schema>,
      )) {
        keywordsIn(sub, `${at}properties.${name}.`, out);
      }
    }
  }
}

/** Just enough JSON Schema for `HERO_SPEC_SCHEMA`: whether `value` passes.
 * The server uses a full validator (the `jsonschema` crate); this one exists
 * so the package can prove its schema agrees with `validateHero` without a
 * dependency. */
function passes(schema: Schema, value: unknown): boolean {
  const type = schema.type;
  if (type === "object") {
    if (typeof value !== "object" || value === null || Array.isArray(value)) {
      return false;
    }
    const record = value as Record<string, unknown>;
    const properties = (schema.properties ?? {}) as Record<string, Schema>;
    for (const key of (schema.required ?? []) as string[]) {
      if (!(key in record)) return false;
    }
    for (const [key, field] of Object.entries(record)) {
      const sub = properties[key];
      if (sub === undefined) {
        if (schema.additionalProperties === false) return false;
        continue;
      }
      if (!passes(sub, field)) return false;
    }
    return true;
  }
  if (type === "boolean") return typeof value === "boolean";
  if (type === "string") {
    if (typeof value !== "string") return false;
    if (Array.isArray(schema.enum) && !schema.enum.includes(value)) {
      return false;
    }
    if (
      typeof schema.maxLength === "number" &&
      [...value].length > schema.maxLength
    ) {
      return false;
    }
    if (
      typeof schema.pattern === "string" &&
      !new RegExp(schema.pattern, "u").test(value)
    ) {
      return false;
    }
    return true;
  }
  throw new Error(`the test validator does not know type ${String(type)}`);
}

const properties = HERO_SPEC_SCHEMA.properties as Record<string, Schema>;

describe("HERO_SPEC_SCHEMA", () => {
  it("uses only keywords this test can check", () => {
    const unknown: string[] = [];
    keywordsIn(HERO_SPEC_SCHEMA, "", unknown);
    assert.deepEqual(unknown, []);
  });

  it("is the server's copy, byte for byte", () => {
    const stored = readFileSync(SERVER_SCHEMA_PATH, "utf8");
    assert.equal(
      stored,
      heroSpecSchemaText(),
      "src/server/src/heroes/hero_spec_schema.json is stale: run `pnpm -F @thunderforge/heroes run schema`",
    );
  });

  it("names every field and every choice the catalogue has, and no other", () => {
    const expected = [
      "name",
      "title",
      "size",
      ...Object.keys(HERO_PARTS),
      ...HERO_COLORS,
      ...HERO_FLAGS,
    ].sort();
    assert.deepEqual(Object.keys(properties).sort(), expected);
    for (const [part, choices] of Object.entries(HERO_PARTS)) {
      assert.deepEqual(properties[part]!.enum, [...choices], part);
    }
    assert.deepEqual(properties.size!.enum, [...SIZES]);
    assert.equal(HERO_SPEC_SCHEMA.additionalProperties, false);
    assert.deepEqual(HERO_SPEC_SCHEMA.required, ["name"]);
  });

  it("built before a choice existed, would refuse a spec that uses it", () => {
    const stale = structuredClone(HERO_SPEC_SCHEMA) as Schema;
    const headgear = (stale.properties as Record<string, Schema>).headgear!;
    headgear.enum = (headgear.enum as string[]).filter((c) => c !== "wizard");
    assert.equal(passes(stale, { name: "Mira", headgear: "wizard" }), false);
    assert.notDeepEqual(stale, HERO_SPEC_SCHEMA);
  });

  it("accepts every preset", () => {
    for (const { slug, spec } of PRESET_HEROES) {
      assert.equal(passes(HERO_SPEC_SCHEMA, spec), true, slug);
    }
  });

  it("accepts rolled heroes, with and without a race", () => {
    for (let seed = 0; seed < 50; seed += 1) {
      for (const race of [undefined, "elf", "half-orc", "dwarf"] as const) {
        const spec = {
          name: "Rolled",
          ...randomHero(`seed-${seed}`, { race }),
        };
        assert.equal(validateHero(spec).ok, true);
        assert.equal(passes(HERO_SPEC_SCHEMA, spec), true, `${seed} ${race}`);
      }
    }
  });

  it("refuses every case validateHero refuses", () => {
    const long = (n: number) => "x".repeat(n);
    const refused: unknown[] = [
      null,
      [],
      ["name"],
      "Mira",
      42,
      true,
      {},
      { name: "" },
      { name: "   " },
      { name: 7 },
      { name: long(HERO_TEXT_LIMITS.name + 1) },
      { name: "Mira", title: "  " },
      { name: "Mira", title: long(HERO_TEXT_LIMITS.title + 1) },
      { name: "Mira", title: 3 },
      { name: "Mira", race: "elf" },
      { name: "Mira", seed: 12 },
      { name: "Mira", headgear: "crown" },
      { name: "Mira", headgear: "Wizard" },
      { name: "Mira", size: "enormous" },
      { name: "Mira", skin: "red" },
      { name: "Mira", skin: "#fff" },
      { name: "Mira", skin: "#ff00001" },
      { name: "Mira", skin: " #ff0000" },
      { name: "Mira", eyes: 0xff0000 },
      { name: "Mira", tusks: "yes" },
      { name: "Mira", beard: 1 },
      { name: "Mira", ears: null },
    ];
    for (const value of refused) {
      assert.equal(validateHero(value).ok, false, JSON.stringify(value));
      assert.equal(
        passes(HERO_SPEC_SCHEMA, value),
        false,
        `the schema accepted ${JSON.stringify(value)}`,
      );
    }
  });

  it("accepts what validateHero accepts at the edges", () => {
    const accepted: unknown[] = [
      { name: "Mira" },
      { name: "x".repeat(HERO_TEXT_LIMITS.name) },
      { name: "Mira", title: "" },
      { name: "Mira", title: "x".repeat(HERO_TEXT_LIMITS.title) },
      { name: "Mira", skin: "#ABCDEF" },
      { name: "Mira", tusks: false },
    ];
    for (const value of accepted) {
      assert.equal(validateHero(value).ok, true, JSON.stringify(value));
      assert.equal(
        passes(HERO_SPEC_SCHEMA, value),
        true,
        JSON.stringify(value),
      );
    }
  });
});
