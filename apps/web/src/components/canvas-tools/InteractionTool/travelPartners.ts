import type { Interactive, UpdateInteractiveInput } from "@/api/interactives";
import type { SceneLevel } from "@/api/levels";
import type { ReferenceChoice } from "@/components/InteractionAuthor";

/**
 * Pointing a transition at its other end.
 *
 * A transition is an interactive carrying the travel effect and naming a
 * partner — another interactive of the same scene, usually on another level.
 * A token that walks into one, or whose player clicks one, is set down at the
 * other. A pair that works in both directions is two interactives each naming
 * the other; nothing on the server ties them together, so "link both ways" is
 * a second write this side makes.
 *
 * Kept apart from the panel for the reason `placeAuthoredProp` is: these are
 * decisions, and decisions are checked without mounting a component.
 */

/** The effect id the server's registry declares for level travel. */
export const TRAVEL_EFFECT_ID = "nav.travel";

/** The configuration key naming where a traveller arrives. */
const PARTNER_KEY = "partner";

/** One level's interactives. `level` is `null` when no level list was read. */
export interface LevelInteractives {
  level: SceneLevel | null;
  interactives: Interactive[];
}

const KIND_NAME: Record<Interactive["subjectKind"], string> = {
  prop: "Prop",
  door: "Door",
  region: "Area",
};

/**
 * What a Game Master is shown for one possible partner.
 *
 * Interactives have no names. What tells two apart is what they are, a few
 * characters of their id — the same convention the wall and light pickers
 * use — and, above all, the level they are listed under.
 */
function describe(interactive: Interactive): string {
  const what = `${KIND_NAME[interactive.subjectKind]} ${interactive.interactiveId.slice(0, 8)}`;
  return interactive.effectId === TRAVEL_EFFECT_ID
    ? `${what} (already a way through)`
    : what;
}

/**
 * Everything a transition may arrive at, under the level each is on.
 *
 * `except` is the interactive being authored, which is never its own partner.
 */
export function travelChoices(
  everywhere: readonly LevelInteractives[],
  except: string | null,
): ReferenceChoice[] {
  return everywhere.flatMap(({ level, interactives }) =>
    interactives
      .filter((interactive) => interactive.interactiveId !== except)
      .map((interactive) => ({
        id: interactive.interactiveId,
        label: describe(interactive),
        ...(level ? { group: level.name } : {}),
      })),
  );
}

/** The partner a saved interactive names, if it is a transition with one. */
export function partnerOf(interactive: Interactive): string | null {
  if (interactive.effectId !== TRAVEL_EFFECT_ID) {
    return null;
  }
  const partner = interactive.effectConfig?.[PARTNER_KEY];
  return typeof partner === "string" && partner ? partner : null;
}

export type LinkBackOutcome =
  /** Nothing to do: not a transition, or its partner already leads back. */
  | { kind: "nothing" }
  | { kind: "linked" }
  | { kind: "refused"; message: string };

/**
 * Make the partner of a just-saved transition lead back to it.
 *
 * Refuses, in words, rather than overwrite a partner that already does
 * something else: a lever that opens a door is somebody's work, and quietly
 * turning it into a staircase because a box was ticked on another interactive
 * would destroy it with nothing to say it had. A partner that is scenery, or
 * already a transition, is free to point back.
 */
export async function linkBack(
  api: {
    updateInteractive: (
      interactiveId: string,
      input: UpdateInteractiveInput,
    ) => Promise<Interactive>;
  },
  saved: Interactive,
  known: readonly Interactive[],
): Promise<LinkBackOutcome> {
  const partnerId = partnerOf(saved);
  if (!partnerId) {
    return { kind: "nothing" };
  }
  const partner = known.find((each) => each.interactiveId === partnerId);
  if (partner) {
    if (partnerOf(partner) === saved.interactiveId) {
      return { kind: "nothing" };
    }
    if (partner.effectId !== null && partner.effectId !== TRAVEL_EFFECT_ID) {
      return {
        kind: "refused",
        message:
          "Saved one way only: the other end already does something else. Give it the travel effect yourself to link it back.",
      };
    }
  }
  try {
    await api.updateInteractive(partnerId, {
      effectId: TRAVEL_EFFECT_ID,
      effectConfig: { [PARTNER_KEY]: saved.interactiveId },
    });
    return { kind: "linked" };
  } catch {
    return {
      kind: "refused",
      message: "Saved one way only: the other end could not be linked back.",
    };
  }
}
