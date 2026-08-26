/**
 * Adjudicator implementations.
 *
 * The facets never learn which of these they are holding, which is the
 * point: today every proposal is accepted locally and applied optimistically
 * — exactly what the client already did before facets existed — and wiring
 * Crucible in later is a constructor change, not a rewrite of every call
 * site.
 *
 * This mirrors the split the Rust side already made: `SessionAdjudicator`
 * with a `LocalAdjudicator` (in-process, what a self-hosted deployment gets
 * by default) and a `RemoteAdjudicator` (HTTP to `crucible-server`). Callers
 * there depend only on the trait; callers here depend only on `Adjudicator`.
 */

import type { Adjudicator, IntentResult, Proposal } from "./types";

/**
 * Accepts everything, unchanged.
 *
 * Not a stub to be replaced but the honest description of current
 * behaviour: there is no server-side ruleset yet, the client is the
 * simulation, and every local action already succeeds. Naming that as a
 * policy means the day a real ruleset starts rejecting moves, the change is
 * visible in one line of wiring rather than diffused through the UI.
 */
export function createLocalAdjudicator(): Adjudicator {
  return {
    async resolve<TPayload>(proposal: Proposal<TPayload>): Promise<IntentResult<TPayload>> {
      return { status: "accepted", value: proposal.payload };
    },
  };
}

/**
 * Sends proposals to a Crucible endpoint and reports its verdict.
 *
 * Deliberately fails *closed*. If the adjudicator cannot be reached, the
 * intent is refused rather than optimistically applied: an unreachable
 * server ruleset means the client does not know whether the action is legal,
 * and quietly acting as though it is would produce a board that disagrees
 * with the server and reconciles later by yanking tokens back.
 */
export function createRemoteAdjudicator(endpoint: string): Adjudicator {
  return {
    async resolve<TPayload>(proposal: Proposal<TPayload>): Promise<IntentResult<TPayload>> {
      let response: Response;
      try {
        response = await fetch(endpoint, {
          method: "POST",
          headers: { "content-type": "application/json" },
          credentials: "include",
          body: JSON.stringify({
            world_id: proposal.worldId,
            actor_id: proposal.actorId,
            // Crucible's `ActionKind` serialises capitalised.
            kind: proposal.kind === "move" ? "Move" : "Manipulate",
            payload: proposal.payload,
          }),
        });
      } catch {
        return { status: "refused", reason: "not-connected" };
      }

      if (!response.ok) {
        return { status: "rejected", reason: `adjudicator returned ${response.status}` };
      }

      const verdict = (await response.json()) as {
        outcome?: string;
        payload?: TPayload;
        reason?: string;
      };

      switch (verdict.outcome) {
        case "Accepted":
          return { status: "accepted", value: proposal.payload };
        case "Adjusted":
          return {
            status: "adjusted",
            // An `Adjusted` verdict with no corrected payload is a server
            // bug; treating it as "accepted as asked" would apply the very
            // position the ruleset just declined to grant.
            value: verdict.payload ?? proposal.payload,
            requested: proposal.payload,
          };
        case "Rejected":
          return { status: "rejected", reason: verdict.reason ?? "rejected" };
        default:
          return { status: "rejected", reason: "unrecognised adjudicator outcome" };
      }
    },
  };
}
