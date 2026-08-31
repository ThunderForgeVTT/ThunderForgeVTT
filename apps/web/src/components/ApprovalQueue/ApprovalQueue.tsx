import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Panel } from "@/components/ui/panel/Panel";
import {
  approveRequest,
  getPendingRequests,
  refuseRequest,
  type InteractionRequest,
} from "@/api/interactives";

/**
 * What the table has asked for, and has not been given yet.
 *
 * Spec 030, US6. A player steps onto the stairs and *asks*; this is where the
 * Game Master decides. Nothing in it moves anybody — approval does, and only
 * when the GM says so.
 *
 * # Why there is no timer anywhere in this component
 *
 * FR-027: a request must never expire into approval. Silence is not consent,
 * and a queue that eventually says yes on the GM's behalf is a queue that
 * decides things they did not. There is deliberately no auto-approve, no
 * countdown, and no default action — a request that nobody answers stays
 * exactly where it is, which is the correct behaviour and also the one a busy
 * GM relies on.
 *
 * # Why it is read from the server rather than accumulated locally
 *
 * A Game Master on a second device must see the same queue, and one who
 * refreshes mid-session must not lose what is in it. That is the reason
 * requests are a table rather than memory (research §7).
 */

export interface ApprovalQueueProps {
  sceneId: string;
  /** Re-read when this changes — a world event says something happened. */
  revision?: number;
  onDecided?: (requestId: string, approved: boolean) => void;
}

export function ApprovalQueue({
  sceneId,
  revision = 0,
  onDecided,
}: ApprovalQueueProps) {
  const [requests, setRequests] = useState<InteractionRequest[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      setRequests(await getPendingRequests(sceneId));
    } catch {
      // Distinguished from an empty queue on purpose. "Nobody has asked for
      // anything" and "I could not find out" look identical if both render as
      // an empty list, and only one of them is a reason to look at the logs.
      setRequests(null);
      setProblem("Could not read what the table has asked for.");
    }
  }, [sceneId]);

  useEffect(() => {
    // Guarded rather than fire-and-forget: a GM switching scenes quickly would
    // otherwise have the first scene's answer arrive last and win.
    let cancelled = false;
    getPendingRequests(sceneId)
      .then((pending) => {
        if (!cancelled) setRequests(pending);
      })
      .catch(() => {
        if (cancelled) return;
        setRequests(null);
        setProblem("Could not read what the table has asked for.");
      });
    return () => {
      cancelled = true;
    };
  }, [sceneId, revision]);

  const decide = useCallback(
    async (request: InteractionRequest, approve: boolean) => {
      setBusy(request.requestId);
      setProblem(null);
      try {
        const result = approve
          ? await approveRequest(request.requestId)
          : await refuseRequest(request.requestId);
        // Approval re-checks permission at decision time, so approving can
        // still come back refused — a door locked since the asking. Saying so
        // beats a queue that silently swallowed the contradiction.
        if (approve && result.outcome !== "performed") {
          setProblem(
            result.reason === "locked"
              ? "You locked that since they asked, so it stayed shut."
              : "That could not run after all.",
          );
        }
        onDecided?.(request.requestId, approve);
      } catch {
        setProblem("That did not go through.");
      } finally {
        setBusy(null);
        await reload();
      }
    },
    [onDecided, reload],
  );

  if (requests === null) {
    return (
      <Panel>
        <h3>Asked for</h3>
        {problem && <p role="alert">{problem}</p>}
      </Panel>
    );
  }

  return (
    <Panel>
      <h3>Asked for</h3>

      {requests.length === 0 && <p>Nothing is waiting on you.</p>}

      <ul>
        {requests.map((request) => (
          <li key={request.requestId}>
            <span>
              {request.requestedByName ?? "Someone"} wants to{" "}
              {request.proposed ?? "do something"}.
            </span>
            <Button
              disabled={busy === request.requestId}
              onClick={() => void decide(request, true)}
            >
              Let them
            </Button>
            <Button
              variant="ghost"
              disabled={busy === request.requestId}
              onClick={() => void decide(request, false)}
            >
              Not yet
            </Button>
          </li>
        ))}
      </ul>

      {problem && <p role="alert">{problem}</p>}
    </Panel>
  );
}
