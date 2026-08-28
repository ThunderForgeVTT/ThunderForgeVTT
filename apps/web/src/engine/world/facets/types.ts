/**
 * Facets: the capability surfaces a session is driven through.
 *
 * A facet is one scoped slice of what someone may do in a world — control
 * their own tokens, draw, watch what the server decided — expressed as a
 * typed API rather than as scattered dispatches. Three things pushed this
 * shape:
 *
 * 1. **Authority does not live in the client.** `thunderforge-crucible` is
 *    the `SessionAdjudicator`: a client *proposes* and the server *decides*
 *    (`Accepted` / `Rejected` / `Adjusted`). Code that dispatches straight
 *    into the world store cannot express "I asked and was told no", so the
 *    distinction has to exist in the types or it will not exist at all.
 *
 * 2. **Crucible's payload is untyped on purpose, for now.** Its
 *    `AdjudicationRequest.payload` is a bare `serde_json::Value`, with the
 *    crate noting that "a typed payload-per-`kind` is a natural evolution
 *    once the real ruleset needs to actually inspect it". These intent
 *    types are that typing, from the client side: every intent here maps
 *    onto one `ActionKind` and a payload shape the ruleset can rely on.
 *
 * 3. **A player is not a GM with fewer buttons.** Players control their
 *    own tokens — plural, and their drawings and measurements besides —
 *    while the GM controls the scene. Encoding that as a boolean at each
 *    call site is how it drifts; `Authority` makes each facet answer for
 *    itself who may do what.
 *
 * Nothing here renders, and nothing here decides. A facet turns an intent
 * into a proposal, hands it to whatever is adjudicating, and reports what
 * came back.
 */

import type { WorldToken } from "../types";

/**
 * Who is asking, as far as a facet is concerned.
 *
 * Deliberately not a boolean `isGm`. `"observer"` is a real, distinct case
 * — a spectator, or a player looking at a scene where they control nothing
 * — and it is the one that a boolean silently collapses into "player",
 * which then has to be re-checked everywhere downstream.
 */
export type Authority = "gm" | "player" | "observer";

/** Who is driving, and what they are allowed to reach. */
export interface FacetPrincipal {
  /** The viewing user. `null` while auth is still resolving. */
  userId: string | null;
  authority: Authority;
}

/**
 * Why an intent was refused before it was ever sent.
 *
 * Client-side refusals are separated from server verdicts on purpose: the
 * server rejecting a move is gameplay and belongs in the log, while the
 * client refusing to send one is a bug or a permissions boundary, and
 * conflating them makes both harder to read.
 */
export type RefusalReason =
  | "not-yours"
  | "gm-only"
  | "observer"
  | "unknown-subject"
  | "not-connected";

/**
 * What became of an intent.
 *
 * Mirrors Crucible's `Outcome` — `Accepted` / `Rejected` / `Adjusted` —
 * with `refused` added for the never-sent case above. `adjusted` carries
 * the authoritative correction, which is the whole point of having a
 * server ruleset: the client asked to move somewhere and the server says
 * where the token actually ended up.
 */
export type IntentResult<T> =
  | { status: "accepted"; value: T }
  | { status: "adjusted"; value: T; requested: T }
  | { status: "rejected"; reason: string }
  | { status: "refused"; reason: RefusalReason };

/** Convenience guard: did anything actually change? */
export function didApply<T>(
  result: IntentResult<T>,
): result is Extract<IntentResult<T>, { status: "accepted" | "adjusted" }> {
  return result.status === "accepted" || result.status === "adjusted";
}

/**
 * The action kinds Crucible adjudicates, named as it names them.
 *
 * Kept in lockstep with `crates/thunderforge-crucible/src/lib.rs`'s
 * `ActionKind` rather than invented here — if that enum grows, this is the
 * one place the client has to follow.
 */
export type ActionKind = "move" | "manipulate";

/**
 * A proposal, in the shape Crucible receives it.
 *
 * `payload` is typed per intent by the facets below; this is the envelope
 * they all serialise into, so an adjudicator transport can be written once
 * against this type instead of once per facet.
 */
export interface Proposal<TPayload = unknown> {
  worldId: string;
  /** The actor on whose behalf this is proposed. */
  actorId: string;
  kind: ActionKind;
  payload: TPayload;
}

/**
 * The seam a facet resolves proposals through.
 *
 * One method, so the local pass-through (today's behaviour: dispatch and
 * assume success) and a real Crucible round trip are interchangeable, and
 * a facet never learns which it has. This is the client-side mirror of the
 * `SessionAdjudicator` trait.
 */
export interface Adjudicator {
  resolve<TPayload>(
    proposal: Proposal<TPayload>,
  ): Promise<IntentResult<TPayload>>;
}

/** Everything a facet needs to be constructed. */
export interface FacetContext {
  worldId: string;
  sceneId: string | null;
  principal: FacetPrincipal;
  adjudicator: Adjudicator;
}

/**
 * A token together with what this principal may do to it.
 *
 * The permissions are resolved once, here, rather than recomputed at each
 * button: a panel that greys out a control and a facet that refuses the
 * intent must agree, and the only way to guarantee that is for both to read
 * the same answer.
 */
export interface ControllableToken {
  token: WorldToken;
  canMove: boolean;
  canRotate: boolean;
  canResize: boolean;
  canSetArt: boolean;
  canDelete: boolean;
}
