import {
  useEffect,
  useRef,
  type Dispatch,
  type RefObject,
  type SetStateAction,
} from "react";
import {
  parseSceneLaunchedEvent,
  startRollSync,
  subscribeToWorldEvents,
} from "@/engine/world/sync";
import { reportPlayPaused } from "@/api/playPauseSignal";
import {
  parseReconciledEvent,
  supersededBy,
  type AppliedChange,
} from "@/engine/world/sync/reconcile";
import { setReducedMotion, triggerDiceRollAnimation } from "@/engine/bevy";
import { watchReducedMotion } from "@/engine/bevy/diceThrow";
import type { ReconcileReportState } from "./worldPageHelpers";

interface UseWorldEventStreamsParams {
  id: string;
  engineReady: boolean;
  setSelectedSceneId: Dispatch<SetStateAction<string | null>>;
  appliedRef: RefObject<AppliedChange[]>;
  userIdRef: RefObject<string | null>;
  setReconcileReport: Dispatch<SetStateAction<ReconcileReportState | null>>;
}

/**
 * The world-level event streams, open for as long as the world is: scene
 * launches, play pauses and reconcile supersession; rolls; and the reduced
 * motion preference the dice honour.
 *
 * Moved out of `WorldPage` unchanged.
 */
export function useWorldEventStreams({
  id,
  engineReady,
  setSelectedSceneId,
  appliedRef,
  userIdRef,
  setReconcileReport,
}: UseWorldEventStreamsParams) {
  // Spec 022 (FR-002b/FR-002d, ADR-046): a second, independent
  // subscription that's open as soon as `id` is known — deliberately NOT
  // gated on `sceneId`/`bridgeReady` like the content-sync subscription
  // below, because its whole job is to *set* `selectedSceneId` (including
  // from `null`, before anything has ever been launched). Two open
  // subscriptions to the same `worldEventsCreated` stream once a scene is
  // selected is an accepted tradeoff for keeping this concern decoupled
  // from the content-sync effect's sceneId-scoped lifecycle.
  useEffect(() => {
    if (!id) {
      return;
    }

    /**
     * Spec 051: `EVENT_CODE_WORLD_PLAY_PAUSED` in `world_events.rs`. An
     * operator paused this world's play. Declared beside its only handler, as
     * every event code in this app is.
     *
     * The event carries `pausedAt` and nothing else, and nothing here reads
     * even that: the notice asks `worldPlayState` for itself.
     */
    const EVENT_CODE_WORLD_PLAY_PAUSED = 28;

    const iterator = subscribeToWorldEvents(id)[Symbol.asyncIterator]();
    let cancelled = false;

    (async () => {
      try {
        while (!cancelled) {
          const { value: event, done } = await iterator.next();
          if (done || cancelled || !event) break;
          // This subscription, and not the scene's content one below, because
          // it is open from the moment the world is: a table that has not
          // launched a scene yet is still a table that has to leave.
          if (
            (event.event_code ?? event.eventCode) ===
            EVENT_CODE_WORLD_PLAY_PAUSED
          ) {
            reportPlayPaused(id);
            break;
          }
          const launchedSceneId = parseSceneLaunchedEvent(event);
          if (launchedSceneId) {
            setSelectedSceneId(launchedSceneId);
          }

          // Spec 028 FR-041, the `Applied → Superseded` case. A change this
          // client applied at its own reconnect can be overridden minutes
          // later by a Game Master reconnecting with a conflicting offline
          // edit. There is no response left to carry that news — this client
          // is long gone from that reconcile call — so it arrives here, as an
          // ordinary world event, and is only recognisable as *supersession*
          // because the event says it was a replay and says who made it.
          const reconciled = parseReconciledEvent(event);
          if (reconciled && userIdRef.current) {
            const hits = supersededBy(
              reconciled,
              appliedRef.current,
              userIdRef.current,
            );
            if (hits.length > 0) {
              appliedRef.current = appliedRef.current.filter(
                (change) => !hits.some((hit) => hit.localId === change.localId),
              );
              setReconcileReport((current) => ({
                applied: current?.applied ?? [],
                rejected: current?.rejected ?? [],
                unanswered: current?.unanswered ?? [],
                stillQueued: current?.stillQueued ?? [],
                superseded: [
                  ...(current?.superseded ?? []),
                  ...hits.map((change) => ({
                    change,
                    byRole: reconciled.byRole,
                  })),
                ],
              }));
            }
          }
        }
      } catch (error) {
        console.error("Scene-launch live-sync error:", error);
      }
    })();

    return () => {
      cancelled = true;
      void iterator.return?.();
    };
    // The setters and refs are the page's own: stable, so this still opens
    // one stream per world.
  }, [id, setSelectedSceneId, appliedRef, userIdRef, setReconcileReport]);

  // Spec 081: every roll at the table lands on this board, this client's
  // own included — the dice animate from the roll's world event, never from
  // the panel that rolled it, which is what lets a sheet in another tab roll
  // onto this board. World-level rather than per scene, as rolls are. Read
  // through a ref so the engine coming up does not reopen the stream, and
  // checked at each roll so a board without an engine never loads one.
  const engineReadyRef = useRef(engineReady);
  useEffect(() => {
    engineReadyRef.current = engineReady;
  }, [engineReady]);
  useEffect(() => {
    if (!id) {
      return;
    }
    return startRollSync({
      worldId: id,
      events: subscribeToWorldEvents(id),
      animate: (roll) => {
        if (engineReadyRef.current) {
          void triggerDiceRollAnimation(roll);
        }
      },
    });
  }, [id]);

  // Spec 083 (research R11): the engine throws dice landed and faded in for
  // a viewer who prefers reduced motion. Sent once the engine is up, and
  // again whenever the preference changes.
  useEffect(() => {
    if (!engineReady) {
      return;
    }
    return watchReducedMotion((reduced) => {
      void setReducedMotion(reduced);
    });
  }, [engineReady]);
}
