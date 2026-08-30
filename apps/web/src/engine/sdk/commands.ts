import type { Disclosed } from "./Disclosed";
import type { ResourceDefinition } from "./ResourceDefinition";

/**
 * Typed commands for the engine's status surface.
 *
 * Hand-written — only the *shapes* in this directory are generated. What this
 * adds over calling `apply_world_command` with a JSON string is that a wrong
 * field name or type is a compile error rather than a display that silently
 * never appears.
 *
 * That failure mode is not hypothetical. The engine deserializes what it
 * recognises and ignores the rest, so before this boundary was typed a drifted
 * field produced nothing at all — no error, no warning, nothing to attach a
 * debugger to. Three defects in spec 029 alone had exactly that shape.
 *
 * The engine now also *reports* what it could not accept (`sdkError` through
 * the event callback), so the two halves work together: the compiler catches
 * what it can see, and the runtime says what it could not.
 */

/**
 * The contract version this bundle speaks.
 *
 * Must match `SDK_VERSION` in `src/engine/src/lib.rs`. A mismatch makes the
 * engine refuse the command outright and report it, rather than applying the
 * part it happened to understand.
 */
export const SDK_VERSION = 1;

/** One resource on one token, as the server resolved it for this viewer. */
export interface StatusResource {
  definition: ResourceDefinition;
  disclosed: Disclosed;
}

/** A colour as sRGB components in 0.0–1.0. */
export type Rgb = [number, number, number];

/**
 * A partial appearance for status displays.
 *
 * Every field optional, and an absent field means "leave this alone" rather
 * than "reset this to the default". That distinction is the whole point: an
 * application that only wants taller bars should not have to restate the
 * palette, because restating it pins those colours to whatever the defaults
 * happened to be the day the call was written, and they then never improve.
 *
 * The documented default set lives in exactly one place —
 * `DisplayAppearance::default()` in `thunderforge-canvas-core` — and this
 * folds onto whatever is currently in effect, so successive overrides
 * accumulate instead of each one discarding the last.
 */
export interface AppearanceOverride {
  /** The unfilled part of a bar. */
  track?: Rgb;
  trackAlpha?: number;
  /** Fill for a resource the viewer is not being told the value of. */
  undisclosed?: Rgb;
  /** Fills taken in the system's declared order, wrapping if it runs out. */
  palette?: Rgb[];
  barHeight?: number;
  barGap?: number;
  firstBarOffset?: number;
}

/** Every status command, discriminated on `type`. */
export type StatusCommand =
  | { type: "set_token_status"; tokenId: string; resources: StatusResource[] }
  | { type: "clear_token_status"; tokenId: string }
  | { type: "set_display_appearance"; appearance: AppearanceOverride };

/** What the engine reports when it cannot accept a command. */
export interface EngineSdkError {
  type: "sdkError";
  code:
    | "versionMismatch"
    | "unknownDefinition"
    | "duplicateDefinition"
    | "stackingNotAllowed"
    | "valueOutOfRange"
    | "malformed";
  message: string;
  command?: string | null;
}

/** Narrow an engine event to an SDK error, if that is what it is. */
export function asSdkError(event: unknown): EngineSdkError | null {
  if (typeof event !== "object" || event === null) return null;
  const candidate = event as { type?: unknown };
  return candidate.type === "sdkError" ? (event as EngineSdkError) : null;
}

/**
 * Stamp a command with the SDK version and serialise it.
 *
 * Every caller goes through here, so the version cannot be forgotten on one
 * command and remembered on another — which would leave exactly one path
 * failing silently, the hardest kind to find.
 */
export function encodeStatusCommand(command: StatusCommand): string {
  return JSON.stringify({ ...command, sdkVersion: SDK_VERSION });
}
