import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
  claimPlayField,
  subscribeToPlayFieldState,
  type PlayFieldState,
} from "../../services/playFieldClaim";
import { SEO } from "@/components/seo/SEO";
import { WorldLayout } from "@/layouts/world-layout/WorldLayout";
import type { SeoConfig } from "@/types/seo";
import { createWorldStore } from "@/engine/world/store";
import {} from "@/engine/world/sync";
import type { AppliedChange } from "@/engine/world/sync/reconcile";
import {
  StatusPanel,
  type PanelPosition,
} from "@/components/StatusPanel/StatusPanel";
import { railTools } from "@/lib/authoringTools";
import { usePlaceActorTokens } from "./usePlaceActorTokens";
import { useGmToolRail } from "./useGmToolRail";
import { useSceneLoadState } from "./useSceneLoadState";
import type { ReconcileReportState } from "./worldPageHelpers";
import { useSceneRecord } from "./useSceneRecord";
import { useEngineCanvas } from "./useEngineCanvas";
import { useBoardInput } from "./useBoardInput";
import { useEngineBridge } from "./useEngineBridge";
import {
  useExplorationControls,
  useExplorationPersistence,
} from "./useExploration";
import { useSceneTransition } from "./useSceneTransition";
import { useSceneContent } from "./useSceneContent";
import { useWorldEventStreams } from "./useWorldEventStreams";
import { useSceneLiveSync } from "./useSceneLiveSync";
import { useConnectivity } from "./useConnectivity";
import { useScenePlay } from "./useScenePlay";
import { usePinnedTokenStatus } from "./usePinnedTokenStatus";
import { useMapImport } from "./useMapImport";
import { useDockSections } from "./useDockSections";
import { WorldBoardOverlays } from "./WorldBoardOverlays";

import { useAuth } from "@/hooks/useAuth";
import { useWorldRole } from "@/hooks/useWorldRole";
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
import { GmToolRail } from "@/components/world/GmToolRail/GmToolRail";
import { SelectionFilterMenu } from "@/components/world/GmToolRail/SelectionFilterMenu";
import { PlacedItemPrompt } from "@/components/world/PlacedItemPrompt";
import { toast } from "sonner";
import { WorldDock } from "@/components/world/PlayDock/WorldDock";
import { AttackLog } from "@/components/world/PlayDock/AttackLog/AttackLog";
import { MapCreditLine } from "@/components/world/MapCredit";
import { OfferPrompt } from "@/components/world/PlayDock/OfferPrompt/OfferPrompt";
import type { WorldRecord } from "@/types/world";
import type { SceneRecord } from "@/types/scene";
import { PANEL_PIN_KEY, readSavedPin } from "./worldPageHelpers";

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

  useSceneContent({
    id,
    sceneId,
    worldStore,
    isSceneOwner,
    selectedScene,
    sceneRecordSettledFor,
    sceneLevel,
    bridgeReady,
    sceneLoadGranted,
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
  });

  useWorldEventStreams({
    id,
    engineReady,
    setSelectedSceneId,
    appliedRef,
    userIdRef,
    setReconcileReport,
  });

  // Read through a ref by the world-event loop below, which is declared
  // before the callback and must not reopen its stream when it changes.
  const reloadImportedSceneRef = useRef<(importedSceneId: string) => void>(
    () => {},
  );
  const { sceneAmbient, changeSceneAmbient } = useSceneLiveSync({
    id,
    sceneId,
    user,
    worldStore,
    bridgeReady,
    selectedScene,
    sceneLevel,
    sceneLevelList,
    levelActions,
    onLevelWorldEvent,
    sceneLevelRef,
    sceneLevelListRef,
    reloadImportedSceneRef,
    explorationEpochRef: explorationEpoch,
    setApprovalRevision,
  });

  const {
    explorationOn,
    explorationPlayers,
    changeExploration,
    resetExploration,
  } = useExplorationControls({ sceneId, isSceneOwner, allTokens });

  // Spec 045 FR-061: a light's reach is set in the system's units.
  const sceneUnits = useSceneUnits(sceneId, isSceneOwner);

  // The play field's right-click menu, and its keyboard equivalent: what this
  // viewer may do to the token under the pointer, or on bare board.
  const canvasMenu = useCanvasContextMenu(
    worldStore,
    Boolean(sceneId && user),
    isSceneOwner,
  );

  const claimedActorId = useScenePlay({
    id,
    sceneId,
    worldStore,
    bridgeReady,
    sceneLoadGranted,
    sceneLoadGeneration,
    setApprovalRevision,
  });

  const { pinnedTokenLabel, panelResources } = usePinnedTokenStatus({
    pinned,
    worldState,
  });

  const liveSyncState = useConnectivity({
    id,
    sceneId,
    user,
    worldStore,
    appliedRef,
    setReconcileReport,
    setScenes,
  });

  const { handleMapImportComplete, handleAssetPasted } = useMapImport({
    id,
    sceneId,
    worldStore,
    setScenes,
    setActiveSceneRecord,
    reloadImportedSceneRef,
  });

  const seo = useMemo<SeoConfig>(
    () => ({
      ...worldPageSeo,
      title: `${id || "World"} workspace`,
      canonicalPath: `/world/${id}/play`,
    }),
    [id],
  );

  const dockSections = useDockSections({
    id,
    sceneId,
    user,
    world,
    worldStore,
    isSceneOwner,
    scenes,
    setScenes,
    setSelectedSceneId,
    approvalRevision,
    setApprovalRevision,
    handleMapImportComplete,
    navigate,
  });

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
              <WorldBoardOverlays
                engineReady={engineReady}
                engineError={engineError}
                loadProgress={loadProgress}
                retryEngine={retryEngine}
                engineStopped={engineStopped}
                sceneId={sceneId}
                sceneLoadState={sceneLoadState}
                pendingSceneResources={pendingSceneResources}
                retrySceneLoad={retrySceneLoad}
                liveSyncState={liveSyncState}
                reconcileReport={reconcileReport}
                setReconcileReport={setReconcileReport}
              />
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
