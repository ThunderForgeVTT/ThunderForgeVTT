import type {
  CreateInteractiveInput,
  Interactive,
} from "@/api/interactives";
import type { TokenRecord } from "@/types/token";

/**
 * Turning a confirmed drop into a thing on the map (spec 031 FR-011).
 *
 * # Why this is a module and not a handler inside the panel
 *
 * Two server calls in sequence, where the second can fail after the first has
 * succeeded. That partial state is the whole reason this is worth testing, and
 * `apps/web`'s vitest environment is `node` — a component would only be
 * reachable through `renderToStaticMarkup`, which cannot drive an async
 * sequence at all. So the sequence lives here as a plain function over
 * injected calls, and the panel is left as the thing that draws the outcome.
 *
 * # Why a prop is created before its interactive, and never the other way
 *
 * An interactive points at a subject, so the subject has to exist first. The
 * consequence is the outcome below that names itself honestly: a token can be
 * placed and then fail to be given its effect. Reporting that as a plain
 * failure would be a lie a Game Master discovers later, as a blank prop on the
 * map they do not remember placing.
 *
 * Deleting the token to "undo" was the considered alternative and is worse: it
 * is a third call that can also fail, and a failed cleanup loses the token for
 * good rather than leaving something the GM can see and fix.
 */

/** What the author panel produced, before anything exists to attach it to. */
export interface PropDraft {
  effectId: string | null;
  effectConfig: Record<string, unknown> | null;
  activation: string;
  fireMode: string;
}

/** Where the engine says the carry was dropped, already snapped. */
export interface DropPoint {
  x: number;
  y: number;
}

/**
 * The two calls this needs, injected.
 *
 * Named rather than imported so a test can watch the order and make the second
 * one fail — which is the case that matters and the one that cannot be
 * provoked against a real server.
 */
export interface PlacementCalls {
  placeProp: (
    sceneId: string,
    x: number,
    y: number,
  ) => Promise<Pick<TokenRecord, "tokenId">>;
  createInteractive: (
    input: CreateInteractiveInput,
  ) => Promise<Pick<Interactive, "interactiveId">>;
}

/**
 * What happened, as a tagged outcome rather than a boolean.
 *
 * The same shape, and for the same reason, as `activateInteractive`'s: three
 * situations that a boolean would flatten into one, and the middle one is the
 * only one a Game Master has to be told about in words.
 */
export type PlacementOutcome =
  | { kind: "placed"; tokenId: string; interactiveId: string }
  | { kind: "propOnly"; tokenId: string; message: string }
  | { kind: "refused"; message: string };

export async function placeAuthoredProp(
  calls: PlacementCalls,
  sceneId: string,
  at: DropPoint,
  draft: PropDraft,
): Promise<PlacementOutcome> {
  let tokenId: string;
  try {
    const token = await calls.placeProp(sceneId, at.x, at.y);
    tokenId = token.tokenId;
  } catch {
    // Nothing was created, so there is nothing to say beyond that. The map is
    // exactly as it was.
    return { kind: "refused", message: "That could not be placed." };
  }

  try {
    // An interactive is created even for scenery — an effect of `null` is a
    // legitimate authored thing (spec 030), and it is what makes the prop
    // appear in this scene's list and carry a badge rather than being an
    // anonymous token the Game Master cannot find again.
    const interactive = await calls.createInteractive({
      sceneId,
      subjectKind: "prop",
      subjectRef: tokenId,
      effectId: draft.effectId,
      effectConfig: draft.effectConfig,
      trigger: "click",
      activation: draft.activation,
      fireMode: draft.fireMode,
    });
    return { kind: "placed", tokenId, interactiveId: interactive.interactiveId };
  } catch {
    return {
      kind: "propOnly",
      tokenId,
      message:
        "Placed, but what it does could not be saved. Select it and try again.",
    };
  }
}
