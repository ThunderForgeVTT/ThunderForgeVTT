import { postGraphQL } from "./graphqlClient";
import type { ReconcileOutcome } from "@/engine/world/sync/reconcile";

/**
 * `reconcileQueuedChanges` (spec 028 US7, contracts/graphql-delta-sync.md).
 *
 * The commands travel as opaque JSON. Nothing on this side interprets one —
 * the server replays it through the ordinary mutation path, which is what
 * makes re-authorization at reconnect automatic rather than a second
 * mechanism to keep in step.
 */
const RECONCILE_MUTATION = `
  mutation ReconcileQueuedChanges($worldId: UUID!, $changes: [QueuedChangeInput!]!) {
    reconcileQueuedChanges(worldId: $worldId, changes: $changes) {
      localId
      applied
      reason
      supersededByRole
    }
  }
`;

export interface QueuedChangePayload {
  localId: string;
  command: unknown;
}

/**
 * Submit queued changes and return one outcome per change.
 *
 * Throws on transport failure, deliberately: the caller's response to "the
 * call did not happen" is to leave everything queued, which is a different
 * decision from "the server refused this change" and must not be confused
 * with it.
 */
export async function submitQueuedChanges(
  worldId: string,
  changes: QueuedChangePayload[],
): Promise<ReconcileOutcome[]> {
  const data = await postGraphQL<{ reconcileQueuedChanges: ReconcileOutcome[] }>(
    RECONCILE_MUTATION,
    { worldId, changes },
  );
  return data.reconcileQueuedChanges;
}
