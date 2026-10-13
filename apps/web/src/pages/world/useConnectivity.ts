import {
  useEffect,
  useMemo,
  useRef,
  useSyncExternalStore,
  type Dispatch,
  type RefObject,
  type SetStateAction,
} from "react";
import {
  getLiveSyncState,
  loadLightsIntoStore,
  loadShapesIntoStore,
  loadTokensIntoStore,
  loadWallsIntoStore,
  catchUpWorldEvents,
  subscribeToLiveSyncState,
} from "@/engine/world/sync";
import {
  queueAdjudicatedChange,
  reconcileWorld,
} from "@/engine/world/sync/offlineQueue";
import {
  startHeartbeat,
  stopHeartbeat,
  subscribeToHeartbeat,
} from "@/engine/world/sync/heartbeat";
import { onPlayPaused } from "@/api/playPauseSignal";
import {
  pruneApplied,
  type AppliedChange,
} from "@/engine/world/sync/reconcile";
import { reportSessionPeers } from "@/engine/world/sync/subscriptionClient";
import {
  beginPeerAdjudication,
  endPeerAdjudication,
  peerAdjudicationActive,
  stopPeerTransfer,
} from "@/engine/bevy";
import {
  getPeerTransferState,
  subscribeToPeerTransfer,
} from "@/services/peerTransfer";
import type { WorldStore } from "@/engine/world/store";
import { getScenes } from "@/api/scenes";
import { useWorldMembers } from "@/hooks/useWorldMembers";
import { runsTheWorld } from "@/types/world";
import type { SceneRecord } from "@/types/scene";
import type { AuthUser } from "@/types/auth";
import type { ReconcileReportState } from "./worldPageHelpers";

interface UseConnectivityParams {
  id: string;
  sceneId: string | null;
  user: AuthUser | null;
  worldStore: WorldStore;
  appliedRef: RefObject<AppliedChange[]>;
  setReconcileReport: Dispatch<SetStateAction<ReconcileReportState | null>>;
  setScenes: Dispatch<SetStateAction<SceneRecord[]>>;
}

/**
 * This tab's connection to the table, and what it does when that changes.
 *
 * Moved out of `WorldPage` unchanged: the live-sync state, the reconcile
 * options, leaving play on a pause, the session heartbeat, peer adjudication
 * while the server is away, and the replay and refetch on reconnect. Returns
 * the live-sync state the connection indicator shows.
 */
export function useConnectivity({
  id,
  sceneId,
  user,
  worldStore,
  appliedRef,
  setReconcileReport,
  setScenes,
}: UseConnectivityParams) {
  // Spec 005 (T014-T016, data-model.md "LiveSyncState"): track this tab's
  // one shared subscription-transport connection state, so a persistent
  // "reconnecting" indicator can render (FR-009/FR-009a) and so a dropped
  // connection triggers a full scene re-fetch once it comes back — the
  // real gap this project's earlier audit found (subscriptionClient.ts
  // had automatic WebSocket retry via graphql-ws, but nothing reacted to
  // a reconnect to recover events missed during the outage).
  // Read through `useSyncExternalStore`, not `useState` + an effect.
  //
  // The connection lives in a module, outside React, and it can reach `live`
  // in the gap between this component rendering and its effects running —
  // which is not an edge case but what a cold load of /play does every time,
  // because the ack lands while the engine's wasm still holds the main
  // thread. `subscribeToLiveSyncState` deliberately does not replay the
  // current value, so a snapshot taken during render and a listener attached
  // afterwards missed that transition entirely, and on a healthy connection
  // there is no later transition to correct it: the "Connecting…" banner
  // stayed on screen for the whole session while live sync worked perfectly.
  //
  // `useSyncExternalStore` closes the gap by construction — it re-reads the
  // snapshot when it subscribes — which is the problem it exists to solve.
  const liveSyncState = useSyncExternalStore(
    subscribeToLiveSyncState,
    getLiveSyncState,
  );
  const sceneIdRef = useRef(sceneId);
  useEffect(() => {
    sceneIdRef.current = sceneId;
  }, [sceneId]);
  // The world id, read the same way: the reconnect handler below runs from a
  // subscription callback and must see the id as it is *now*, not as it was
  // when the effect was created.
  const idRef = useRef(id);
  useEffect(() => {
    idRef.current = id;
  }, [id]);
  /**
   * What a reconcile is allowed to do on this page (spec 028 US7, T103).
   *
   * `revert` is FR-062: a change the server refused must stop being shown, and
   * the only authority on what to show instead is the server — so the revert
   * is a re-read of the scene's tokens, the same one a reconnect already does,
   * asked for at the moment it is required. `selfUserId` is what lets a
   * refusal of a peer-adjudicated change name whose work it was.
   */
  const reconcileOptions = useMemo(
    () => ({
      selfUserId: user?.id,
      revert: () => {
        const currentSceneId = sceneIdRef.current;
        return currentSceneId
          ? loadTokensIntoStore(worldStore, currentSceneId)
          : undefined;
      },
    }),
    [user?.id, worldStore],
  );
  /**
   * The same options, read through a ref by the two reconnect listeners.
   *
   * They need what a reconcile may do to be *current* when a reconnect
   * happens, but naming `reconcileOptions` as a dependency would tear down
   * and re-establish the heartbeat and the live-sync subscription every time
   * the signed-in user or the world store changed — restarting the very
   * machinery whose whole job is to survive an interruption. Same reason
   * `idRef` and `sceneIdRef` exist above.
   */
  const reconcileOptionsRef = useRef(reconcileOptions);
  useEffect(() => {
    reconcileOptionsRef.current = reconcileOptions;
  }, [reconcileOptions]);
  // Only a transition into `live` *after* having already been live once
  // counts as a reconnect worth re-fetching for — the very first
  // `connecting` -> `live` transition on initial mount is already covered
  // by the four loadXIntoStore effects above, and re-running them again
  // here would just be redundant (harmless, since they're idempotent
  // upserts per research.md §5, but pointless).
  //
  // Seeded from the connection's *current* state, not `false`. The socket is
  // module-level and connects on the first `subscribeToWorldEvents`, which
  // happens before this effect subscribes — so on a normal load this listener
  // never sees the initial `connecting -> live` at all, and its first observed
  // transition is already a reconnect. Starting at `false` meant that
  // reconnect was mistaken for a first connect and **nothing was recovered**:
  // not the catch-up, and not the scene refetch this ref was written for.
  // Being already `live` when we start listening is exactly what "has been
  // live once" means.

  /**
   * Spec 051 US1: leave play when an operator pauses this world.
   *
   * The signal is routed centrally (`api/playPauseSignal`), and the app's
   * router is what goes to `/world/:id/paused`. This page does the half only
   * it can: stop what outlives an unmount. The heartbeat is module-level and
   * the peer channels belong to the engine, which persists across route
   * changes, so neither would close with the page. The world subscriptions
   * are each effect's own and are disposed as the page unmounts behind the
   * navigation.
   *
   * Nothing is announced here and no error is shown. The person has done
   * nothing wrong, and the notice says what happened.
   */
  useEffect(() => {
    if (!id) return;
    return onPlayPaused((worldId) => {
      if (worldId !== id) return;
      stopHeartbeat();
      stopPeerTransfer();
      // US2 (FR-023): submit anything queued offline now, while the page and
      // its revert still exist. A paused world refuses it all, and the
      // reconcile hands the count to the notice. Usually already running:
      // stopping the heartbeat of a page that was offline reads as
      // recovering, which reconciles, and this call joins that one. Nothing
      // queued is the common case, and costs one read of an empty outbox.
      void reconcileWorld(id, reconcileOptionsRef.current).catch(
        () => undefined,
      );
    });
  }, [id]);

  /**
   * The session heartbeat (spec 028 US7).
   *
   * Started for as long as a world is open. It is what tells the server this
   * client is still at the table — so a Game Master can be told when someone
   * drops — and what tells this client whether its edits can be sent at all.
   * The WebSocket cannot answer the second question: `graphql-ws` is lazy and
   * lets its connection go whenever nothing is subscribed.
   */
  useEffect(() => {
    if (!id) return;
    const stop = startHeartbeat(id, () => sceneIdRef.current);
    const unsubscribe = subscribeToHeartbeat((offline) => {
      if (offline) return;
      // Recovered. Replay before anything refetches server state, or the
      // refetch overwrites the local view with the pre-reconcile truth and
      // the user watches their offline work vanish and reappear.
      const worldIdNow = idRef.current;
      if (!worldIdNow) return;
      void reconcileWorld(worldIdNow, reconcileOptionsRef.current)
        .then((report) => {
          if (!report) return;
          appliedRef.current = [
            ...pruneApplied(appliedRef.current, Date.now()),
            ...report.applied.map((change) => ({
              ...change,
              appliedAt: Date.now(),
            })),
          ];
          setReconcileReport({ ...report, superseded: [] });
        })
        .catch(() => {
          // Everything stays queued and goes again on the next recovery.
        });
    });
    return () => {
      unsubscribe();
      stop();
    };
    // `appliedRef` and the setter are the page's own: stable, so this still
    // restarts only with the world.
  }, [id, appliedRef, setReconcileReport]);

  /**
   * The third connectivity state (spec 028 US7, T096, FR-055 to FR-059).
   *
   * This page is the only place that holds both halves of the question. Who
   * was at the table — which is the peer roster as it stood while there was
   * still a server to agree it with — and who is reachable now, which the
   * peer indicator already counts. Neither half is enough alone: a count of
   * open channels says nothing about who is missing, and a roster says
   * nothing about who is still there.
   *
   * Whether the **Game Master** is among them is not answerable from counts
   * at all, so it is not answered here. The peer fabric learns it from the
   * channel that identifies itself as the user the server named, and reports
   * it back through `peerAdjudicationActive` (FR-059).
   *
   * The safe direction is everywhere the same: not knowing reads as "no", and
   * "no" is plain offline, where the outbox already handles everything.
   */
  const { members } = useWorldMembers(id);
  const gmUserId = useMemo(() => {
    const gm = members.find((member) => runsTheWorld(member.role));
    return gm?.user_id ?? null;
  }, [members]);

  /**
   * How many people were at the table while the server was still answering.
   *
   * The high-water mark of connected peers while live, and not the current
   * count — the current count is exactly what an outage changes, so using it
   * as the roster would make "everyone is here" true by definition and the
   * full-connectivity rule meaningless.
   *
   * It errs high: somebody who legitimately left before the outage keeps the
   * roster larger than the table really is, and adjudicated play is refused.
   * That is the direction to err in. The cost is a session that falls back to
   * plain offline; the cost of erring the other way is two halves of a
   * partition both playing.
   */
  const tableSizeRef = useRef(0);

  useEffect(() => {
    if (!id || !user) return;
    let cancelled = false;

    /**
     * Apply an adjudicated move to the scene.
     *
     * Provisional, and merged into the token as it stands rather than
     * replacing it: the proposal carries only the fields it changed, because
     * position, rotation and scale are the only fields it is allowed to carry
     * (FR-060).
     */
    const applyAdjudicated = (changeJson: string) => {
      try {
        const change = JSON.parse(changeJson) as {
          entityId?: string;
          originUser?: string;
          transform?: {
            x?: number;
            y?: number;
            rotation?: number;
            scale?: number;
          };
        };
        const tokenId = change.entityId;
        const transform = change.transform;
        if (!tokenId || !transform) return;
        const existing = worldStore.getState().tokens[tokenId];
        if (!existing) return;
        const adjudicated = {
          type: "upsert_token" as const,
          token: {
            ...existing,
            ...(transform.x !== undefined ? { x: transform.x } : {}),
            ...(transform.y !== undefined ? { y: transform.y } : {}),
            ...(transform.rotation !== undefined
              ? { rotation: transform.rotation }
              : {}),
            ...(transform.scale !== undefined
              ? { scale: transform.scale }
              : {}),
          },
        };

        // Dispatched as `sync`, not `ui`.
        //
        // This is an outcome arriving, not somebody authoring at this
        // keyboard. Sending it as `ui` put it through the token mutation
        // bridge, which — being disconnected — queued it by the ordinary
        // offline path, as this client's own edit with no author on it. The
        // attribution the whole of T102/T103 exists to carry was dropped
        // exactly where it was created, and on a player's client the change
        // was queued a second time by someone who must never submit it.
        // `sync` also teaches the bridge this token's id, which is what
        // stops a later drag of it reading as a first sighting.
        worldStore.dispatch(adjudicated, "sync");

        // Only the Game Master's client owes the server anything. Submission
        // rides their session and the server checks the role (FR-061), so a
        // player queueing this would be queueing a change it cannot submit
        // and that nobody would ever drain.
        if (id && gmUserId && user.id === gmUserId) {
          void queueAdjudicatedChange({
            worldId: id,
            localId: crypto.randomUUID(),
            kind: "move",
            command: adjudicated,
            // Whoever actually made the move. An origin we cannot read falls
            // back to the submitter rather than to a guess at a third party:
            // naming the wrong person is the one failure here that is worse
            // than naming nobody, and the server treats "attributed to the
            // submitter" as an ordinary unattributed change. Dropping it
            // instead would lose the edit, which is the unrecoverable error.
            originatorUserId:
              typeof change.originUser === "string"
                ? change.originUser
                : user.id,
          });
        }
      } catch {
        // A change that does not parse is not a change. Nothing here is the
        // record of anything, and the server will send the truth on
        // reconnection regardless.
      }
    };

    let adjudicating = false;

    const publish = async () => {
      if (cancelled) return;
      const peers = getPeerTransferState();
      // Peer transfer off means no peer connections were ever opened, which
      // is what forfeits server-isolated play — exactly as the setting warns.
      if (!peers.enabled) {
        reportSessionPeers(null);
        return;
      }

      const live = getLiveSyncState().status === "live";
      if (live) {
        tableSizeRef.current = Math.max(
          tableSizeRef.current,
          peers.connectedPeers,
        );
        if (adjudicating) {
          // The server is the arbiter again. Everything adjudicated is now
          // owed a submission, which is why this says *why* it is stopping
          // rather than simply stopping (FR-062).
          adjudicating = false;
          void endPeerAdjudication(true);
        }
        reportSessionPeers({
          expected: tableSizeRef.current,
          reachable: peers.connectedPeers,
          gmReachable: false,
        });
        return;
      }

      // Not live. Ask the fabric to start — it refuses unless there is a
      // table to play with, and on a player's client it stays inactive until
      // the Game Master's channel has said who it belongs to.
      if (!adjudicating && gmUserId && tableSizeRef.current > 0) {
        adjudicating = await beginPeerAdjudication(
          user.id,
          gmUserId,
          applyAdjudicated,
        );
      }
      const gmReachable = adjudicating ? await peerAdjudicationActive() : false;
      if (cancelled) return;
      reportSessionPeers({
        expected: tableSizeRef.current,
        reachable: peers.connectedPeers,
        gmReachable,
      });
    };

    void publish();
    // Driven by the peer fabric's own reports rather than a poller of its
    // own: the engine already refreshes those every second, and a second
    // clock here would be a second answer to one question.
    const unsubscribePeers = subscribeToPeerTransfer(() => {
      void publish();
    });
    const unsubscribeSync = subscribeToLiveSyncState(() => {
      void publish();
    });

    return () => {
      cancelled = true;
      unsubscribePeers();
      unsubscribeSync();
      reportSessionPeers(null);
      if (adjudicating) void endPeerAdjudication(false);
    };
  }, [id, user, gmUserId, worldStore]);

  const wasLiveRef = useRef(false);
  useEffect(() => {
    // Read when the listener is actually attached, not during render: the
    // socket is module-level and can finish connecting in the gap between
    // this component rendering and its effects running, which is exactly
    // what happens on a normal load.
    if (getLiveSyncState().status === "live") {
      wasLiveRef.current = true;
    }
    const unsubscribe = subscribeToLiveSyncState((state) => {
      if (state.status === "live") {
        const currentSceneId = sceneIdRef.current;
        const worldIdNow = idRef.current;

        // Replay first, refetch second.
        //
        // The loaders below recover *scene content* — walls, lights, shapes,
        // tokens — which was the original fix for a reconnect losing updates.
        // They do not recover anything else that happened in the gap: a scene
        // launched, a chat message posted, combat advanced. Those arrive only
        // as events, and events during an outage were simply lost.
        //
        // The catch-up asks the durable record for exactly those events and
        // feeds them through the same handlers that would have run live, so a
        // reconnect now recovers the whole world rather than four of its
        // tables. When the gap is too large to replay the server says so, and
        // the full refetch below is the fallback that already existed.
        // Spec 028 US7: replay what was edited while there was nowhere to
        // send it, before the loaders below refetch server state — otherwise
        // a refetch would overwrite the local view with the pre-reconcile
        // truth and the user would watch their offline work vanish and then
        // reappear.
        if (wasLiveRef.current && worldIdNow) {
          void reconcileWorld(worldIdNow, reconcileOptionsRef.current)
            .then((report) => {
              if (!report) return;
              // Remember what applied, so a later Game Master reconnect can
              // be recognised as overriding it. Pruned on the way in rather
              // than on a timer: the list is only ever read here and when an
              // event arrives, so there is nothing for a timer to be timely
              // for.
              appliedRef.current = [
                ...pruneApplied(appliedRef.current, Date.now()),
                ...report.applied.map((change) => ({
                  ...change,
                  appliedAt: Date.now(),
                })),
              ];
              setReconcileReport({ ...report, superseded: [] });
            })
            .catch(() => {
              // Everything stays queued and goes again next reconnect. The
              // user has already been told they were offline; a second
              // failure notice here would be noise they cannot act on.
            });
        }

        if (wasLiveRef.current && worldIdNow) {
          void catchUpWorldEvents(worldIdNow)
            .then((outcome) => {
              if (outcome === "resync-required") {
                console.warn(
                  "Missed too many world events to replay; resynchronising.",
                );
                void getScenes(worldIdNow)
                  .then(setScenes)
                  .catch(() => {});
              }
            })
            .catch(() => {
              // Never fatal: the scene loaders below still run.
            });
        }

        if (wasLiveRef.current && currentSceneId) {
          void loadWallsIntoStore(worldStore, currentSceneId).catch((error) => {
            console.error(
              "Failed to re-fetch scene walls after reconnect:",
              error,
            );
          });
          void loadLightsIntoStore(worldStore, currentSceneId).catch(
            (error) => {
              console.error(
                "Failed to re-fetch scene lights after reconnect:",
                error,
              );
            },
          );
          void loadShapesIntoStore(worldStore, currentSceneId).catch(
            (error) => {
              console.error(
                "Failed to re-fetch scene shapes after reconnect:",
                error,
              );
            },
          );
          void loadTokensIntoStore(worldStore, currentSceneId).catch(
            (error) => {
              console.error(
                "Failed to re-fetch scene tokens after reconnect:",
                error,
              );
            },
          );
        }
        wasLiveRef.current = true;
      }
    });

    return unsubscribe;
    // `appliedRef` and the setters are the page's own: stable, so this still
    // re-subscribes only with the store.
  }, [worldStore, appliedRef, setReconcileReport, setScenes]);

  return liveSyncState;
}
