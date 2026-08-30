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

/** Every status command, discriminated on `type`. */
export type StatusCommand =
  | { type: "set_token_status"; tokenId: string; resources: StatusResource[] }
  | { type: "clear_token_status"; tokenId: string };

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
