/**
 * How a world's interface-pack binding is written, everywhere it is written.
 *
 * # Why this is one function and not two labels
 *
 * The hub card said `"Unbound placeholder"` and the dashboard said `"Not yet
 * assigned"`, both for the same world in the same state — and both were
 * false. A world with no stored pack is not unbound and nothing is awaiting
 * assignment: the base pack is in force, it is what the participant is looking
 * at, and it has a name. FR-023 is about that falsehood and FR-022 is about
 * there being two of them.
 *
 * SC-008 measures the fix as zero distinct strings for the unset state across
 * the product. Two call sites agreeing today is a convention, and a convention
 * is what produced the two strings in the first place. One function that both
 * surfaces call cannot disagree with itself, so the property holds by
 * construction rather than by everyone remembering.
 */
import { useEffect, useState } from "react";

import {
  listInterfacePacks,
  type InterfacePackSummary,
} from "@/api/interfacePacks";
import { BASE_PACK_ID } from "./appearance-context";

/**
 * The base pack's title, for the moment before the pack list arrives.
 *
 * A transcribed value, which is the thing this repository keeps getting wrong,
 * so it is bound to its source: `interface-pack-label.test.ts` reads
 * `packs/interface/forge/interface.json` and fails if the two drift. The
 * alternative — rendering the raw id and letting it flip to the title a moment
 * later — shows every card on the hub saying "forge" before saying "Forge".
 */
export const BASE_PACK_TITLE = "Forge";

/**
 * What to show for a world bound to `packId`.
 *
 * `null` means no pack is stored, which is not an absence to report but the
 * base pack applying — so it resolves to the base pack exactly as
 * `resolveAppearance` does, and reads as the name of the thing on screen.
 *
 * An id with no installed pack falls through to the id itself. That is the
 * FR-018 case, where the world has opened under the base pack and
 * `MissingPackNotice` has already said so by name; repeating the explanation
 * in a two-word field would be the third wording of the same fact.
 */
export function interfacePackLabel(
  packId: string | null,
  packs: InterfacePackSummary[],
): string {
  const id = packId ?? BASE_PACK_ID;
  const installed = packs.find((pack) => pack.id === id);
  if (installed) return installed.title;
  return id === BASE_PACK_ID ? BASE_PACK_TITLE : id;
}

/**
 * The installed packs, fetched once per page load however many components ask.
 *
 * The hub renders a card per world and each one needs a title, so a fetch per
 * component would be one request per world to answer a question with a single
 * answer. The promise is cached rather than the result, so callers mounting
 * during the flight share it instead of starting a second.
 */
let inFlight: Promise<InterfacePackSummary[]> | null = null;

function loadPacks(): Promise<InterfacePackSummary[]> {
  // A failed listing is an empty listing: `interfacePackLabel` falls back to
  // the base pack's title, which is what is rendering anyway. A label is not
  // worth an error state.
  inFlight ??= listInterfacePacks().catch(() => []);
  return inFlight;
}

/** Test seam — the module cache outlives a test file otherwise. */
export function resetInterfacePackCache(): void {
  inFlight = null;
}

export function useInterfacePacks(): InterfacePackSummary[] {
  const [packs, setPacks] = useState<InterfacePackSummary[]>([]);

  useEffect(() => {
    let live = true;
    void loadPacks().then((loaded) => {
      if (live) setPacks(loaded);
    });
    return () => {
      live = false;
    };
  }, []);

  return packs;
}
