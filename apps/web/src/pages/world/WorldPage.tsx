import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
  claimPlayField,
  subscribeToPlayFieldState,
  type PlayFieldState,
} from "../../services/playFieldClaim";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { WorldLayout } from "@/layouts/world-layout/WorldLayout";
import type { SeoConfig } from "@/types/seo";
import { createWorldStore } from "@/engine/world/store";
import {
  applyInteractiveWorldEvent,
  applyLightWorldEvent,
  refreshInteractives,
  setScenePlaying,
  startTriggerBridge,
  applyShapeWorldEvent,
  applyTokenWorldEvent,
  applyTokenVisionWorldEvent,
  applyTokenGridWorldEvent,
  applyWallWorldEvent,
  getLiveSyncState,
  loadLightsIntoStore,
  loadShapesIntoStore,
  loadTokensIntoStore,
  loadWallsIntoStore,
  parseSceneLaunchedEvent,
  startLightMutationBridge,
  startShapeMutationBridge,
  startTokenMutationBridge,
  startWallMutationBridge,
  catchUpWorldEvents,
  startRollSync,
  subscribeToLiveSyncState,
  subscribeToWorldEvents,
} from "@/engine/world/sync";
import { parseMapImportedEvent } from "@/engine/world/sync/scenes";
import {
  queueAdjudicatedChange,
  reconcileWorld,
} from "@/engine/world/sync/offlineQueue";
import {
  startHeartbeat,
  stopHeartbeat,
  subscribeToHeartbeat,
} from "@/engine/world/sync/heartbeat";
import { onPlayPaused, reportPlayPaused } from "@/api/playPauseSignal";
import {
  parseReconciledEvent,
  pruneApplied,
  supersededBy,
  type AppliedChange,
  type ReconcileOutcome,
  type SubmittedChange,
} from "@/engine/world/sync/reconcile";
import { ReconcileReport } from "@/components/world/ReconcileReport";
import { ConnectionStatus } from "@/components/world/ConnectionStatus";
import { reportSessionPeers } from "@/engine/world/sync/subscriptionClient";
import {
  StatusPanel,
  type PanelPosition,
} from "@/components/StatusPanel/StatusPanel";
import {
  readTokenStatus,
  type TokenStatusResource,
} from "@/engine/bevy/tokenStatus";
import {
  applyTokenStatusWorldEvent,
  refreshTokenStatus,
} from "@/engine/world/sync/tokenStatus";
import {
  applySceneLightingWorldEvent,
  asAmbientLevel,
  setSceneAmbient,
  type AmbientLevel,
} from "@/engine/world/sync/sceneLighting";
import {
  beginPeerAdjudication,
  endPeerAdjudication,
  onOpenLore,
  peerAdjudicationActive,
  stopPeerTransfer,
  triggerDiceRollAnimation,
  setReducedMotion,
} from "@/engine/bevy";
import { watchReducedMotion } from "@/engine/bevy/diceThrow";
import { railTools } from "@/lib/authoringTools";
import { usePlaceActorTokens } from "./usePlaceActorTokens";
import { useGmToolRail } from "./useGmToolRail";
import { useSceneLoadState } from "./useSceneLoadState";
import { useSceneRecord } from "./useSceneRecord";
import { useEngineCanvas } from "./useEngineCanvas";
import { useBoardInput } from "./useBoardInput";
import { useEngineBridge } from "./useEngineBridge";
import { useExplorationPersistence } from "./useExploration";
import { useSceneTransition } from "./useSceneTransition";
import {
  getPeerTransferState,
  subscribeToPeerTransfer,
} from "@/services/peerTransfer";
import { PeerIndicator } from "@/components/diagnostics/PeerIndicator";
import { EngineMonitor } from "@/components/diagnostics/EngineMonitor";

/**
 * What the reconcile panel is currently showing.
 *
 * `superseded` is not part of a reconcile *response* — it accumulates
 * afterwards, from world events, as other clients reconnect and override what
 * this one already applied. Keeping it in the same value is what lets one
 * panel tell the whole story of an offline session rather than two competing
 * notices appearing minutes apart.
 */
interface ReconcileReportState {
  applied: SubmittedChange[];
  rejected: { change: SubmittedChange; outcome: ReconcileOutcome }[];
  unanswered: SubmittedChange[];
  stillQueued: SubmittedChange[];
  superseded: { change: SubmittedChange; byRole: string }[];
  /** Peer-adjudicated refusals submitted for another player (spec 028 T103). */
  onBehalf?: { change: SubmittedChange; outcome: ReconcileOutcome }[];
}
import { EngineLoader } from "@/components/engine/EngineLoader";
import { EngineStopped } from "@/components/engine/EngineStopped";
import { BoardLoading } from "@/components/engine/BoardLoading";
import { getScene, getScenes, updateSceneAmbientLight } from "@/api/scenes";
import { useAuth } from "@/hooks/useAuth";
import { useWorldRole } from "@/hooks/useWorldRole";
import { useWorldMembers } from "@/hooks/useWorldMembers";
import { useSceneUnits } from "@/hooks/useSceneUnits";
import { useSceneLevels } from "@/pages/world/useSceneLevels";
import { LevelTabs } from "@/components/world/LevelTabs";
import type { SceneLevel } from "@/api/levels";
import {
  AssetPasteTool,
  InteractionTool,
  LightingTool,
  ShapeTool,
  TokenTool,
  WallTool,
} from "@/pages/world/gmTools";
import { TokenPanel } from "@/components/TokenPanel";
import { DiceRollerPanel } from "@/components/world/DiceRollerPanel/DiceRollerPanel";
import { TokenStackPicker } from "@/components/canvas-tools/TokenStackPicker";
import { SelectionCount } from "@/components/world/SelectionCount";
import { SelectionBar } from "@/components/canvas-tools/SelectionBar";
import { CanvasContextMenu } from "@/components/world/CanvasContextMenu/CanvasContextMenu";
import { useCanvasContextMenu } from "@/components/world/CanvasContextMenu/useCanvasContextMenu";
import { ApprovalQueue } from "@/components/ApprovalQueue";
import { GmToolRail } from "@/components/world/GmToolRail/GmToolRail";
import { SelectionFilterMenu } from "@/components/world/GmToolRail/SelectionFilterMenu";
import { PlacedItemPrompt } from "@/components/world/PlacedItemPrompt";
import { toast } from "sonner";
import { getMyActorClaim } from "@/api/actorClaims";
import {
  WorldDock,
  type DockSection,
} from "@/components/world/PlayDock/WorldDock";
import { ChatPanel } from "@/components/world/PlayDock/ChatPanel";
import { ActorsPanel } from "@/components/world/PlayDock/ActorsPanel";
import { CombatPanel } from "@/components/world/PlayDock/CombatPanel";
import { AttackLog } from "@/components/world/PlayDock/AttackLog/AttackLog";
import { MapCreditLine } from "@/components/world/MapCredit";
import { OfferPrompt } from "@/components/world/PlayDock/OfferPrompt/OfferPrompt";
import { SystemDockPanel } from "@/components/world/PlayDock/SystemDockPanel";
import { useSystemDockTitle } from "@/panels/systemPanels";
import { SettingsPanel } from "@/components/world/PlayDock/SettingsPanel";
import { HelpPanel } from "@/components/world/PlayDock/HelpPanel";
import type { CanvasImageAsset } from "@/api/assets";
import { runsTheWorld, type WorldRecord } from "@/types/world";
import type { SceneRecord } from "@/types/scene";
import { appHref } from "@/lib/appHref";
import {
  describeSceneLoad,
  LOADING_PLATE,
  PANEL_PIN_KEY,
  readSavedPin,
} from "./worldPageHelpers";

export const worldPageSeo: SeoConfig = {
  title: "World workspace",
  description:
    "Launch the ThunderForge VTT collaborative world canvas powered by Bevy and synchronized world state.",
  canonicalPath: "/world/play",
  noindex: true,
};

export default function WorldPage() {
  const { id = "" } = useParams();
  const navigate = useNavigate();
  const canvasContainerId = "game-canvas-container";
  // Scene-scoped tokens are loaded from the server (tokens(sceneId)) once
  // a scene is selected — see the loadTokensIntoStore/startTokenMutationBridge
  // effect below — rather than seeded here with fixture data. The engine's
  // red/blue demo tokens are the engine sandbox's alone (`spawn_demo_tokens`);
  // a world session never asks for them, so every token here has a row.
  const [worldStore] = useState(() =>
    createWorldStore({
      worldId: id,
    }),
  );

  // Spec 036 US3a: this window is the account's play field for as long as it
  // is mounted. Subscribing is claiming — there is no mutation — so the claim
  // lives exactly as long as this effect, and a window that is displaced by
  // another of the same account finds out on the same stream.
  const [playField, setPlayField] = useState<PlayFieldState>({
    status: "unknown",
    holderClientId: null,
    error: null,
  });
  useEffect(() => {
    if (!id) {
      return;
    }
    const unwatch = subscribeToPlayFieldState(setPlayField);
    const release = claimPlayField(id);
    return () => {
      release();
      unwatch();
    };
  }, [id]);

  // Spec 029: the selected token's resources, read back from the engine rather
  // than held here. React observes; it does not become a second store
  // (Constitution I, ADR-053).
  // Keyed by the token it was read for, so a slow read that resolves after
  // the selection has moved cannot paint one token's resources under
  // another's name — and so deselection needs no synchronous state write.
  const [panelStatus, setPanelStatus] = useState<{
    tokenId: string;
    resources: TokenStatusResource[] | null;
  } | null>(null);
  // Spec 029 FR-011a (playtest 2026-09-10 P6): the pinned panel — which
  // token, and where it sits. Selection does not open it; a double-click does.
  const [pinned, setPinned] = useState<{
    tokenId: string;
    at: PanelPosition;
  } | null>(null);
  // Where the viewer last left one. A ref as well as storage, so the
  // double-click handler reads the latest without re-subscribing.
  const savedPinRef = useRef<PanelPosition | null>(readSavedPin());
  const { user } = useAuth();
  /**
   * What this client applied at its own reconnect, still eligible to be
   * superseded by a later one (spec 028 FR-041).
   *
   * A ref, not state: it is read inside a long-lived event loop that must not
   * be torn down and rebuilt every time a change is applied — re-subscribing
   * on each edit would drop events in the gap, which is the exact failure the
   * catch-up exists to fix. Declared up here because that loop is defined
   * above where these would otherwise sit.
   */
  const appliedRef = useRef<AppliedChange[]>([]);
  /**
   * The reconcile panel's contents. Declared alongside `appliedRef` because
   * the world-event loop above updates both, and that loop is defined before
   * this point in the component.
   */
  const [reconcileReport, setReconcileReport] =
    useState<ReconcileReportState | null>(null);
  /**
   * This client's own user id, for telling its replays apart from everyone
   * else's.
   *
   * Kept in sync from an effect rather than assigned during render: the loop
   * that reads it outlives any single render, and closing over the value
   * would pin it to whatever it was when the loop started — which, on a page
   * that mounts before the session resolves, is `null` forever.
   */
  const userIdRef = useRef<string | null>(null);
  useEffect(() => {
    userIdRef.current = user?.id ?? null;
  }, [user?.id]);
  const [world, setWorld] = useState<WorldRecord | null>(null);
  const [scenes, setScenes] = useState<SceneRecord[]>([]);
  // Which world the scene list above has actually finished fetching for.
  // `scenes` alone can't distinguish "still in flight" from "fetched and
  // genuinely empty" — both look like `[]` — and the scene-load state
  // below has to tell those apart.
  const [scenesFetchedForWorld, setScenesFetchedForWorld] = useState<
    string | null
  >(null);
  const [selectedSceneId, setSelectedSceneId] = useState<string | null>(null);
  // T006/T009: TokenPanel is mounted for both GM and player (a player needs
  // it to edit their own primary token's photo — FR-009a) via a toggle
  // button always visible once a scene is selected; the panel's own
  // internal isSceneOwner check (see TokenPanel.tsx) restricts create/
  // delete/ownership-assignment controls to the GM.
  const [tokenPanelOpen, setTokenPanelOpen] = useState(false);

  // GM/scene-owner check. Spec 009 (T007): this used to be a bare
  // `world.createdBy === user.id` comparison, which only recognized the
  // world's original creator — an invited co-GM (a `world_members` row
  // with role "GM") was silently treated as a non-owner. `useWorldRole`
  // fixes that by also matching an accepted GM-role membership, falling
  // back to the same createdBy check when no membership row exists yet
  // (research.md §3). Wall authoring tools (FR-009) are hidden entirely
  // for non-GMs, never merely disabled.
  const { isGm: isSceneOwner } = useWorldRole(id, world);
  // Spec 010: staging now happens earlier, at its own `/world/:id/staging`
  // route (`WorldStagingRoutePage.tsx`) — landing here means staging is
  // already done, so this page always renders the full-screen canvas
  // directly. `playView` itself is kept (rather than removed outright) so
  // the canvas-visibility effect below — which spec 009's research.md §1
  // found load-bearing for not invalidating the already-booted Bevy
  // engine's canvas handle — continues to work unmodified; it now simply
  // never leaves `"playing"`.
  const [playView] = useState<"staging" | "playing">("playing");

  const {
    setOpenGmToolId,
    allowedTools,
    revokedTool,
    setRevokedTool,
    effectiveGmToolId,
    hasToolRail,
  } = useGmToolRail({ id, isSceneOwner });

  // Playtest 2026-09-10 P1: the actors pane's Place creates a token where it
  // is dropped. `selectedSceneId`, not `sceneId`: the alias is declared
  // further down.
  usePlaceActorTokens(worldStore, selectedSceneId, isSceneOwner);

  /**
   * Bumped whenever something might have changed the approval queue, so it
   * re-reads. A counter rather than the queue itself: the server is the one
   * that knows what is pending, and holding a copy here would be a second
   * answer to that question.
   */
  const [approvalRevision, setApprovalRevision] = useState(0);
  const sceneId = selectedSceneId;

  const {
    sceneLoadGeneration,
    sceneLoadGenerationRef,
    sceneLoadState,
    pendingSceneResources,
    markSceneResourceLoaded,
    markSceneResourceFailed,
    retrySceneLoad,
  } = useSceneLoadState({ sceneId });

  // Scene levels: which floor of the scene is on the board.
  //
  // The engine is shown one level at a time and knows nothing of the others,
  // so changing level is a reload of the board — the same despawn, grant and
  // re-fetch a retry of the scene performs, which is why the switch is handed
  // the retry. By the time it runs the hook has already told the api layer
  // which level is wanted (`api/viewedLevel.ts`), so every loader below asks
  // for the new floor without being told.
  const {
    levels: sceneLevelList,
    level: sceneLevel,
    settled: sceneLevelSettled,
    onWorldEvent: onLevelWorldEvent,
    actions: levelActions,
  } = useSceneLevels({
    worldId: id,
    sceneId,
    userId: user?.id ?? null,
    isGm: isSceneOwner,
    onSwitch: retrySceneLoad,
  });
  // For the world-event loop, which lives as long as the scene does and must
  // not be torn down and reopened every time a token count changes.
  const sceneLevelRef = useRef<SceneLevel | null>(null);
  const sceneLevelListRef = useRef<readonly SceneLevel[]>(sceneLevelList);
  useEffect(() => {
    sceneLevelRef.current = sceneLevel;
    sceneLevelListRef.current = sceneLevelList;
  }, [sceneLevel, sceneLevelList]);
  const { selectedScene, sceneRecordSettledFor, setActiveSceneRecord } =
    useSceneRecord({
      id,
      sceneId,
      scenes,
      scenesFetchedForWorld,
      setWorld,
      setSelectedSceneId,
      setScenes,
      setScenesFetchedForWorld,
    });
  // Playtest 2026-09-10 P9: a scene's light as last heard — from a change
  // event or this Game Master's own control — ahead of the scene list, which
  // is fetched once and would otherwise go on holding the old level.
  //
  // Scene levels: the light is the *level's* now, and the level list is
  // re-read whenever a level changes — so what was last heard is trusted only
  // until the list it was heard against is replaced. `against` is that list;
  // a newer one carries the server's own answer and outranks it. Without that
  // an old "last heard" would go on masking a light another Game Master has
  // since changed, and would follow this one from floor to floor.
  const [ambientBySceneId, setAmbientBySceneId] = useState<
    Record<string, { level: AmbientLevel; against: readonly SceneLevel[] }>
  >({});
  const heardAmbient = sceneId ? ambientBySceneId[sceneId] : undefined;
  const sceneAmbient: AmbientLevel =
    (heardAmbient?.against === sceneLevelList
      ? heardAmbient.level
      : undefined) ??
    asAmbientLevel(sceneLevel?.ambientLight ?? selectedScene?.ambientLight);

  const {
    containerRef,
    engineReady,
    loadProgress,
    engineError,
    retryEngine,
    engineStopped,
  } = useEngineCanvas({ id, canvasContainerId, playView });

  const {
    openDockSectionId,
    setOpenDockSectionId,
    stackPicker,
    setStackPicker,
    facets,
  } = useBoardInput({
    id,
    sceneId,
    user,
    isSceneOwner,
    worldStore,
    playView,
    hasToolRail,
    effectiveGmToolId,
    setOpenGmToolId,
    setPinned,
    savedPinRef,
  });

  const { worldState, bridgeReady, allTokens } = useEngineBridge({
    id,
    user,
    isSceneOwner,
    worldStore,
    engineReady,
  });

  // Spec 045 US7: what this player has explored, kept in their own browser.
  //
  // Loaded on scene entry, saved on a timer, and re-read when a Game Master
  // resets the fog. The timer rather than a save per change: the engine adds
  // cells as a token walks, and writing storage at that rate would be a write
  // per step. A session that ends unsaved loses a few seconds of walking,
  // which a player recovers by standing still.
  const explorationEpoch = useRef(0);
  useExplorationPersistence({
    id,
    sceneId,
    userId: user?.id,
    engineReady,
    sceneLevel,
    sceneLevelSettled,
    explorationEpoch,
  });

  // Spec 045 FR-013/FR-015: when the engine stops a move at a wall, say so.
  //
  // The engine stops it locally and sends nothing, so no server refusal
  // arrives to explain it — without this the token simply does not move, which
  // from the player's chair is indistinguishable from a key that did not
  // register. The same sentence the server uses, and for the same reason it
  // says no more than this: a closed secret door stops a player like any wall.
  useEffect(() => {
    let unsubscribed = false;
    let stop: (() => void) | undefined;
    void import("@/engine/bevy").then(({ onMovementBlocked }) => {
      if (unsubscribed) {
        return;
      }
      stop = onMovementBlocked(() => {
        toast.warning("A wall is in the way");
      });
    });
    return () => {
      unsubscribed = true;
      stop?.();
    };
  }, []);

  useEffect(() => {
    if (!id || !bridgeReady) {
      return;
    }

    worldStore.dispatch({ type: "set_world", worldId: id }, "ui");

    void import("@/engine/bevy").then(({ setActiveWorld }) =>
      setActiveWorld(id),
    );
  }, [id, worldStore, bridgeReady]);

  const sceneLoadGranted = useSceneTransition({
    sceneId,
    sceneLoadGeneration,
    sceneLoadGenerationRef,
    sceneLoadState,
    bridgeReady,
    engineReady,
    sceneLevelSettled,
  });

  // The level's board, as plain values rather than the level itself: the level
  // list is re-read on every token that arrives or leaves (a Game Master's
  // tabs count them), and a new object for an unchanged floor must not send
  // the engine its map again.
  const hasLevelBoard = sceneLevel !== null;
  const levelBackgroundUrl = sceneLevel?.backgroundUrl ?? null;
  const levelWidth = sceneLevel?.width;
  const levelHeight = sceneLevel?.height;

  useEffect(() => {
    // Gated on the engine's grant (see `sceneLoadGrant`): the background and
    // the grid are scene content like everything else, and re-pointing the
    // sprite before the despawn frame would show the new map under the old
    // scene's walls.
    if (!bridgeReady || !sceneLoadGranted) {
      return;
    }

    // No scene record at all, after we have finished looking for one.
    //
    // This used to be every player: `scenes(worldId:)` filters hidden
    // scenes out for non-GMs (spec 022 FR-008) while `selectedSceneId`
    // comes from the world's unfiltered `activeSceneId`, so `selectedScene`
    // stayed null for the whole session, the dispatch below never ran, and
    // "background" was never marked — the pending set never emptied and
    // players sat under a permanent "Loading scene…" over a canvas that had
    // in fact fully loaded. That is fixed at the source now: a member may
    // read the scene their world is playing, and `activeSceneRecord` above
    // fetches it. What remains is the genuine case — there really is no
    // record to be had — where there is no `backgroundUrl` for anything to
    // load, so the resource is satisfied rather than left pending, the same
    // call the no-art branch below makes.
    //
    // Gated on having *finished looking*, not on the world's scene list
    // arriving: resolving while the by-id fetch is still in flight would
    // clear the overlay before the answer was known.
    if (!selectedScene) {
      if (sceneId && sceneRecordSettledFor === sceneId) {
        markSceneResourceLoaded("background", sceneLoadGeneration);
      }
      return;
    }

    const boardBackgroundUrl = hasLevelBoard
      ? levelBackgroundUrl
      : selectedScene.backgroundUrl;
    const boardWidth = levelWidth ?? selectedScene.width;
    const boardHeight = levelHeight ?? selectedScene.height;

    // Switching scenes re-points the engine's background sprite at the
    // newly-selected scene's imported map art (null clears it for a scene
    // with no import). worldStore.dispatch's generic bindWorldStore
    // forwarder relays this to the engine the same way every other
    // WorldCommand is relayed — no direct engine-bridge call needed.
    // Gated on `bridgeReady` (see its declaration) so this dispatch isn't
    // lost to the bindWorldStore-registration race.
    // Bug fix: this used to read `backgroundImagePath` directly, which
    // dd2vtt/map import (spec 022/002) never sets — it writes the
    // background to RustFS via `background_asset_id` instead, so an
    // imported map's art silently never loaded into the canvas.
    // `backgroundUrl` (GraphQLScene, graphql.rs) is the fetchable URL for
    // whichever mechanism actually populated the scene, computed
    // server-side — use that here instead.
    //
    // Scene levels: the art and its extent are the *level's*. The scene's own
    // are its entry level's, and a player standing on any other floor is
    // refused them outright — so the scene's are used only when no level
    // could be read at all.
    worldStore.dispatch(
      {
        type: "set_scene_background",
        backgroundImagePath: boardBackgroundUrl,
        width: boardWidth,
        height: boardHeight,
        worldId: id,
      },
      "ui",
    );

    // Sent with the background, never separately: the grid the engine draws
    // and snaps to has to be the one this scene's art was authored against.
    // Before this the engine had no way to learn a scene's grid at all — it
    // drew a hardcoded lattice and snapped to a different hardcoded one, so
    // an imported dd2vtt's `pixels_per_grid` (commonly 128) was ignored on
    // both counts.
    worldStore.dispatch(
      {
        type: "set_scene_grid",
        gridType: selectedScene.gridType,
        size: selectedScene.gridSize,
        // The scene's extent, so the engine anchors the lattice to the map's
        // corner rather than the world origin. The background sprite is
        // centred on the origin, so an origin-anchored grid only lines up with
        // the art when the map is an even number of cells across — a coin
        // flip, and wrong for half this project's own example maps.
        mapWidth: boardWidth,
        mapHeight: boardHeight,
        visible: true,
      },
      "ui",
    );

    // Spec 004 (US4): a scene with no imported art has nothing to fail —
    // mark "background" satisfied immediately. A scene with art gets a
    // real reachability check (the engine loads the sprite itself with no
    // JS-visible success/failure signal, so this is the only way to
    // surface "the background asset is unreachable" per FR-013 rather
    // than leaving the canvas silently blank).
    const generation = sceneLoadGeneration;
    if (!boardBackgroundUrl) {
      markSceneResourceLoaded("background", generation);
      return;
    }

    let cancelled = false;
    void fetch(boardBackgroundUrl, {
      method: "HEAD",
      credentials: "same-origin",
    })
      .then((response) => {
        if (cancelled) {
          return;
        }
        if (response.ok) {
          markSceneResourceLoaded("background", generation);
        } else {
          markSceneResourceFailed("background", generation);
        }
      })
      .catch(() => {
        if (!cancelled) {
          markSceneResourceFailed("background", generation);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [
    selectedScene,
    sceneId,
    sceneRecordSettledFor,
    worldStore,
    id,
    bridgeReady,
    sceneLoadGranted,
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
    hasLevelBoard,
    levelBackgroundUrl,
    levelWidth,
    levelHeight,
  ]);

  useEffect(() => {
    // `sceneLoadGranted`: the engine has finished taking the previous scene
    // off the canvas and is waiting for this one. Loading before that races
    // its despawn — see `sceneLoadGrant` above.
    if (!sceneId || !bridgeReady || !sceneLoadGranted) {
      return;
    }

    // Kept, with a narrower job than it used to have.
    //
    // The canvas is no longer this loop's responsibility: spec 031's
    // transition machine despawns the previous scene's walls (with its
    // tokens, lights, shapes and selection) before the grant that let this
    // effect run at all, and Constitution Principle I says one owner — the
    // engine owns what is on the canvas. Removing this outright was the
    // considered alternative, and it is wrong: `worldStore` is chrome's own
    // copy, not a view of the engine's, and the engine cannot reach into it.
    // `loadWallsIntoStore` only ever upserts, so without this the previous
    // scene's rows accumulate in chrome's state for the life of the session —
    // invisible on the canvas now, but read by every panel that lists walls.
    //
    // `"sync"` source: a confirmed-state correction, not a delete intent —
    // startWallMutationBridge below ignores `source: "sync"` dispatches, so
    // this never calls deleteWall. It is relayed on into the engine as well,
    // where it lands on entities the despawn already took: a no-op, and
    // cheaper to leave alone than to build a second dispatch path that
    // skips the engine.
    for (const wallId of Object.keys(worldStore.getState().walls)) {
      worldStore.dispatch({ type: "remove_wall", wallId }, "sync");
    }

    // Gated on `bridgeReady` (see its declaration): without it, this can
    // fire and dispatch the confirmed walls before bindWorldStore has
    // registered its forwarding subscription, silently losing them from
    // the engine's WallSet even though they're genuinely persisted.
    const generation = sceneLoadGeneration;
    void loadWallsIntoStore(worldStore, sceneId)
      .then(() => markSceneResourceLoaded("walls", generation))
      .catch((error) => {
        console.error("Failed to load scene walls:", error);
        markSceneResourceFailed("walls", generation);
      });

    const stopBridge = startWallMutationBridge(worldStore, sceneId);

    return () => {
      stopBridge();
    };
  }, [
    sceneId,
    worldStore,
    bridgeReady,
    sceneLoadGranted,
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
  ]);

  useEffect(() => {
    // `sceneLoadGranted`: the engine has finished taking the previous scene
    // off the canvas and is waiting for this one. Loading before that races
    // its despawn — see `sceneLoadGrant` above.
    if (!sceneId || !bridgeReady || !sceneLoadGranted) {
      return;
    }

    // Chrome's own copy of the tokens the canvas no longer holds, cleared for
    // the same reason as the walls and shapes loops above. Spec 031's
    // transition despawns every token on the canvas, so without this the store
    // keeps rows for tokens that no longer exist there: a panel listing one, a
    // party roster placing one, and a click at its reported position landing on
    // empty canvas. (The engine's demo tokens, which once made this worst, are
    // spawned only in the engine sandbox now.)
    //
    // `loadTokensIntoStore` only upserts, so leaving the stale rows for it to
    // overwrite was never an option — nothing overwrites a row whose id the
    // new scene never mentions. `"sync"` source: a confirmed-state correction,
    // not a delete intent, and `startTokenMutationBridge` returns early on
    // `"sync"`, so this never calls `deleteToken`.
    for (const tokenId of Object.keys(worldStore.getState().tokens)) {
      worldStore.dispatch({ type: "remove_token", tokenId }, "sync");
    }

    const tokensGeneration = sceneLoadGeneration;
    void loadTokensIntoStore(worldStore, sceneId)
      .then(() => markSceneResourceLoaded("tokens", tokensGeneration))
      .catch((error) => {
        console.error("Failed to load scene tokens:", error);
        markSceneResourceFailed("tokens", tokensGeneration);
      });

    const stopBridge = startTokenMutationBridge(
      worldStore,
      sceneId,
      isSceneOwner,
      // Spec 028 US7: the bridge needs the world to queue an edit against
      // when there is nowhere to send it.
      id,
    );

    return () => {
      stopBridge();
    };
  }, [
    sceneId,
    worldStore,
    bridgeReady,
    sceneLoadGranted,
    isSceneOwner,
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
    // The world the token bridge queues offline edits against (spec 028 US7).
    id,
  ]);

  useEffect(() => {
    // `sceneLoadGranted`: the engine has finished taking the previous scene
    // off the canvas and is waiting for this one. Loading before that races
    // its despawn — see `sceneLoadGrant` above.
    if (!sceneId || !bridgeReady || !sceneLoadGranted) {
      return;
    }

    // Chrome's own copy of the previous board's lights, cleared for the same
    // reason as the walls, tokens and shapes loops. It was the one loader
    // without such a loop, and a level switch is what made that matter: the
    // floor left behind and the floor arrived at are one scene, so nothing
    // else would ever take the old floor's lights out of the Lights panel.
    // `"sync"` source, so the light bridge never calls `deleteLight`.
    for (const lightId of Object.keys(worldStore.getState().lights)) {
      worldStore.dispatch({ type: "remove_light", lightId }, "sync");
    }

    const lightsGeneration = sceneLoadGeneration;
    void loadLightsIntoStore(worldStore, sceneId)
      .then(() => markSceneResourceLoaded("lights", lightsGeneration))
      .catch((error) => {
        console.error("Failed to load scene lights:", error);
        markSceneResourceFailed("lights", lightsGeneration);
      });

    const stopBridge = startLightMutationBridge(worldStore, sceneId);

    return () => {
      stopBridge();
    };
  }, [
    sceneId,
    worldStore,
    bridgeReady,
    sceneLoadGranted,
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
  ]);

  useEffect(() => {
    // `sceneLoadGranted`: the engine has finished taking the previous scene
    // off the canvas and is waiting for this one. Loading before that races
    // its despawn — see `sceneLoadGrant` above.
    if (!sceneId || !bridgeReady || !sceneLoadGranted) {
      return;
    }

    // Chrome's own copy of the previous scene's shapes, cleared for the same
    // narrowed reason as the walls loop above: the engine has already
    // despawned them from the canvas, and this keeps `worldStore` from
    // accumulating rows the canvas no longer has. Spec 002 T017's original
    // symptom — a shape from scene A still hit-testable after switching to
    // scene B — is the engine's to prevent now, and it does.
    for (const shapeId of Object.keys(worldStore.getState().shapes)) {
      worldStore.dispatch({ type: "remove_shape", shapeId }, "sync");
    }

    const shapesGeneration = sceneLoadGeneration;
    void loadShapesIntoStore(worldStore, sceneId)
      .then(() => markSceneResourceLoaded("shapes", shapesGeneration))
      .catch((error) => {
        console.error("Failed to load scene shapes:", error);
        markSceneResourceFailed("shapes", shapesGeneration);
      });

    const stopBridge = startShapeMutationBridge(worldStore, sceneId);

    return () => {
      stopBridge();
    };
  }, [
    sceneId,
    worldStore,
    bridgeReady,
    sceneLoadGranted,
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
  ]);

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
  }, [id]);

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

  // Live cross-client sync: one subscription per mounted scene, feeding
  // every apply*WorldEvent for this scene's canvas primitives — each of
  // those already filters by its own event code internally (walls=10,
  // tokens=14, etc, see their own doc comments), so a single shared
  // subscription driving all four is correct and avoids opening four
  // separate WebSocket subscriptions for the same event stream. The
  // backend transport (Postgres listener -> broadcast channel -> this
  // `worldEventsCreated` GraphQL subscription -> /api/ws) already existed
  // in full; this is the first thing in apps/web to actually open it.
  // Read through a ref by the world-event loop below, which is declared
  // before the callback and must not reopen its stream when it changes.
  const reloadImportedSceneRef = useRef<(importedSceneId: string) => void>(
    () => {},
  );
  useEffect(() => {
    if (!id || !sceneId || !bridgeReady) {
      return;
    }

    // Prime before the stream opens: the first paint should already carry
    // bars rather than waiting for something to change.
    void refreshTokenStatus(worldStore, sceneId).catch((error) => {
      console.error("Failed to load token status:", error);
    });

    // Not an AbortController + in-loop flag check: a `for await` loop
    // only re-checks anything between events, so if the world goes quiet
    // (no new events) the loop hangs forever awaiting the next one and
    // the subscription would never actually close on unmount/scene
    // change. Holding the iterator directly and calling `.return()` on
    // it unblocks that pending `next()` immediately (it synchronously
    // disposes the underlying graphql-ws subscription, which resolves
    // the pending promise via the `complete` callback) — this is why
    // `for await...of` itself calls `.return()` on early exit, and why a
    // manual cleanup needs to do the same explicitly.
    const iterator = subscribeToWorldEvents(id)[Symbol.asyncIterator]();
    let cancelled = false;

    (async () => {
      try {
        while (!cancelled) {
          const { value: event, done } = await iterator.next();
          if (done || cancelled || !event) break;
          // Scene levels: a level changed, or a token changed floor. Told
          // first and not awaited — it re-reads which level this viewer is on
          // and reloads the board itself if the answer moved.
          onLevelWorldEvent(event);
          // A map was imported into this scene. Nothing else announces its
          // walls, lights or art, so they are re-read here.
          if (parseMapImportedEvent(event) === sceneId) {
            reloadImportedSceneRef.current(sceneId);
          }
          await Promise.all([
            applyWallWorldEvent(worldStore, sceneId, event),
            applyTokenWorldEvent(worldStore, sceneId, event),
            applyShapeWorldEvent(worldStore, sceneId, event),
            applyLightWorldEvent(worldStore, sceneId, event),
            applyTokenStatusWorldEvent(worldStore, sceneId, event),
            applyInteractiveWorldEvent(worldStore, sceneId, event),
            // Spec 045 FR-067: a character's sheet moving changes what their
            // token can see, and until spec 045 a sheet edit announced
            // nothing at all.
            applyTokenVisionWorldEvent(worldStore, sceneId, event, () =>
              Object.keys(worldStore.getState().tokens),
            ),
            // Spec 046 US4: a sheet's size is the squares its tokens fill.
            applyTokenGridWorldEvent(worldStore, sceneId, event, () =>
              Object.keys(worldStore.getState().tokens),
            ),
          ]);
          // Spec 045 US7: a Game Master reset the fog. Re-read rather than
          // trusting the payload — a reset aimed at one player and one aimed
          // at everyone are the same event on the bus, and only the server
          // can say which applies to whoever is holding this browser.
          if (user?.id) {
            const epoch = await import("@/engine/world/sync/exploration").then(
              ({ applyExplorationWorldEvent }) =>
                applyExplorationWorldEvent(
                  user.id,
                  id,
                  sceneId,
                  event,
                  sceneLevelRef.current
                    ? {
                        levelId: sceneLevelRef.current.levelId,
                        isEntry: sceneLevelRef.current.isEntry,
                      }
                    : undefined,
                ),
            );
            if (epoch !== null) {
              explorationEpoch.current = epoch;
            }
          }
          // Scene levels: the scene's own light is its entry level's. On
          // any other floor a change to it is news about a board this client
          // is not showing, and applying it would relight the wrong one.
          const shownLevel = sceneLevelRef.current;
          const ambient =
            shownLevel === null || shownLevel.isEntry
              ? applySceneLightingWorldEvent(worldStore, sceneId, event)
              : null;
          if (ambient) {
            const against = sceneLevelListRef.current;
            setAmbientBySceneId((current) => ({
              ...current,
              [sceneId]: { level: ambient, against },
            }));
          }
          // The approval queue is server state, so a nudge on the bus is all
          // this side needs — it re-reads rather than holding a copy.
          setApprovalRevision((n) => n + 1);
        }
      } catch (error) {
        console.error("World events live-sync error:", error);
      }
    })();

    return () => {
      cancelled = true;
      void iterator.return?.();
    };
    // `user?.id` because the exploration handler above resets *this
    // player's* fog: a subscription that outlived a change of user would
    // carry the previous one's id and reset the wrong person's map.
  }, [id, sceneId, bridgeReady, worldStore, user?.id, onLevelWorldEvent]);

  // Playtest 2026-09-10 P9: hand the engine the scene's light. Nothing did,
  // so every scene rendered in daylight and no wall ever cast a shadow.
  useEffect(() => {
    if (!sceneId || !bridgeReady) {
      return;
    }
    setSceneAmbient(worldStore, sceneAmbient);
  }, [sceneId, bridgeReady, sceneAmbient, worldStore]);

  const changeSceneAmbient = useCallback(
    (level: AmbientLevel) => {
      if (!sceneId) {
        return;
      }
      const previous = sceneAmbient;
      const against = sceneLevelList;
      setAmbientBySceneId((current) => ({
        ...current,
        [sceneId]: { level, against },
      }));
      if (sceneLevel) {
        // Scene levels: the control lights the floor on the board, not the
        // scene. No rollback to write — the level list is re-read whether the
        // server agreed or refused, and a re-read list outranks what was set
        // here a moment ago (see `ambientBySceneId`).
        void levelActions.setAmbient(sceneLevel.levelId, level);
        return;
      }
      updateSceneAmbientLight(sceneId, level).catch((error: unknown) => {
        console.error("Failed to change the scene's light:", error);
        setAmbientBySceneId((current) => ({
          ...current,
          [sceneId]: { level: previous, against },
        }));
      });
    },
    [sceneId, sceneAmbient, sceneLevel, sceneLevelList, levelActions],
  );

  // Spec 045 US7: the Game Master's two controls — whether this scene
  // remembers, and resetting what it has remembered.
  const [explorationOn, setExplorationOn] = useState(false);
  useEffect(() => {
    if (!sceneId || !isSceneOwner) {
      return;
    }
    let stale = false;
    void import("@/engine/world/sync/exploration")
      .then(({ getSceneExploration }) => getSceneExploration(sceneId))
      .then((scene) => {
        if (!stale) {
          setExplorationOn(scene.enabled);
        }
      })
      .catch(() => {
        // A control that cannot read its own state shows off, which is the
        // scene's default and the safe thing to show.
      });
    return () => {
      stale = true;
    };
  }, [sceneId, isSceneOwner]);

  // Who a Game Master may reset individually.
  //
  // Derived from the tokens that have owners, labelled by the token's own
  // name: it is the name on the board the Game Master is looking at, and it
  // is what they would say out loud. A world-membership list would be more
  // complete and less useful — it would offer players who have no token in
  // this scene and therefore no fog to reset.
  // Spec 045 FR-061: a light's reach is set in the system's units.
  const sceneUnits = useSceneUnits(sceneId, isSceneOwner);

  // The play field's right-click menu, and its keyboard equivalent: what this
  // viewer may do to the token under the pointer, or on bare board.
  const canvasMenu = useCanvasContextMenu(
    worldStore,
    Boolean(sceneId && user),
    isSceneOwner,
  );

  const explorationPlayers = useMemo(() => {
    const byOwner = new Map<string, string>();
    for (const token of Object.values(allTokens)) {
      if (token.ownerUserId && !byOwner.has(token.ownerUserId)) {
        byOwner.set(token.ownerUserId, token.label ?? "a player");
      }
    }
    return [...byOwner].map(([userId, name]) => ({ userId, name }));
  }, [allTokens]);

  const changeExploration = useCallback(
    (enabled: boolean) => {
      if (!sceneId) {
        return;
      }
      // Shown immediately and put back if the server refuses, like the
      // scene's light above: a toggle that waits for a round trip feels
      // broken even when it works.
      setExplorationOn(enabled);
      void import("@/api/exploration")
        .then(({ setSceneExploration }) =>
          setSceneExploration(sceneId, enabled),
        )
        .catch((error: unknown) => {
          console.error("Failed to change exploration:", error);
          setExplorationOn(!enabled);
        });
    },
    [sceneId],
  );

  const resetExploration = useCallback(
    (forUser: string | null) => {
      if (!sceneId) {
        return;
      }
      void import("@/api/exploration")
        .then(({ resetSceneExploration }) =>
          resetSceneExploration(sceneId, forUser),
        )
        .catch((error: unknown) => {
          console.error("Failed to reset exploration:", error);
        });
    },
    [sceneId],
  );

  /**
   * Spec 030: interactive elements.
   *
   * Three things, and they belong together because each is useless without the
   * others: load what this viewer may interact with, tell the engine the scene
   * is being *played* (region entry fires on nothing otherwise — FR-032), and
   * bridge engine-detected triggers back to the server, which decides.
   */
  useEffect(() => {
    // Scene levels: gated on the engine's grant like the other loaders, and
    // re-run by the load generation. Interactives are per level, and a level
    // switch changes neither `sceneId` nor `bridgeReady` — without the grant
    // in this list the new floor's stairs would never be read.
    if (!sceneId || !bridgeReady || !sceneLoadGranted) {
      return;
    }

    void refreshInteractives(worldStore, sceneId).catch((error: unknown) => {
      console.error("Failed to load interactive elements:", error);
    });

    // The play view is, by definition, play. Preparation happens on the
    // staging route, which does not mount this.
    setScenePlaying(worldStore, true);

    const stopTriggers = startTriggerBridge(worldStore, (result) => {
      if (result.outcome === "requested") {
        setApprovalRevision((n) => n + 1);
      }
    });

    return () => {
      stopTriggers();
      // A scene nobody is looking at is not being played, and leaving this on
      // would let a background tab's stale positions fire a region.
      setScenePlaying(worldStore, false);
    };
  }, [sceneId, bridgeReady, worldStore, sceneLoadGranted, sceneLoadGeneration]);

  /**
   * Which character the person at this keyboard is playing.
   *
   * Needed only to know where a picked-up item goes (spec 031 FR-015). `null`
   * is an ordinary answer, not a failure: a Game Master has no claim, and the
   * pickup prompt says so rather than offering an action that cannot work.
   */
  const [claimedActorId, setClaimedActorId] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    getMyActorClaim(id)
      .then((claim) => {
        if (active) {
          setClaimedActorId(claim?.actorId ?? null);
        }
      })
      .catch(() => {
        // A claim that cannot be read leaves Pick up unavailable and explained,
        // which is the same state as having no claim. Nothing on the map
        // depends on this, so there is nothing to report.
        if (active) {
          setClaimedActorId(null);
        }
      });
    return () => {
      active = false;
    };
  }, [id]);

  /**
   * Spec 030: an interactive asked for a lore page to be opened.
   *
   * The engine resolves *which* entry; opening a tab needs this application's
   * URL structure, which is chrome and belongs on this side (Principle I).
   */
  useEffect(
    () =>
      onOpenLore((event) => {
        window.open(
          appHref(`/world/${id}/lore/${event.entryId}`),
          "_blank",
          "noopener,noreferrer",
        );
      }),
    [id],
  );

  /** The pinned token's name, for the panel heading. */
  const pinnedTokenLabel = useMemo(() => {
    const id = pinned?.tokenId;
    if (!id) return undefined;
    return worldState.tokens[id]?.label ?? undefined;
  }, [pinned?.tokenId, worldState.tokens]);

  // Follow the pin, not the selection (spec 029 FR-012a): read what the
  // engine would draw for the pinned token, whatever is selected now.
  useEffect(() => {
    const tokenId = pinned?.tokenId;
    if (!tokenId) {
      return;
    }

    let cancelled = false;
    void readTokenStatus(tokenId).then((resources) => {
      if (!cancelled) setPanelStatus({ tokenId, resources });
    });
    return () => {
      cancelled = true;
    };
  }, [pinned?.tokenId, worldState.tokens]);

  // Derived rather than stored: the panel shows a token's resources only while
  // that token is the pinned one, so unpinning — or pinning another — can
  // never show the previous token's numbers.
  const panelResources =
    pinned && panelStatus?.tokenId === pinned.tokenId
      ? panelStatus.resources
      : null;

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
  }, [id]);

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
  }, [worldStore]);

  // A map import writes walls, doors, lights and the scene's art straight to
  // Postgres (the REST endpoint, not the GraphQL mutation bridge), and says so
  // with one world event (code 13) and nothing else. So every client — the
  // importer's and everyone else's — re-runs the same loaders used on mount to
  // pull the new content in without a reload. The art reaches the board two
  // ways, both covered: the scene's levels are re-read by `useSceneLevels`
  // (the import's art is mirrored onto the entry level, which is the board's
  // art), and the scene record is re-read here for a board with no levels.
  const reloadImportedScene = useCallback(
    (importedSceneId: string) => {
      if (!id) {
        return;
      }
      void loadWallsIntoStore(worldStore, importedSceneId).catch((error) => {
        console.error("Failed to reload scene walls after map import:", error);
      });
      void loadLightsIntoStore(worldStore, importedSceneId).catch((error) => {
        console.error("Failed to reload scene lights after map import:", error);
      });
      void loadShapesIntoStore(worldStore, importedSceneId).catch((error) => {
        console.error("Failed to reload scene shapes after map import:", error);
      });
      void getScenes(id)
        .then(setScenes)
        .catch((error) => {
          console.error("Failed to reload scenes after map import:", error);
        });
      // A player's list leaves out a hidden scene — and a world's own
      // Starting Scene is hidden — so the record they play from is the one
      // fetched by id, and that is the one that has to be fetched again.
      void getScene(importedSceneId)
        .then((scene) => {
          if (scene) {
            setActiveSceneRecord(scene);
          }
        })
        .catch((error) => {
          console.error("Failed to reload the scene after map import:", error);
        });
    },
    // `setActiveSceneRecord` is a state setter from `useSceneRecord`: stable.
    [id, worldStore, setActiveSceneRecord],
  );
  useEffect(() => {
    reloadImportedSceneRef.current = reloadImportedScene;
  }, [reloadImportedScene]);

  const handleMapImportComplete = useCallback(() => {
    if (!sceneId) {
      return;
    }
    // The importer's own tab does not wait for the event to come back round.
    reloadImportedScene(sceneId);
  }, [sceneId, reloadImportedScene]);

  // T023/T030 (specs/002-canvas-authoring-asset-storage): a pasted image
  // is already persisted by the time AssetPasteTool calls this (the
  // GraphQL mutation already succeeded) — this just tells the engine to
  // spawn it. `path` points at the authenticated `/canvas-assets/{id}`
  // proxy route (crates/thunderforge-server/src/canvas_assets_serve.rs), not RustFS
  // directly: RustFS is private per-campaign storage, so the browser can
  // never fetch from it directly — this proxy is what makes a pasted
  // image (or, latently, a migrated background) actually renderable at
  // all, a gap that existed silently until this was wired up (nothing
  // previously exercised the read path end-to-end).
  const handleAssetPasted = useCallback(
    (asset: CanvasImageAsset) => {
      worldStore.dispatch(
        {
          type: "upsert_canvas_image_asset",
          assetId: asset.id,
          // `.webp`, not a bare id: the engine resolves an image
          // loader by extension (see canvas_assets_serve::parse_asset_id).
          path: `/api/canvas-assets/${asset.id}.webp`,
          x: 0,
          y: 0,
          width: asset.widthPx,
          height: asset.heightPx,
        },
        "ui",
      );
    },
    [worldStore],
  );

  const seo = useMemo<SeoConfig>(
    () => ({
      ...worldPageSeo,
      title: `${id || "World"} workspace`,
      canonicalPath: `/world/${id}/play`,
    }),
    [id],
  );

  const systemDockLabel = useSystemDockTitle(world?.gameSystemId ?? null);
  const dockSections: DockSection[] = [
    {
      id: "chat",
      label: "Chat",
      icon: "quill",
      content: (
        <ChatPanel
          worldId={id}
          sceneId={sceneId}
          currentUserId={user?.id ?? null}
          isGm={isSceneOwner}
        />
      ),
    },
    {
      id: "actors",
      label: "Actors",
      icon: "actors",
      content: (
        <ActorsPanel
          worldId={id}
          worldStore={worldStore}
          sceneId={sceneId}
          isGm={isSceneOwner}
        />
      ),
    },
    {
      id: "combat",
      label: "Combat",
      icon: "shield",
      content: (
        <CombatPanel
          worldId={id}
          worldStore={worldStore}
          sceneId={sceneId}
          isGm={isSceneOwner}
        />
      ),
    },
    // Spec 067: the tab is the pack's, titled by the pack, and absent for a
    // system that puts nothing in the dock.
    ...(systemDockLabel
      ? [
          {
            id: "system" as const,
            label: systemDockLabel,
            icon: "moon" as const,
            content: (
              <SystemDockPanel
                worldId={id}
                isGm={isSceneOwner}
                currentUserId={user?.id}
                gameSystemId={world?.gameSystemId ?? null}
              />
            ),
          },
        ]
      : []),
    // Spec 030. Game-Master-only: a player is not shown the queue. Their own
    // outcome reaches them directly, and the rest of it is a list of what other
    // people asked for — which at some tables is information the GM is
    // deliberately not sharing yet.
    ...(isSceneOwner && sceneId
      ? [
          {
            id: "requests" as const,
            label: "Player requests",
            icon: "quill" as const,
            content: (
              <ApprovalQueue
                sceneId={sceneId}
                revision={approvalRevision}
                onDecided={() => setApprovalRevision((n) => n + 1)}
              />
            ),
          },
        ]
      : []),
    {
      id: "settings",
      label: "Settings",
      icon: "settings",
      content: (
        <SettingsPanel
          worldId={id}
          sceneId={sceneId}
          scenes={scenes}
          isGm={isSceneOwner}
          onSceneChange={setSelectedSceneId}
          onSceneCreated={(scene) =>
            setScenes((current) => [...current, scene])
          }
          onMapImportComplete={handleMapImportComplete}
          onBackToStaging={() => navigate(`/world/${id}/staging`)}
        />
      ),
    },
    // Playtest 2026-09-10 P4: About and Feedback, for every role. They used
    // to float over the map, under the tool rail and the dock.
    {
      id: "help",
      label: "About & feedback",
      icon: "rune",
      content: <HelpPanel />,
    },
  ];

  return (
    <>
      <SEO {...seo} />
      {/* Always present, so "has this window taken the table yet?" is
          observable rather than inferred from the absence of a banner —
          which is also true before anything has happened. */}
      <span
        hidden
        data-testid="play-field-status"
        data-status={playField.status}
      />
      {/* Two states, one notice — because "not at the table" is one situation
          for the person even though the registry distinguishes two.

          `unclaimed` was the gap, and e2e found it. A displaced window is
          told `companion` and offered the way back; when the window that took
          the table then closes, the release arrives on that same stream as
          "nobody holds it" and the status becomes `unclaimed`. Rendering only
          for `companion` made the notice — and the only route back to the
          table — disappear at exactly that moment, leaving a window that
          looks ordinary, applies nothing to the map, and says nothing about
          why. Silent limbo is the worst of the three states to be in and was
          the only one with no way out.

          Not re-claiming automatically is deliberate: the person moved to the
          other window on purpose, and that window closing is not them asking
          to be dragged back. It is an offer, not a restoration. */}
      {playField.status === "companion" || playField.status === "unclaimed" ? (
        <div
          role="status"
          data-testid="play-field-taken-over-notice"
          data-play-field-state={playField.status}
          className="fixed inset-x-0 top-0 z-50 bg-amber-900/90 px-4 py-2 text-center text-sm text-amber-50"
        >
          {playField.status === "companion"
            ? "Another window of your account has taken the table. This one is now a companion — it will not apply changes to the map."
            : "No window of your account is at the table, so this one will not apply changes to the map."}{" "}
          <button
            type="button"
            data-testid="take-play-field-back"
            className="underline underline-offset-2"
            onClick={() => claimPlayField(id)}
          >
            Take it back here
          </button>
        </div>
      ) : null}
      <CanvasContextMenu
        request={canvasMenu.request}
        onDone={canvasMenu.done}
        worldId={id}
        sceneId={sceneId}
        worldStore={worldStore}
        control={facets.tokens}
        isGameMaster={isSceneOwner}
        userId={user?.id ?? null}
        units={sceneUnits}
      />
      {stackPicker ? (
        <TokenStackPicker
          members={stackPicker.members}
          at={stackPicker.at}
          onPick={(tokenId) => {
            facets.selection.selectOne(tokenId);
            setStackPicker(null);
          }}
          onDismiss={() => setStackPicker(null)}
        />
      ) : null}
      {revokedTool ? (
        <div
          role="status"
          data-testid="authoring-tool-revoked-notice"
          className="fixed top-4 left-1/2 z-[1100] -translate-x-1/2 rounded-lg border border-border bg-background/95 px-4 py-2 text-sm shadow-xl backdrop-blur"
        >
          <span>
            The {revokedTool} tool is no longer available to you. Anything you
            were drawing with it was discarded.
          </span>
          <button
            type="button"
            className="ml-3 rounded-md px-1.5 text-muted-foreground hover:text-foreground"
            aria-label="Dismiss"
            onClick={() => setRevokedTool(null)}
          >
            ×
          </button>
        </div>
      ) : null}
      <div style={{ display: playView === "playing" ? "block" : "none" }}>
        <WorldLayout
          worldId={id}
          toolRail={
            hasToolRail && sceneId ? (
              <GmToolRail
                openToolId={effectiveGmToolId}
                onOpenToolChange={setOpenGmToolId}
                // Filtered, not merely disabled: FR-047 says a tool a person
                // may not use is not offered. A greyed-out button still tells
                // them the tool exists and invites them to ask why it does not
                // work.
                tools={railTools(
                  [
                    {
                      id: "select",
                      label: "Select",
                      icon: "select",
                      // A panel after all, but a collapsible one that remembers
                      // being collapsed. The original reasoning — that an
                      // always-open panel would cover 256px of map to say "you
                      // may now click things" — is answered by the collapse,
                      // not contradicted: a Game Master who does not want it
                      // never sees it again (spec 031 FR-010).
                      content: <SelectionFilterMenu />,
                    },
                    {
                      id: "walls",
                      label: "Walls",
                      icon: "shield",
                      content: (
                        <WallTool
                          worldStore={worldStore}
                          walls={worldState.walls}
                          selectedWallId={worldState.selectedWallId}
                        />
                      ),
                    },
                    {
                      id: "lights",
                      label: "Lights",
                      icon: "torch",
                      content: (
                        <LightingTool
                          worldStore={worldStore}
                          lights={worldState.lights}
                          selectedLightId={worldState.selectedLightId}
                          tokens={worldState.tokens}
                          ambientLight={sceneAmbient}
                          onAmbientLightChange={
                            sceneId ? changeSceneAmbient : undefined
                          }
                          explorationEnabled={explorationOn}
                          onExplorationChange={
                            sceneId ? changeExploration : undefined
                          }
                          onExplorationReset={
                            sceneId ? resetExploration : undefined
                          }
                          players={explorationPlayers}
                          units={sceneUnits}
                        />
                      ),
                    },
                    {
                      id: "shapes",
                      label: "Shapes",
                      icon: "tokens",
                      content: (
                        <ShapeTool
                          worldStore={worldStore}
                          shapes={worldState.shapes}
                          selectedShapeId={worldState.selectedShapeId}
                          sceneId={sceneId}
                          canvasContainerRef={containerRef}
                          isGm={isSceneOwner}
                          sceneName={selectedScene?.name}
                        />
                      ),
                    },
                    {
                      id: "tokens",
                      label: "Tokens",
                      icon: "actors",
                      content: (
                        <TokenTool
                          control={facets.tokens}
                          selectedTokenId={worldState.selectedTokenId}
                          worldId={id}
                          sceneId={sceneId}
                        />
                      ),
                    },
                    {
                      // Spec 030. Last in the rail because it is authored *onto*
                      // what the other four place: you draw a wall, then decide
                      // it is a door that opens.
                      id: "interactions",
                      label: "Interactions",
                      icon: "rune",
                      content: (
                        <InteractionTool
                          worldStore={worldStore}
                          worldId={id}
                          sceneId={sceneId}
                          selectedTokenId={worldState.selectedTokenId}
                          selectedWallId={worldState.selectedWallId}
                          walls={worldState.walls}
                          lights={worldState.lights}
                          levels={sceneLevelList}
                        />
                      ),
                    },
                  ],
                  allowedTools,
                  isSceneOwner,
                )}
              />
            ) : null
          }
          dock={
            <WorldDock
              sections={dockSections}
              openSectionId={openDockSectionId}
              onOpenSectionIdChange={setOpenDockSectionId}
            />
          }
          canvas={
            <div
              ref={containerRef}
              id={canvasContainerId}
              style={{
                display: "block",
                width: "100%",
                height: "100%",
                position: "relative",
                background: "#2a2a2a",
                overflow: "hidden",
              }}
            >
              {/* Spec 046: what the table sees of a fight — every seat's view
               * of the scene's attacks, and the offers waiting for this
               * viewer. Over the board rather than in the dock, which mounts
               * only its open section: a hit has to reach a player who is
               * looking at something else. Bottom centre: the combat playtest
               * found the dice roller's Roll button under it at the bottom
               * left, and the Game Master's tool flyout over it at the top
               * left. The open dock covers the right. */}
              {/* Scene levels: which floor this is. Top centre — the Game
               * Master's tool flyout has the top left, the open dock the
               * right, and the table feed the bottom. It renders nothing for
               * a player on a scene with one level, so a scene made before
               * levels existed looks exactly as it did. */}
              <div className="pointer-events-none absolute top-3 left-1/2 z-[1040] flex max-w-[50%] -translate-x-1/2 items-center justify-center gap-2">
                <LevelTabs
                  levels={sceneLevelList}
                  level={sceneLevel}
                  isGm={isSceneOwner}
                  selectedTokenIds={worldState.selectedTokenIds}
                  actions={levelActions}
                />
                {/* How many tokens a stack click picked up; nothing for one. */}
                <SelectionCount
                  selectedTokenIds={worldState.selectedTokenIds}
                />
                {/* Spec 085 FR-018: a group in Select. Someone without a rail
                 * is always in Select. */}
                {!hasToolRail ||
                (effectiveGmToolId ?? "select") === "select" ? (
                  <SelectionBar
                    worldStore={worldStore}
                    isGm={isSceneOwner}
                    walls={worldState.walls}
                    selectedTokenIds={worldState.selectedTokenIds}
                    selectedWallIds={worldState.selectedWallIds}
                    selectedLightIds={worldState.selectedLightIds}
                    selectedShapeIds={worldState.selectedShapeIds}
                  />
                ) : null}
              </div>
              {id ? (
                <div
                  className="pointer-events-none absolute bottom-3 left-1/2 z-[1040] flex w-80 max-w-[40%] -translate-x-1/2 flex-col-reverse gap-2"
                  data-testid="table-feed"
                >
                  {/* Spec 088 FR-030: a base map is credited while it is shown. */}
                  {selectedScene?.backgroundCredit ? (
                    <MapCreditLine credit={selectedScene.backgroundCredit} />
                  ) : null}
                  <OfferPrompt worldId={id} isGm={isSceneOwner} />
                  <AttackLog worldId={id} sceneId={sceneId} />
                </div>
              ) : null}

              {/* Spec 029 FR-011a: a token's resources, pinned by a double-click
               * and dragged wherever the viewer wants them. */}
              <StatusPanel
                resources={panelResources}
                title={pinnedTokenLabel}
                position={pinned?.at ?? { x: 0, y: 0 }}
                onMove={(at) => {
                  setPinned((current) =>
                    current ? { ...current, at } : current,
                  );
                  savedPinRef.current = at;
                  try {
                    localStorage.setItem(PANEL_PIN_KEY, JSON.stringify(at));
                  } catch {
                    // Storage refused. The position still applies for this
                    // session; only its persistence is lost, which is not
                    // worth interrupting play over.
                  }
                }}
                onClose={() => setPinned(null)}
              />

              {/* Bevy mounts canvas here */}
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
                  <EngineStopped
                    reason={engineStopped}
                    className={LOADING_PLATE}
                  />
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
              {isSceneOwner && sceneId ? (
                <AssetPasteTool
                  worldId={id}
                  sceneId={sceneId}
                  active={true}
                  onPasted={handleAssetPasted}
                />
              ) : null}
              {sceneId ? (
                <div
                  style={{
                    position: "absolute",
                    bottom: "1rem",
                    // Clear of the dock's icon rail on the right.
                    right: "4rem",
                    zIndex: 900,
                  }}
                >
                  <button
                    type="button"
                    data-testid="token-panel-toggle-button"
                    onClick={() => setTokenPanelOpen(true)}
                    className="token-panel-toggle-button"
                  >
                    Tokens
                  </button>
                  <TokenPanel
                    sceneId={sceneId}
                    worldStore={worldStore}
                    currentUserId={user?.id ?? null}
                    isSceneOwner={isSceneOwner}
                    isOpen={tokenPanelOpen}
                    onOpenChange={setTokenPanelOpen}
                    worldId={id}
                  />
                </div>
              ) : null}
              <div
                style={{
                  position: "absolute",
                  bottom: "1rem",
                  // Clear of the GM tool rail (3rem) on the left.
                  left: "4rem",
                  zIndex: 900,
                }}
              >
                <DiceRollerPanel
                  worldId={id}
                  engineReady={engineReady}
                  isGameMaster={isSceneOwner}
                />
              </div>
            </div>
          }
        />
      </div>

      {/*
        Outside the layout on purpose: it is a fixed overlay over the map, and
        nesting it in the dock would put it inside a scrolling, clipping
        container. Spec 031 FR-014.
      */}
      <PlacedItemPrompt worldId={id} actorId={claimedActorId} />
    </>
  );
}
