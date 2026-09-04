/**
 * `(systemId, slot) -> Panel` dispatch, discovered rather than listed.
 *
 * A system pack that contributes a panel puts it at
 * `packs/systems/<id>/web/src/panels/<slot>.tsx`, default-exporting a
 * component that takes that slot's props. This file finds it. Nothing here,
 * and nothing anywhere else in shared web code, names a game system.
 *
 * # What this replaced
 *
 * Four pages, each holding a comparison against one system's id and mounting
 * that system's component if it matched:
 *
 * | Page | Asked | Mounted |
 * |---|---|---|
 * | `ActorDetailPage` | is this that system, and is the actor an NPC | the NPC shop |
 * | `WorldStagingPage` | is this world that system | the session loop |
 * | `WorldSystemSettingsPage` | is this a GM, on a world of that system | a carryover card |
 * | `ClocksPanel` | is this world *not* that system → empty state | the session loop |
 *
 * All four sat on `check-system-registry.mjs`'s `KNOWN` list against
 * `032/T108`, which is this file. FR-029 is the rule they broke: a pack
 * declares what it contributes, and shared code collects contributions
 * without knowing a single system's name.
 *
 * The fourth one is the interesting one. It did not mount a panel *for*
 * Genie — it printed an empty state for everyone else, which is the same
 * violation wearing the opposite sign. It now asks whether any pack filled
 * the `clocks` slot and prints the same empty state when none did, so the
 * comparison is deleted rather than moved.
 *
 * # Why a glob, and why that is not "loading a pack at runtime"
 *
 * The same argument `systemActorSheets.ts` makes, and for the same reason:
 * `import.meta.glob` is resolved by Vite **at build time**. It expands to a
 * static import map before anything ships, so the bundle contains exactly the
 * panels that existed when the product was compiled — no fetch, no evaluation
 * of anything the build did not see, and no way for a pack dropped into a
 * running deployment to be picked up. That is the whole of ADR-029: bundled
 * packs may contribute behaviour because their code is reviewed and compiled
 * here; outside code is not executed at all.
 *
 * `eager: true` for the same reason too — these components are in the bundle
 * and already paid for, so a promise per module would only make every caller
 * async for nothing.
 *
 * # Why a slot vocabulary, where the sheet needed none
 *
 * A sheet has one mount point and one props contract, so a filename
 * convention was the whole declaration. A panel has four mount points that
 * hand it different things. `@thunderforge/host` therefore declares the set
 * of slot names and the props of each (`PanelSlot`, `PanelSlotProps`), and a
 * pack fills one by naming its file after it.
 *
 * ADR-066 and spec 032's `T108` entry both said a filename was the wrong
 * place to encode which slot a panel fills. That was overstated — the
 * two-level path carries it fine — and both documents are corrected. The real
 * difference from the sheet is the vocabulary, not the path.
 */

import type { ComponentType } from "react";
import type { PanelSlot, PanelSlotProps } from "@thunderforge/host";

/**
 * Every bundled pack's panels, keyed by the pack directory name — which *is*
 * the system id, the same equivalence `/api/systems` relies on when it lists
 * the systems directory — and the slot file's basename.
 *
 * Typed as `ComponentType<never>` on the way in because the glob cannot know
 * which slot each module fills; `resolvePanel` is where the slot and its
 * props meet, and the cast is confined to that one function.
 */
const DISCOVERED = import.meta.glob<{
  default: ComponentType<never>;
}>("../../../../packs/systems/*/web/src/panels/*.tsx", { eager: true });

function keyFromPath(modulePath: string): string | null {
  const match = /packs\/systems\/([^/]+)\/web\/src\/panels\/([^/]+)\.tsx$/.exec(
    modulePath,
  );
  return match ? `${match[1]}:${match[2]}` : null;
}

/**
 * `` `${systemId}:${slot}` `` to component.
 *
 * A pack may point two slot files at one component — Genie's `world-staging`
 * and `clocks` both export the session loop — and this map then holds the
 * same reference under both keys, which is the intended shape rather than a
 * duplication to collapse.
 *
 * Nothing validates that a slot name is in `PanelSlot`. A pack that ships
 * `panels/wherever.tsx` gets an entry nobody ever looks up, which is
 * indistinguishable from shipping nothing — the type error is on the pack's
 * side, where its `import type { WhateverPanelProps }` fails to resolve.
 */
export const SYSTEM_PANELS: Record<
  string,
  ComponentType<never>
> = Object.fromEntries(
  Object.entries(DISCOVERED).flatMap(([modulePath, module]) => {
    const key = keyFromPath(modulePath);
    return key ? [[key, module.default]] : [];
  }),
);

export function panelKey(systemId: string, slot: PanelSlot): string {
  return `${systemId}:${slot}`;
}

/**
 * The panel a system contributes to a slot, or `null` where it contributes
 * none.
 *
 * The absence is an answer, not an accident — the same call
 * `resolveActorSheet` makes. A world whose system fills no `clocks` slot has
 * no clocks, and the dock says so plainly; it does not get a substitute, and
 * it does not get an empty frame that reads as broken.
 *
 * A world with no system at all, a system that ships no panels, and an id
 * that matches nothing are one answer on screen, so they are one answer here.
 */
export function resolvePanel<S extends PanelSlot>(
  gameSystemId: string | null | undefined,
  slot: S,
): ComponentType<PanelSlotProps[S]> | null {
  if (!gameSystemId) {
    return null;
  }
  const found = SYSTEM_PANELS[panelKey(gameSystemId, slot)];
  return (found as ComponentType<PanelSlotProps[S]> | undefined) ?? null;
}
