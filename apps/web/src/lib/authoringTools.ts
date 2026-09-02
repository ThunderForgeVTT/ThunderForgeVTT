import type { GmToolId } from "@/components/world/GmToolRail/GmToolRail";

/**
 * Filtering the rail by what the viewer may actually use.
 *
 * Kept apart from the rail component and from the hook that fetches, because
 * this is the only part with a rule in it and rules are what a test should be
 * able to reach without a DOM. The type comes across as a type-only import, so
 * nothing here pulls a component into a node-environment test.
 */

/**
 * `null` means "not resolved yet", which is **not** the same as "none".
 *
 * The distinction is the whole reason this is a nullable list rather than an
 * empty one. A Game Master's answer arrives a moment after the canvas does; if
 * the unresolved state read as "no tools", their rail would blink out and back
 * on every load. Unresolved therefore shows what the viewer's role already
 * allowed — which is what the rail did before permissions existed — and the
 * server and the engine, not this, are what stop a tool being used.
 */
export type ResolvedTools = readonly GmToolId[] | null;

/** The subset of `tools` this viewer may use. */
export function permittedTools<T extends { id: GmToolId }>(
  tools: readonly T[],
  allowed: ResolvedTools,
): T[] {
  if (allowed === null) return [...tools];
  return tools.filter((tool) => allowed.includes(tool.id));
}

/**
 * Which tool the rail should have open, given what the viewer may use.
 *
 * A tool that has just been taken away must not stay open — the flyout would
 * go on offering controls for something the engine now refuses, which is the
 * "silently ceases to respond" failure spec 031 names. Falling back to the
 * first tool they *do* hold rather than to `null` keeps the rail in the same
 * shape it has any other time: something is armed.
 */
export function reconcileOpenTool(
  openToolId: GmToolId | null,
  allowed: ResolvedTools,
): GmToolId | null {
  if (allowed === null) return openToolId;
  if (openToolId !== null && allowed.includes(openToolId)) return openToolId;
  return allowed[0] ?? null;
}
