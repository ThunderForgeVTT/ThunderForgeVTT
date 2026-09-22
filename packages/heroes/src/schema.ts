/**
 * `HERO_SPEC_SCHEMA` — what a stored hero spec may contain, as JSON Schema
 * (draft 2020-12), for the one reader that cannot import this package: the
 * server, which stores the spec that drew an actor's image (spec 044 phase d,
 * contract B7, ADR-106).
 *
 * Built from the same lists `validateHero` reads — `HERO_PARTS`,
 * `HERO_COLORS`, `HERO_FLAGS`, `SIZES`, `HEX_COLOR` and `HERO_TEXT_LIMITS` —
 * so a choice added to the catalogue is in the schema the moment it exists.
 * `additionalProperties: false` is the schema's half of B5a: the race a roll
 * used is not a hero field, so a spec that carries one is refused, not
 * stored.
 *
 * The server holds a copy, `src/server/src/heroes/hero_spec_schema.json`,
 * written by `pnpm -F @thunderforge/heroes run schema`. `schema.test.ts`
 * fails when that copy differs from this one, so the two cannot drift.
 *
 * Two edges JSON Schema cannot state the way `String.length` and `trim()` do:
 * `maxLength` counts code points where `validateHero` counts UTF-16 code
 * units, and the schema's "not only whitespace" pattern is a regular
 * expression's idea of whitespace. The server applies `validateHero`'s
 * measure to `name` and `title` after the schema (`heroes/spec_schema.rs`),
 * so nothing `validateHero` refuses is stored.
 */
import { HEX_COLOR } from "./color.ts";
import {
  HERO_COLORS,
  HERO_FLAGS,
  HERO_PARTS,
  HERO_TEXT_LIMITS,
  SIZES,
} from "./spec.ts";

type JsonSchema = { readonly [key: string]: unknown };

/** Text that is not empty and not only whitespace. */
const SOME_TEXT = "\\S";

function schema(): JsonSchema {
  const properties: Record<string, JsonSchema> = {
    name: {
      type: "string",
      pattern: SOME_TEXT,
      maxLength: HERO_TEXT_LIMITS.name,
    },
    // A title may be written empty ("no title") but not as spaces.
    title: {
      type: "string",
      pattern: `^$|${SOME_TEXT}`,
      maxLength: HERO_TEXT_LIMITS.title,
    },
    size: { type: "string", enum: [...SIZES] },
  };
  for (const [part, choices] of Object.entries(HERO_PARTS)) {
    properties[part] = { type: "string", enum: [...choices] };
  }
  for (const color of HERO_COLORS) {
    properties[color] = { type: "string", pattern: HEX_COLOR.source };
  }
  for (const flag of HERO_FLAGS) {
    properties[flag] = { type: "boolean" };
  }
  return {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    title: "HeroSpec",
    description:
      "A hero as written: a name, and whatever else differs from the defaults. Generated from @thunderforge/heroes; do not edit by hand.",
    type: "object",
    required: ["name"],
    additionalProperties: false,
    properties,
  };
}

export const HERO_SPEC_SCHEMA: JsonSchema = schema();

/** The schema as the server's copy is written: two-space JSON, one trailing
 * newline. */
export function heroSpecSchemaText(): string {
  return `${JSON.stringify(HERO_SPEC_SCHEMA, null, 2)}\n`;
}
