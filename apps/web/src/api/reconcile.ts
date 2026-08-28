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
      discrepancy {
        userId
        recordId
        reportedValue
        determinedValue
      }
    }
  }
`;

export interface QueuedChangePayload {
  localId: string;
  command: unknown;
  /**
   * Who actually made this change, when it is not the person submitting it.
   *
   * Sent as its own input field rather than left inside the command, because
   * the command is opaque to the server by design — it is replayed through
   * the ordinary mutation path and nothing unpacks it. The attribution has
   * to be somewhere the server *does* read, or FR-061's role check has
   * nothing to check. Absent means "the submitter made it", which is the
   * ordinary case and the safe reading.
   */
  attributedToUserId?: string;
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
  const data = await postGraphQL<{
    reconcileQueuedChanges: ReconcileOutcome[];
  }>(RECONCILE_MUTATION, { worldId, changes });
  return data.reconcileQueuedChanges;
}
