import type { Dispatch, SetStateAction } from "react";
import { Button } from "@/components/ui/button/Button";
import { ReconcileReport } from "@/components/world/ReconcileReport";
import { ConnectionStatus } from "@/components/world/ConnectionStatus";
import { PeerIndicator } from "@/components/diagnostics/PeerIndicator";
import { EngineMonitor } from "@/components/diagnostics/EngineMonitor";
import { EngineLoader } from "@/components/engine/EngineLoader";
import { EngineStopped } from "@/components/engine/EngineStopped";
import { BoardLoading } from "@/components/engine/BoardLoading";
import {
  describeSceneLoad,
  LOADING_PLATE,
  type ReconcileReportState,
} from "./worldPageHelpers";
import type { useEngineCanvas } from "./useEngineCanvas";
import type { useSceneLoadState } from "./useSceneLoadState";
import type { useConnectivity } from "./useConnectivity";

type EngineCanvas = ReturnType<typeof useEngineCanvas>;
type SceneLoad = ReturnType<typeof useSceneLoadState>;

interface WorldBoardOverlaysProps {
  engineReady: EngineCanvas["engineReady"];
  engineError: EngineCanvas["engineError"];
  loadProgress: EngineCanvas["loadProgress"];
  retryEngine: EngineCanvas["retryEngine"];
  engineStopped: EngineCanvas["engineStopped"];
  sceneId: string | null;
  sceneLoadState: SceneLoad["sceneLoadState"];
  pendingSceneResources: SceneLoad["pendingSceneResources"];
  retrySceneLoad: SceneLoad["retrySceneLoad"];
  liveSyncState: ReturnType<typeof useConnectivity>;
  reconcileReport: ReconcileReportState | null;
  setReconcileReport: Dispatch<SetStateAction<ReconcileReportState | null>>;
}

/**
 * What the board shows over itself while it is not simply playing: the engine
 * and scene loaders and their failures, the connection surfaces, and the
 * reconcile report.
 *
 * Moved out of `WorldPage` unchanged. A fragment, so these stay direct
 * children of the canvas container, in the same order.
 */
export function WorldBoardOverlays({
  engineReady,
  engineError,
  loadProgress,
  retryEngine,
  engineStopped,
  sceneId,
  sceneLoadState,
  pendingSceneResources,
  retrySceneLoad,
  liveSyncState,
  reconcileReport,
  setReconcileReport,
}: WorldBoardOverlaysProps) {
  return (
    <>
      {/* Spec 008 (US1, FR-002/SC-002): the flat #2a2a2a background
       * above used to show nothing at all here while the ~190MB
       * WASM engine loaded — this closes that gap with continuous
       * status text, styled identically to the scene-load-indicator
       * block below it. */}
      {/* Spec 028 US6: the same indicator, now carrying real byte
        progress and a distinct "starting" phase. */}
      {!engineReady && !engineError ? (
        <div
          data-testid="engine-load-indicator"
          style={{
            position: "absolute",
            top: "50%",
            left: "50%",
            transform: "translate(-50%, -50%)",
            zIndex: 1000,
          }}
        >
          {/* `step="engine"`: by the time this page is mounted the
              world has been fetched and the route's chunk has landed,
              so the first of the four waits is genuinely done. The
              suspense fallback that preceded it said so too, which is
              what makes the hand-off invisible. */}
          <EngineLoader
            progress={loadProgress}
            error={null}
            step="engine"
            // On its own ground rather than over the raw canvas: the
            // play field is a flat dark plate the theme knows nothing
            // about, and text laid straight onto it was white by hand
            // and unreadable the moment a light map loaded behind it.
            className={LOADING_PLATE}
          />
        </div>
      ) : null}
      {/* Deliberately outside `engine-load-indicator`: spec 008
        established that testid as meaning loading-in-progress, and a
        failure is not progress. Nesting the error inside it would
        make "still working" and "gave up" indistinguishable to
        anything selecting on it. */}
      {engineError && (
        <div
          style={{
            position: "absolute",
            top: "50%",
            left: "50%",
            transform: "translate(-50%, -50%)",
            zIndex: 1000,
          }}
        >
          <EngineLoader
            progress={null}
            error={engineError}
            className={LOADING_PLATE}
            onRetry={retryEngine}
          />
        </div>
      )}
      {/* Spec 070: a board that was running and is not any more. The
        canvas keeps its last frame, so without this a dead board
        looks like a live one that ignores you. */}
      {engineStopped && (
        <div
          style={{
            position: "absolute",
            top: "50%",
            left: "50%",
            transform: "translate(-50%, -50%)",
            zIndex: 1000,
          }}
        >
          <EngineStopped reason={engineStopped} className={LOADING_PLATE} />
        </div>
      )}
      {/*
        The scene loader waits for the engine loader.
                
        Both are true at once during a cold load — the engine is still
        arriving *and* the scene's content is being fetched — and both
        render dead centre, on top of each other. That is the two
        loaders a playtest reported, and it is a different pair from
        the route-fallback overlap fixed alongside it.
                
        The engine one wins because it is the longer wait and the one
        carrying real progress; the scene loader has nothing to add
        while the thing that draws scenes does not yet exist. Spec 031
        FR-041: at most one loading indicator visible at any moment.
      */}
      {engineReady && sceneId && sceneLoadState.status === "loading" ? (
        <div
          data-testid="scene-load-indicator"
          // Announced by `BoardLoading` itself, which is a status.
          style={{
            position: "absolute",
            top: "50%",
            left: "50%",
            transform: "translate(-50%, -50%)",
            textAlign: "center",
            zIndex: 1000,
          }}
        >
          {/*
            The map image is the specific complaint: it is the largest
            thing a scene waits on and, until this, the only one with
            no signal at all. So it is its own named step, and it is
            the step shown whenever it is what is left — everything
            else in a scene is small and arrives together.
          */}
          <BoardLoading
            current={
              pendingSceneResources.length === 1 &&
              pendingSceneResources[0] === "background"
                ? "map"
                : "scene"
            }
            detail={describeSceneLoad(pendingSceneResources)}
            className={LOADING_PLATE}
          />
        </div>
      ) : null}
      {sceneId && sceneLoadState.status === "error" ? (
        <div
          data-testid="scene-load-error"
          style={{
            position: "absolute",
            top: "50%",
            left: "50%",
            transform: "translate(-50%, -50%)",
            color: "white",
            textAlign: "center",
            zIndex: 1000,
          }}
        >
          <p>
            Failed to load{" "}
            {sceneLoadState.failedResource === "background"
              ? "the scene's background image"
              : `the scene's ${sceneLoadState.failedResource}`}
            .
          </p>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            data-testid="scene-load-retry"
            onClick={retrySceneLoad}
          >
            Retry
          </Button>
        </div>
      ) : null}
      {/*
        Spec 005 (FR-009/FR-009a) and spec 028 US7 (T097, FR-063,
        SC-022): one connection surface, never two. The badge used to
        be written inline here and has grown a third state, a second
        line and a colour of its own — and the moment a second
        indicator appears anywhere on this canvas, the two disagree
        and the user believes whichever they happen to look at.
      */}
      <ConnectionStatus state={liveSyncState} />
      {/*
        Spec 028 FR-049 (T092). The disclosure that peer transfer is
        in use has to reach someone who is playing, not someone who
        went to a settings page — so it renders here, and only while
        peers are actually connected. It manages its own visibility.
      */}
      <PeerIndicator />
      {/*
        The canvas readout: frames, latency, peers. Off by default
        and remembered once turned on, from the dock's Settings
        panel — diagnostics nobody asked for do not belong over the
        map.
      */}
      <EngineMonitor />
      {reconcileReport ? (
        <div
          style={{
            position: "absolute",
            top: "4rem",
            right: "4rem",
            zIndex: 1000,
            maxWidth: "22rem",
          }}
        >
          <ReconcileReport
            applied={reconcileReport.applied}
            rejected={reconcileReport.rejected}
            unanswered={reconcileReport.unanswered}
            // Supersession that happens *later* arrives as a world
            // event, not in this report; the list here is what the
            // reconcile call itself refused as superseded.
            superseded={reconcileReport.superseded}
            onBehalf={reconcileReport.onBehalf}
            onDismiss={() => setReconcileReport(null)}
          />
        </div>
      ) : null}
    </>
  );
}
