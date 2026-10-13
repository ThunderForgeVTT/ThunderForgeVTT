import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
  refreshInteractives,
  setScenePlaying,
  startTriggerBridge,
  loadLightsIntoStore,
  loadShapesIntoStore,
  loadWallsIntoStore,
} from "@/engine/world/sync";
import type { AppliedChange } from "@/engine/world/sync/reconcile";
import { ReconcileReport } from "@/components/world/ReconcileReport";
import { ConnectionStatus } from "@/components/world/ConnectionStatus";
import {
  StatusPanel,
  type PanelPosition,
} from "@/components/StatusPanel/StatusPanel";
import {
  readTokenStatus,
  type TokenStatusResource,
} from "@/engine/bevy/tokenStatus";
import { onOpenLore } from "@/engine/bevy";
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
import { PeerIndicator } from "@/components/diagnostics/PeerIndicator";
import { EngineMonitor } from "@/components/diagnostics/EngineMonitor";

import { EngineLoader } from "@/components/engine/EngineLoader";
import { EngineStopped } from "@/components/engine/EngineStopped";
import { BoardLoading } from "@/components/engine/BoardLoading";
import { getScene, getScenes } from "@/api/scenes";
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
import type { WorldRecord } from "@/types/world";
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

  const liveSyncState = useConnectivity({
    id,
    sceneId,
    user,
    worldStore,
    appliedRef,
    setReconcileReport,
    setScenes,
  });

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
