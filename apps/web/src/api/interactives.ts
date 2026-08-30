import { postGraphQL } from "./graphqlClient";
import { createToken } from "./tokens";
import type { TokenRecord } from "@/types/token";

/**
 * Authoring, activating and approving interactive elements.
 *
 * Spec 030. Every rule this module appears to describe is actually enforced on
 * the server — a player is refused a locked door there, not here — so nothing
 * below checks permission. A second opinion in the client would be a second
 * thing to keep right, and the one that drifts is always the one people
 * believe.
 *
 * What this module does own is the *shape*: the authoring form is built from
 * `effectRegistry` rather than from a list written here, so a Game Master is
 * offered exactly what this build can perform and never an option that would
 * silently do nothing (FR-038).
 */

/** Which subjects an effect may attach to. */
export type SubjectKind = "prop" | "door" | "region";

/** How a configuration field is filled in. */
export type ConfigFieldKind =
  | "boolean"
  | "choice"
  | "reference"
  | "referenceList";

export interface ChoiceOption {
  value: string;
  label: string;
}

export interface ConfigField {
  key: string;
  label: string;
  kind: ConfigFieldKind;
  /** What a reference points at — `wall`, `light`, `loreEntry`, `scene`. */
  referenceOf: string | null;
  options: ChoiceOption[] | null;
  required: boolean;
}

/**
 * One effect this build can perform.
 *
 * Note what is absent: any notion of an effect being unavailable. The registry
 * is the union of what is compiled in, so a subsystem that does not exist
 * contributes nothing and there is no dead option to grey out.
 */
export interface EffectDeclaration {
  id: string;
  label: string;
  description: string;
  subjectKinds: SubjectKind[];
  config: ConfigField[];
}

/**
 * One authored interactive.
 *
 * A player receives a narrower version of this: the Game-Master-only fields
 * arrive as `null`. That is an interface boundary rather than a security one —
 * per the spec, secrets are a table concern — but sending a player an effect's
 * configuration would invite some future client to render it.
 */
export interface Interactive {
  interactiveId: string;
  sceneId: string;
  subjectKind: SubjectKind;
  subjectRef: string | null;
  geometry: unknown | null;
  trigger: "click" | "enter";
  effectId: string | null;
  effectConfig: Record<string, unknown> | null;
  activation: string | null;
  fireMode: string | null;
  firedAt: string | null;
  /** Whether the subsystem that performs the effect is in this build. */
  available: boolean | null;
  /** Whether this viewer's activation would do anything. A hint, not a right. */
  canActivate: boolean;
}

/** What happened, as a tagged outcome rather than a boolean. */
export interface ActivationResult {
  outcome: "performed" | "requested" | "refused" | "unavailable" | "noEffect";
  /** `gmOnly`, `locked` or `alreadyFired`. Present only when refused. */
  reason: string | null;
  requestId: string | null;
  effectId: string | null;
  effectConfig: Record<string, unknown> | null;
  /**
   * Things the Game Master should know about what just ran.
   *
   * Always empty for a player. These are notes about the *authoring* — a
   * switch naming a lamp that has been deleted — and a player has no use for
   * one and no way to act on it.
   */
  notices: string[];
}

const INTERACTIVE_FIELDS = `
  interactiveId
  sceneId
  subjectKind
  subjectRef
  geometry
  trigger
  effectId
  effectConfig
  activation
  fireMode
  firedAt
  available
  canActivate
`;

export async function getEffectRegistry(): Promise<EffectDeclaration[]> {
  const payload = await postGraphQL<{ effectRegistry: EffectDeclaration[] }>(
    `query {
      effectRegistry {
        id
        label
        description
        subjectKinds
        config { key label kind referenceOf options { value label } required }
      }
    }`,
    {},
  );
  return payload?.effectRegistry ?? [];
}

export async function getInteractives(sceneId: string): Promise<Interactive[]> {
  const payload = await postGraphQL<{ interactives: Interactive[] }>(
    `query ($sceneId: UUID!) {
      interactives(sceneId: $sceneId) { ${INTERACTIVE_FIELDS} }
    }`,
    { sceneId },
  );
  return payload?.interactives ?? [];
}

export interface CreateInteractiveInput {
  sceneId: string;
  subjectKind: SubjectKind;
  subjectRef?: string | null;
  geometry?: unknown;
  effectId?: string | null;
  effectConfig?: Record<string, unknown> | null;
  trigger: "click" | "enter";
  activation: string;
  fireMode?: string | null;
}

export async function createInteractive(
  input: CreateInteractiveInput,
): Promise<Interactive> {
  const payload = await postGraphQL<{ createInteractive: Interactive }>(
    `mutation ($input: GraphQLCreateInteractiveInput!) {
      createInteractive(input: $input) { ${INTERACTIVE_FIELDS} }
    }`,
    { input },
  );
  return payload.createInteractive;
}

export interface UpdateInteractiveInput {
  geometry?: unknown;
  effectId?: string | null;
  effectConfig?: Record<string, unknown> | null;
  trigger?: string | null;
  activation?: string | null;
  fireMode?: string | null;
  /**
   * Turn an interactive back into scenery.
   *
   * Needed because an absent `effectId` means "leave it alone" in a partial
   * update, so there would otherwise be no way to say "remove it".
   */
  clearEffect?: boolean;
}

export async function updateInteractive(
  interactiveId: string,
  input: UpdateInteractiveInput,
): Promise<Interactive> {
  const payload = await postGraphQL<{ updateInteractive: Interactive }>(
    `mutation ($id: UUID!, $input: GraphQLUpdateInteractiveInput!) {
      updateInteractive(interactiveId: $id, input: $input) { ${INTERACTIVE_FIELDS} }
    }`,
    { id: interactiveId, input },
  );
  return payload.updateInteractive;
}

export async function deleteInteractive(
  interactiveId: string,
): Promise<boolean> {
  const payload = await postGraphQL<{ deleteInteractive: boolean }>(
    `mutation ($id: UUID!) { deleteInteractive(interactiveId: $id) }`,
    { id: interactiveId },
  );
  return payload?.deleteInteractive ?? false;
}

export async function resetInteractive(
  interactiveId: string,
): Promise<Interactive> {
  const payload = await postGraphQL<{ resetInteractive: Interactive }>(
    `mutation ($id: UUID!) {
      resetInteractive(interactiveId: $id) { ${INTERACTIVE_FIELDS} }
    }`,
    { id: interactiveId },
  );
  return payload.resetInteractive;
}

/**
 * The one mutation a player calls.
 *
 * Returns what happened rather than whether it worked, because "it did not
 * run" covers four different situations and a player told only "no" cannot
 * tell a locked door from a broken product.
 */
export async function activateInteractive(
  interactiveId: string,
): Promise<ActivationResult> {
  const payload = await postGraphQL<{
    activateInteractive: ActivationResult;
  }>(
    `mutation ($id: UUID!) {
      activateInteractive(interactiveId: $id) {
        outcome
        reason
        requestId
        effectId
        effectConfig
        notices
      }
    }`,
    { id: interactiveId },
  );
  return payload.activateInteractive;
}

/**
 * Place a prop: a book, a chest, a lever.
 *
 * A prop is a token of the existing `object` kind with no actor. That is the
 * whole implementation, and deliberately so — token placement, artwork,
 * movement, ordering and live sync all exist and work, and a parallel "props"
 * concept would duplicate every one of them.
 *
 * The consequence to watch is that anything consuming tokens must tolerate one
 * with no actor. Spec 029 established that precedent: `tokenStatus` skips
 * actorless tokens, calling them markers rather than creatures.
 *
 * This wrapper exists to name the intent, not to add behaviour. If it ever
 * grows a second step, that is the signal a prop has stopped being a token.
 */
export function placeProp(
  sceneId: string,
  x: number,
  y: number,
  photoUrl?: string,
): Promise<TokenRecord> {
  return createToken({ sceneId, x, y, tokenType: "object", photoUrl });
}

/**
 * Make a wall a door, or stop it being one (FR-007).
 *
 * Designating also gives the door an interactive, so clicking it opens it
 * without the Game Master having to author one — a door nobody can touch is
 * not what "designate a door" means to anybody.
 */
export async function setDoorDesignation(
  wallId: string,
  isDoor: boolean,
): Promise<boolean> {
  const payload = await postGraphQL<{ setDoorDesignation: boolean }>(
    `mutation ($wallId: UUID!, $isDoor: Boolean!) {
      setDoorDesignation(wallId: $wallId, isDoor: $isDoor)
    }`,
    { wallId, isDoor },
  );
  return payload?.setDoorDesignation ?? false;
}

/** Change who may open a door, not whether it is open (FR-013). */
export async function setDoorLock(
  wallId: string,
  locked: boolean,
): Promise<boolean> {
  const payload = await postGraphQL<{ setDoorLock: boolean }>(
    `mutation ($wallId: UUID!, $locked: Boolean!) {
      setDoorLock(wallId: $wallId, locked: $locked)
    }`,
    { wallId, locked },
  );
  return payload?.setDoorLock ?? false;
}

/** Hide a door from the table, or show it. */
export async function setDoorSecret(
  wallId: string,
  secret: boolean,
): Promise<boolean> {
  const payload = await postGraphQL<{ setDoorSecret: boolean }>(
    `mutation ($wallId: UUID!, $secret: Boolean!) {
      setDoorSecret(wallId: $wallId, secret: $secret)
    }`,
    { wallId, secret },
  );
  return payload?.setDoorSecret ?? false;
}

/**
 * What to tell somebody whose activation did not run.
 *
 * `null` for an outcome that needs nothing said. Silence for a *refusal* is
 * indistinguishable from the product being broken, which is the whole reason
 * the outcome is tagged rather than boolean (FR-014) — but silence for a
 * performed effect is correct, because the effect is the feedback.
 */
export function refusalNotice(result: ActivationResult): string | null {
  switch (result.outcome) {
    case "performed":
      return null;
    case "requested":
      return "Asked the GM. Nothing happens until they say so.";
    case "unavailable":
      // Not a failure the player caused, and not one they can do anything
      // about. Saying so beats a click that appears to do nothing.
      return "This does not work in this session.";
    case "noEffect":
      return null;
    case "refused":
      switch (result.reason) {
        case "locked":
          return "It is locked.";
        case "gmOnly":
          return "Only the GM can do this.";
        case "alreadyFired":
          return "This has already happened.";
        default:
          return "That did not work.";
      }
    default:
      return null;
  }
}
