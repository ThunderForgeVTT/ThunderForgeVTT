/**
 * Reading actor system data, which has no shape this app can know.
 *
 * The `ability_data`/`resource_data`/`proficiency_data`/`trait_data`/
 * `spell_data` columns are JSONB, and what lives in them is decided by
 * whichever game system the actor belongs to — dnd5e stores
 * `{ strength: 10 }` where Pathfinder stores `{ strength_mod: 0 }`, in the
 * same column. So they arrive typed `Record<string, unknown>`, and a reader
 * that wants a number has to say so.
 *
 * Every helper here refuses rather than guesses: a value of the wrong type
 * reads as absent. Coercing instead (`Number(value)`, `String(value)`) would
 * turn a system whose data this app cannot read into a sheet full of `NaN`
 * and `[object Object]` that looks like real character data.
 */

/** A number stored at `key`, or `undefined` if it is missing or not a number. */
export function readNumber(
  data: Record<string, unknown> | undefined | null,
  key: string,
): number | undefined {
  const value = data?.[key];
  return typeof value === "number" && Number.isFinite(value)
    ? value
    : undefined;
}

/** A string stored at `key`, or `undefined` if it is missing or not a string. */
export function readString(
  data: Record<string, unknown> | undefined | null,
  key: string,
): string | undefined {
  const value = data?.[key];
  return typeof value === "string" ? value : undefined;
}

/**
 * Just the numeric entries of an open record.
 *
 * For handing a whole column to a component that indexes it by key (ability
 * scores, for one). Non-numeric entries are dropped, not converted — keep the
 * original record around for writes, since a round trip through this would
 * silently delete every key it could not read.
 */
export function numberEntries(
  data: Record<string, unknown> | undefined | null,
): Record<string, number> {
  const out: Record<string, number> = {};
  for (const [key, value] of Object.entries(data ?? {})) {
    if (typeof value === "number" && Number.isFinite(value)) out[key] = value;
  }
  return out;
}

/** Just the boolean entries of an open record. See {@link numberEntries}. */
export function booleanEntries(
  data: Record<string, unknown> | undefined | null,
): Record<string, boolean> {
  const out: Record<string, boolean> = {};
  for (const [key, value] of Object.entries(data ?? {})) {
    if (typeof value === "boolean") out[key] = value;
  }
  return out;
}
