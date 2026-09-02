/**
 * Who has already been told about a missing interface pack.
 *
 * # Why this is not a ref inside the component
 *
 * It was, and the ref was wrong. `WorldSectionShell` is deliberately "a thin
 * wrapper, not a nested route layout" so that each world route keeps its own
 * data fetching — which means every route page renders its *own* shell, its
 * own `WorldAppearance`, and its own `MissingPackNotice`. Walking from Session
 * Setup to the Compendium unmounts one and mounts another, and a ref inside
 * the component starts empty again each time.
 *
 * So the notice fired on every navigation, which is the exact thing FR-018
 * says it must not do: inform each participant **once**. The component looked
 * correct in isolation and was defeated by where it is mounted, which is not
 * something the component can see.
 *
 * Module state outlives the remount. It does not outlive a reload, and that is
 * right: a fresh page load is a fresh session, and saying it once more there
 * is informing rather than nagging.
 *
 * # Why the key is the pack and not the world
 *
 * "This pack is not installed" is a fact about the deployment, not about the
 * world that happens to name it. Two worlds bound to the same absent pack have
 * one problem between them, and hearing about it twice would not tell the
 * second one anything the first did not.
 */

const told = new Set<string>();

/**
 * Whether to raise the notice for `missing`, recording that it was raised.
 *
 * Deliberately not a predicate — it has the side effect its callers need, so
 * that "ask" and "record" cannot come apart. A pure `hasBeenTold` plus a
 * separate `markTold` is two calls a caller can get half right, and the half
 * that gets forgotten is the second one.
 */
export function shouldTellAboutMissingPack(missing: string | null): boolean {
  if (!missing || told.has(missing)) {
    return false;
  }
  told.add(missing);
  return true;
}

/** Test seam — module state outlives a test file otherwise. */
export function resetToldAboutPacks(): void {
  told.clear();
}
