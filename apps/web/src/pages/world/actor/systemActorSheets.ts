/**
 * `systemId -> ActorSheet` dispatch, discovered rather than listed.
 *
 * A system pack that ships a data-connected character sheet puts it at
 * `packs/systems/<id>/web/src/ActorSheet.tsx`, default-exporting a component
 * that takes `ActorSheetProps`. This file finds it. Nothing here, and nothing
 * anywhere else in shared web code, names a game system.
 *
 * # Why a glob, and why that is not "loading a pack at runtime"
 *
 * `import.meta.glob` is resolved by Vite **at build time**: it expands to a
 * static import map before anything ships, so the bundle contains exactly the
 * sheets that existed when the product was compiled. There is no fetch, no
 * evaluation of anything the build did not see, and no way for a pack dropped
 * into a running deployment to be picked up.
 *
 * That distinction is the whole of ADR-029. Bundled packs may contribute
 * behaviour because their code is reviewed here and compiled with the product;
 * outside code is not executed at all. A build-time glob is the browser's
 * version of what `inventory` does on the server — the linker finds the
 * contributions, and no list has to be kept in step with reality.
 *
 * `eager: true` matters for a reason beyond convenience. The lazy form hands
 * back a promise per module, which would make `resolveActorSheet` async and
 * ripple through every caller — for a set of components already in the bundle
 * and already paid for. Eager keeps the lookup synchronous, which is what it
 * honestly is.
 *
 * # What this replaced
 *
 * A hand-written `{ genie: GenieActorSheet }`, plus a container living in
 * `apps/web` because there was nowhere in a pack to put a component that
 * fetches and mutates. `@thunderforge/host` is now that somewhere. This was
 * the last entry on `check-system-registry.mjs`'s web-side conscience, and
 * that check now scans this directory so it cannot quietly come back.
 */

import type { ComponentType } from "react";
import type { ActorSheetProps } from "@thunderforge/host";

export type { ActorSheetProps };

/**
 * Every bundled pack's sheet, keyed by the pack directory name — which *is*
 * the system id, the same equivalence `/api/systems` relies on when it lists
 * the systems directory.
 */
const DISCOVERED = import.meta.glob<{
  default: ComponentType<ActorSheetProps>;
}>("../../../../../../packs/systems/*/web/src/ActorSheet.tsx", { eager: true });

function systemIdFromPath(modulePath: string): string | null {
  const match = /packs\/systems\/([^/]+)\/web\/src\/ActorSheet\.tsx$/.exec(
    modulePath,
  );
  return match ? match[1] : null;
}

export const SYSTEM_ACTOR_SHEETS: Record<
  string,
  ComponentType<ActorSheetProps>
> = Object.fromEntries(
  Object.entries(DISCOVERED).flatMap(([modulePath, module]) => {
    const systemId = systemIdFromPath(modulePath);
    return systemId ? [[systemId, module.default]] : [];
  }),
);

/**
 * The sheet for a system, or `null` where that system ships none.
 *
 * Spec 031's edge case — "a game system that defines no character sheet, when
 * a player chooses View" — is asking for the absence to be an *answer* rather
 * than an accident. Callers used to open-code the two-step lookup, which
 * collapses three different situations into `undefined`: an actor belonging to
 * no system, a system with no sheet, and a typo. They are the same answer to
 * the person looking at the screen — there is nothing systemic to draw — so
 * they are one answer here, and the caller renders whatever it can render
 * without a sheet.
 *
 * What it deliberately does not do is substitute a generic sheet. Inventing
 * stats a system never declared would be this app forming an opinion about
 * rules it does not own, and a wrong sheet is worse at a table than no sheet.
 */
export function resolveActorSheet(
  gameSystemId: string | null | undefined,
): ComponentType<ActorSheetProps> | null {
  if (!gameSystemId) {
    return null;
  }
  return SYSTEM_ACTOR_SHEETS[gameSystemId] ?? null;
}
