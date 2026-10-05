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
 * the slot, so the comparison is deleted rather than moved. Since spec 067
 * that slot is named `dock`, and the dock shows no tab when nobody fills it.
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
 * # Why each panel is its own download
 *
 * Until spec 068 the glob was `eager`, on the argument that these components
 * were "in the bundle and already paid for". They were paid for by everyone:
 * a table playing one system downloaded every system's panels, 98 kB of them,
 * to open the board. The glob is lazy now, so the build still sees every
 * panel and still ships exactly those, and a browser fetches the ones the
 * world it opened can mount.
 *
 * Callers did not become async for it. Which panels exist is the glob's keys,
 * known without loading anything; what `resolvePanel` hands back is a
 * component that fetches its own code when it is first rendered (`onDemand`).
 * The one thing that does need the module is the name a pack gives its dock
 * tab, which rides on the slot file — `useSystemDockTitle` loads that one
 * file, for the system on the board.
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

import { useEffect, useState, type ComponentType } from "react";
import type { PanelSlot, PanelSlotProps } from "@thunderforge/host";
import { onDemand } from "@/components/ui/lazy-boundary/onDemand";

/**
 * Every bundled pack's panels, keyed by the pack directory name — which *is*
 * the system id, the same equivalence `/api/systems` relies on when it lists
 * the systems directory — and the slot file's basename.
 *
 * Typed as `ComponentType<never>` on the way in because the glob cannot know
 * which slot each module fills; `resolvePanel` is where the slot and its
 * props meet, and the cast is confined to that one function.
 */
interface PanelModule {
  default: ComponentType<never>;
  /** What a slot that shows a heading or a tab calls this panel. */
  title?: unknown;
}

const DISCOVERED = import.meta.glob<PanelModule>(
  "../../../../packs/systems/*/web/src/panels/*.tsx",
);

/** `` `${systemId}:${slot}` `` to what fetches that slot file. */
const LOADERS: Record<string, () => Promise<PanelModule>> = Object.fromEntries(
  Object.entries(DISCOVERED).flatMap(([modulePath, load]) => {
    const key = keyFromPath(modulePath);
    return key ? [[key, load]] : [];
  }),
);

function keyFromPath(modulePath: string): string | null {
  const match = /packs\/systems\/([^/]+)\/web\/src\/panels\/([^/]+)\.tsx$/.exec(
    modulePath,
  );
  return match ? `${match[1]}:${match[2]}` : null;
}

/**
 * `` `${systemId}:${slot}` `` to a component that loads that slot's panel
 * when it is first rendered.
 *
 * A pack may point two slot files at one component — Genie's `world-staging`
 * and `dock` both export the session loop. Each key here has its own loader,
 * and the two load one module's worth of code: `loadPanel` is where that
 * identity can be seen.
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
  Object.entries(LOADERS).map(([key, load]) => [
    key,
    onDemand(load, "This game system's panel"),
  ]),
);

/**
 * The slot file a system ships for a slot, fetched; `null` where it ships
 * none. For what needs the module itself rather than something to mount.
 */
export function loadPanel(
  gameSystemId: string | null | undefined,
  slot: PanelSlot,
): Promise<PanelModule> | null {
  if (!gameSystemId) {
    return null;
  }
  return LOADERS[panelKey(gameSystemId, slot)]?.() ?? null;
}

export function panelKey(systemId: string, slot: PanelSlot): string {
  return `${systemId}:${slot}`;
}

/**
 * The panel a system contributes to a slot, or `null` where it contributes
 * none.
 *
 * The absence is an answer, not an accident — the same call
 * `resolveActorSheet` makes. A world whose system fills no `dock` slot has
 * no tab in the dock; it does not get a substitute, and it does not get an
 * empty frame that reads as broken.
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

/**
 * What a system calls the panel it contributes to a slot, or `null` where it
 * contributes none or left it untitled. The caller owns the fallback wording.
 *
 * Anything but a non-empty string is no title.
 */
export async function loadPanelTitle(
  gameSystemId: string | null | undefined,
  slot: PanelSlot,
): Promise<string | null> {
  const module = await loadPanel(gameSystemId, slot);
  const title = module?.title;
  return typeof title === "string" && title.trim() ? title.trim() : null;
}

/** What the tab reads when a pack fills the slot and does not title it. */
const UNTITLED = "Game system";

/**
 * The dock tab's label for this world's system, or `null` when its system
 * fills no `dock` slot — in which case the dock shows no tab at all (spec 067
 * FR-021). A tab that opens onto "this system has nothing here" told a table
 * about a feature it does not have.
 */
export async function systemDockTitle(
  gameSystemId: string | null,
): Promise<string | null> {
  if (!resolvePanel(gameSystemId, "dock")) {
    return null;
  }
  // A panel that did not arrive still has a tab: opening it is where the
  // person is told, by the boundary the panel sits in.
  const title = await loadPanelTitle(gameSystemId, "dock").catch(() => null);
  return title ?? UNTITLED;
}

/**
 * `systemDockTitle` for a component: `null` while the system's dock file is
 * on its way, and for good when the system fills no `dock` slot. The tab
 * appears with its name rather than appearing and then being renamed.
 */
export function useSystemDockTitle(gameSystemId: string | null): string | null {
  const [known, setKnown] = useState<{
    gameSystemId: string | null;
    title: string | null;
  } | null>(null);

  useEffect(() => {
    let current = true;
    void systemDockTitle(gameSystemId).then((title) => {
      if (current) {
        setKnown({ gameSystemId, title });
      }
    });
    return () => {
      current = false;
    };
  }, [gameSystemId]);

  // What was learned about another system is not this one's title.
  return known?.gameSystemId === gameSystemId ? known.title : null;
}
