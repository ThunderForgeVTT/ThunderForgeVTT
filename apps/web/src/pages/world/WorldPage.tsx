import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { WorldLayout } from "@/layouts/world-layout/WorldLayout";
import type { SeoConfig } from "@/types/seo";
import { createWorldStore } from "@/engine/world/store";
import {
  applyLightWorldEvent,
  applyShapeWorldEvent,
  applyTokenWorldEvent,
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
  subscribeToLiveSyncState,
  subscribeToWorldEvents,
  type LiveSyncState,
} from "@/engine/world/sync";
import { reconcileWorld } from "@/engine/world/sync/offlineQueue";
import {
  startHeartbeat,
  subscribeToHeartbeat,
} from "@/engine/world/sync/heartbeat";
import {
  parseReconciledEvent,
  pruneApplied,
  supersededBy,
  type AppliedChange,
  type ReconcileOutcome,
  type SubmittedChange,
} from "@/engine/world/sync/reconcile";
import { ReconcileReport } from "@/components/world/ReconcileReport";
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
}
import { useCanvasEngine } from "@/engine/bevy/useCanvasEngine";
import { EngineLoader } from "@/components/engine/EngineLoader";
import { getWorld } from "@/api/world";
import { getScene, getScenes } from "@/api/scenes";
import { useAuth } from "@/hooks/useAuth";
import { useWorldRole } from "@/hooks/useWorldRole";
import { WallTool } from "@/components/canvas-tools/WallTool";
import { LightingTool } from "@/components/canvas-tools/LightingTool";
import { ShapeTool } from "@/components/canvas-tools/ShapeTool";
import { AssetPasteTool } from "@/components/canvas-tools/AssetPasteTool";
import { TokenTool } from "@/components/canvas-tools/TokenTool";
import { TokenPanel } from "@/components/TokenPanel";
import { DiceRollerPanel } from "@/components/world/DiceRollerPanel/DiceRollerPanel";
import { startCanvasKeyboardRouting } from "@/engine/canvasKeyboard";
import { installWorldProbe } from "@/engine/world/probe";
import {
  createWorldFacets,
  type ControllableToken,
} from "@/engine/world/facets";
import { TokenStackPicker } from "@/components/canvas-tools/TokenStackPicker";
import { GmToolRail } from "@/components/world/GmToolRail/GmToolRail";
import {
  WorldDock,
  type DockSection,
} from "@/components/world/PlayDock/WorldDock";
import { ChatPanel } from "@/components/world/PlayDock/ChatPanel";
import { ActorsPanel } from "@/components/world/PlayDock/ActorsPanel";
import { CombatPanel } from "@/components/world/PlayDock/CombatPanel";
import { ClocksPanel } from "@/components/world/PlayDock/ClocksPanel";
import { SettingsPanel } from "@/components/world/PlayDock/SettingsPanel";
import type { CanvasImageAsset } from "@/api/assets";
import type { WorldRecord } from "@/types/world";
import type { SceneRecord } from "@/types/scene";

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
  const loaded = useRef(false);
  const canvasContainerId = "game-canvas-container";
  // Scene-scoped tokens are loaded from the server (tokens(sceneId)) once
  // a scene is selected — see the loadTokensIntoStore/startTokenMutationBridge
  // effect below — rather than seeded here with fixture data. The engine's
  // own demo "player"/"npc" tokens still spawn independently at Bevy
  // startup (src/engine/src/lib.rs) regardless of what's loaded here.
  const [worldStore] = useState(() =>
    createWorldStore({
      worldId: id,
    }),
  );
  const [worldState, setWorldState] = useState(() => worldStore.getState());
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
  // True once `bindWorldStore` has actually finished registering its
  // store subscription (the thing that forwards "sync"-sourced dispatches
  // — e.g. loadWallsIntoStore's confirmed wall rows — into the engine via
  // apply_world_command). NOT the same as `engineReady`: bindWorldStore is
  // reached through its own `import("@/engine/bevy")` dynamic-chunk hop,
  // which `useCanvasEngine`'s static-imported `mountEngine` doesn't pay,
  // so `engineReady` can and does flip true before this subscription
  // exists. Without gating on this specifically (found via
  // specs/002-canvas-authoring-asset-storage T014's live debugging),
  // walls/tokens/lights/shapes fetched on mount/reload can lose the race:
  // the GraphQL fetch resolves and dispatches "upsert_wall" (etc.) before
  // anyone is listening to relay it into the engine's WallSet/TokenSet/
  // etc. — the dispatch still updates React state correctly, so nothing
  // looks broken in the UI's data, but the engine-side resource silently
  // stays empty (e.g. no wall is selectable-by-click after a reload, even
  // though it's genuinely persisted and the property panel would show it
  // if selection worked). Every effect that dispatches something meant to
  // reach the engine gates on this now.
  const [bridgeReady, setBridgeReady] = useState(false);
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
  const sceneId = selectedSceneId;

  // Spec 004 (US4, T034-T036): scene-switch loading/error feedback. Before
  // this, the four per-scene loaders below (walls/tokens/lights/shapes)
  // plus the background-image dispatch had zero UI signal on failure —
  // just a `console.error`, per contracts/scene-load-state.md's
  // documented gap. `sceneLoadGeneration` is the "retry"/"latest wins"
  // mechanism (T036): bumping it both re-triggers every loader effect
  // below (added to each one's dependency array) and invalidates any
  // in-flight response from a stale scene/attempt via
  // `sceneLoadGenerationRef` — a loader that resolves after the user has
  // already switched scenes or hit retry again must not resurrect a
  // finished "loading" state or overwrite a newer error.
  const [sceneLoadGeneration, setSceneLoadGeneration] = useState(0);
  const sceneLoadGenerationRef = useRef(0);
  useEffect(() => {
    sceneLoadGenerationRef.current = sceneLoadGeneration;
  }, [sceneLoadGeneration]);

  type SceneLoadResource =
    | "background"
    | "walls"
    | "tokens"
    | "lights"
    | "shapes";
  type SceneLoadState =
    | { status: "loading" }
    | { status: "ready" }
    | { status: "error"; failedResource: SceneLoadResource };

  const [sceneLoadState, setSceneLoadState] = useState<SceneLoadState>({
    status: "loading",
  });
  const pendingSceneResourcesRef = useRef<Set<SceneLoadResource>>(new Set());

  // Reset to "loading" whenever the active scene or the retry generation
  // changes — mirrors the walls/shapes "clear the previous scene's stale
  // data" effects below, but for load-status rather than store contents.
  useEffect(() => {
    if (!sceneId) {
      return;
    }
    pendingSceneResourcesRef.current = new Set<SceneLoadResource>([
      "background",
      "walls",
      "tokens",
      "lights",
      "shapes",
    ]);
    setSceneLoadState({ status: "loading" });
  }, [sceneId, sceneLoadGeneration]);

  const markSceneResourceLoaded = useCallback(
    (resource: SceneLoadResource, generation: number) => {
      if (generation !== sceneLoadGenerationRef.current) {
        return;
      }
      pendingSceneResourcesRef.current.delete(resource);
      if (pendingSceneResourcesRef.current.size === 0) {
        setSceneLoadState((current) =>
          current.status === "error" ? current : { status: "ready" },
        );
      }
    },
    [],
  );

  const markSceneResourceFailed = useCallback(
    (resource: SceneLoadResource, generation: number) => {
      if (generation !== sceneLoadGenerationRef.current) {
        return;
      }
      // Background-image failure takes priority in the reported
      // `failedResource` if multiple fail simultaneously, per
      // contracts/scene-load-state.md — it's the most visually
      // disruptive, so don't let a later walls/lights/shapes/tokens
      // failure downgrade an already-reported background failure.
      setSceneLoadState((current) => {
        if (
          current.status === "error" &&
          current.failedResource === "background"
        ) {
          return current;
        }
        return { status: "error", failedResource: resource };
      });
    },
    [],
  );

  const retrySceneLoad = useCallback(() => {
    setSceneLoadGeneration((generation) => generation + 1);
  }, []);
  // The scene being played, fetched by id when the list does not carry it.
  //
  // `scenes(worldId:)` filters hidden scenes out for non-GMs (spec 022
  // FR-008), while `sceneId` comes from the world's unfiltered
  // `activeSceneId` — and a scene is hidden by default when created
  // (FR-003). So a player playing a scene the GM never un-hid had a scene
  // id and no scene record for the whole session, and the record is what
  // carries the map art and the grid: their canvas silently had neither.
  // The server now lets a member read the one scene their world is actually
  // playing, and this is the fetch that asks for it.
  const [activeSceneRecord, setActiveSceneRecord] = useState<SceneRecord | null>(
    null,
  );
  // Which scene id we have finished trying to find a record for, whether or
  // not we found one. The scene-load state below needs "we looked and there
  // is nothing" — resolving on "we have not looked yet" would clear the
  // loading overlay before the answer was known.
  const [sceneRecordSettledFor, setSceneRecordSettledFor] = useState<
    string | null
  >(null);
  const selectedScene =
    scenes.find((scene) => scene.sceneId === sceneId) ??
    (activeSceneRecord?.sceneId === sceneId ? activeSceneRecord : null);

  useEffect(() => {
    if (!id) {
      return;
    }

    let active = true;

    void getWorld(id)
      .then((response) => {
        if (active) {
          setWorld(response);
          // Spec 022 (FR-002d, ADR-046): scene selection is now
          // server-authoritative (`world.activeSceneId`), not "default to
          // the first scene" — a world nothing has ever been launched in
          // shows the empty/unloaded canvas state rather than picking one
          // arbitrarily. The functional update only fires if nothing has
          // set `selectedSceneId` yet (e.g. a live launch event, handled
          // elsewhere), so this never clobbers a switch already in flight.
          setSelectedSceneId(
            (current) => current ?? response?.activeSceneId ?? null,
          );
        }
      })
      .catch(() => {
        if (active) {
          setWorld(null);
        }
      });

    return () => {
      active = false;
    };
  }, [id]);

  useEffect(() => {
    if (!id) {
      return;
    }

    let active = true;

    void getScenes(id)
      .then((response) => {
        if (!active) {
          return;
        }
        setScenes(response);
        setScenesFetchedForWorld(id);
      })
      .catch((error) => {
        console.error("Failed to load world scenes:", error);
        if (active) {
          setScenes([]);
          setScenesFetchedForWorld(id);
        }
      });

    return () => {
      active = false;
    };
  }, [id]);

  // Only when the list came back without it — a GM's list already has every
  // scene, so this is one extra request for players and none for a GM.
  useEffect(() => {
    if (!sceneId || scenesFetchedForWorld !== id) {
      return;
    }
    if (scenes.some((scene) => scene.sceneId === sceneId)) {
      setActiveSceneRecord(null);
      setSceneRecordSettledFor(sceneId);
      return;
    }
    if (activeSceneRecord?.sceneId === sceneId) {
      return;
    }

    let active = true;
    void getScene(sceneId)
      .then((scene) => {
        if (active && scene) {
          setActiveSceneRecord(scene);
        }
      })
      .catch((error) => {
        // Non-fatal, and deliberately not an error to the user: the canvas
        // still renders tokens, walls and lights, which load on their own.
        // What is lost is the map art and the grid, and there is nothing
        // for anyone to do about it here.
        console.error("Failed to load the active scene:", error);
      })
      .finally(() => {
        if (active) {
          setSceneRecordSettledFor(sceneId);
        }
      });

    return () => {
      active = false;
    };
  }, [sceneId, scenes, scenesFetchedForWorld, id, activeSceneRecord]);

  // Spec 009: stabilized with useCallback — useCanvasEngine's mount effect
  // depends on this callback's identity (see useCanvasEngine.ts), so a
  // fresh inline function here re-triggers mountEngine() on every render.
  // That was always latent, but WorldPage.tsx didn't previously have a
  // render-triggering subscription (useWorldRole -> useWorldMembers, added
  // in this spec) frequent enough to actually surface it as an infinite
  // "Maximum update depth exceeded" loop.
  const handleEngineError = useCallback((err: Error) => {
    console.error("Bevy engine failed to mount:", err);
  }, []);

  // 🎮 Phase 4.7.F1: Use canvas engine hook for responsive sizing
  const {
    containerRef,
    engineReady,
    loadProgress,
    error: engineError,
    retry: retryEngine,
  } = useCanvasEngine({
    worldId: id,
    canvasSelector: `#${canvasContainerId}`,
    onError: handleEngineError,
  });

  // Spec 009: Bevy/winit inserts the real <canvas> as a direct child of
  // <body> (verified via live inspection), not as a descendant of
  // `#game-canvas-container` — the container div only reserves layout
  // space for it, it never actually contains it. Two consequences this
  // effect handles directly, since neither can be solved with CSS on our
  // own React tree:
  // 1. `display:none` on any wrapper around the container never hides the
  //    real canvas — it must be toggled on the canvas element itself.
  // 2. With no positioning of its own, the canvas sits in normal document
  //    flow as the last child of <body> — i.e. *after* the full-viewport
  //    `WorldLayout`/`WorldStagingPage` wrappers that come before it in the
  //    DOM, landing one viewport-height below where it's meant to render.
  //    `position: fixed; inset: 0` removes it from that flow entirely so
  //    it always fills the viewport regardless of surrounding page content.
  // A plain effect keyed on [playView] can miss the exact moment the
  // canvas is inserted (e.g. right after a reload, mid-WASM-boot) and
  // then never re-fire since nothing else changes — a MutationObserver on
  // <body>, plus applying playViewRef's current value whenever a canvas
  // shows up, catches it regardless of timing.
  const playViewRef = useRef(playView);
  playViewRef.current = playView;

  const applyCanvasVisibility = useCallback((canvas: HTMLCanvasElement) => {
    if (playViewRef.current === "playing") {
      canvas.style.display = "";
      canvas.style.position = "fixed";
      canvas.style.inset = "0";
    } else {
      canvas.style.display = "none";
    }
  }, []);

  /** Hides the body-level Bevy canvas without tearing the engine down. */
  const hideEngineCanvas = useCallback(() => {
    const canvas = document.querySelector<HTMLCanvasElement>("canvas");
    if (canvas) {
      canvas.style.display = "none";
    }
  }, []);

  useEffect(() => {
    const existing = document.querySelector<HTMLCanvasElement>("canvas");
    if (existing) {
      applyCanvasVisibility(existing);
    }

    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        mutation.addedNodes.forEach((node) => {
          if (node instanceof HTMLCanvasElement) {
            applyCanvasVisibility(node);
          }
        });
      }
    });
    observer.observe(document.body, { childList: true });

    return () => {
      observer.disconnect();
      // Bug fix: leaving Play (e.g. "Back to setup") unmounts this page,
      // but the canvas is a child of <body> with `position: fixed; inset: 0`
      // — it is not inside this component's tree, so React unmounting does
      // nothing to it and it stayed pinned over the whole viewport,
      // covering the staging page underneath. `playView` never leaves
      // "playing" (see its declaration), so the visibility effect below
      // could never hide it either: nothing in the component's own state
      // changes on navigation away. Hiding it here, in the one callback
      // that reliably runs on unmount, is what actually tears the canvas
      // down visually. The engine itself deliberately stays alive and
      // booted — spec 009's research.md §1 found invalidating its canvas
      // handle load-bearing not to do — so this hides, never destroys, and
      // the mount path above re-shows it on the way back in.
      hideEngineCanvas();
    };
  }, [applyCanvasVisibility, hideEngineCanvas]);

  useEffect(() => {
    const canvas = document.querySelector<HTMLCanvasElement>("canvas");
    if (canvas) {
      applyCanvasVisibility(canvas);
    }
  }, [playView, engineReady, applyCanvasVisibility]);

  // Keyboard input is routed to the canvas from the window rather than
  // relying on the canvas holding focus — clicking any control in the dock
  // or the tool rail used to stop keyboard movement dead until the map was
  // clicked again. See `engine/canvasKeyboard.ts` for why winit makes this
  // necessary.
  useEffect(() => startCanvasKeyboardRouting(), []);

  // Development-only introspection for the world store; see
  // `engine/world/probe.ts`. Compiled out of production builds.
  useEffect(() => installWorldProbe(worldStore), [worldStore]);

  // Stacked-token gestures. A single click takes the whole stack (handled
  // in the engine); a double-click asks which one, and that arrives here as
  // a `disambiguate_tokens` command for this picker to answer.
  const [stackPicker, setStackPicker] = useState<{
    members: ControllableToken[];
    at: { x: number; y: number };
  } | null>(null);

  const facets = useMemo(
    () =>
      createWorldFacets(worldStore, {
        worldId: id,
        sceneId,
        principal: {
          userId: user?.id ?? null,
          authority: isSceneOwner ? "gm" : user ? "player" : "observer",
        },
      }),
    [worldStore, id, sceneId, isSceneOwner, user],
  );

  // Double-click over the canvas asks *which* of the stacked tokens. The
  // click that precedes it has already selected the stack, so this only has
  // to offer a choice between what is selected. Detected here rather than in
  // the engine: two clicks that fast routinely land in one Bevy frame, where
  // the second press is lost outright, while the DOM's `dblclick` is exact.
  useEffect(() => {
    const onDoubleClick = (event: MouseEvent) => {
      if (!(event.target instanceof HTMLCanvasElement)) return;
      const stack = facets.selection.disambiguate();
      if (!stack) return;
      setStackPicker({
        members: stack.members,
        at: { x: event.clientX, y: event.clientY },
      });
    };

    window.addEventListener("dblclick", onDoubleClick, { capture: true });
    return () => {
      window.removeEventListener("dblclick", onDoubleClick, { capture: true });
      facets.stop();
    };
  }, [facets]);

  useEffect(
    () => worldStore.subscribe((event) => setWorldState(event.state)),
    [worldStore],
  );

  useEffect(() => {
    if (loaded.current) {
      return;
    }

    loaded.current = true;

    // Engine is now mounted via useCanvasEngine hook
    // Still need to bind world store for mutations
    void import("@/engine/bevy").then(({ bindWorldStore }) =>
      bindWorldStore(worldStore).then(() => setBridgeReady(true)),
    );
  }, [worldStore]);

  // Spec 028 (US1): bring the local world cache into agreement with the
  // server for the world being opened, before its assets are asked for.
  //
  // Runs on identity, not on `engineReady`: the sync is a network round trip
  // plus OPFS/IndexedDB work that has nothing to do with the render loop, and
  // the sooner it finishes the more of this visit's asset loads can be served
  // from disk. Anything it publishes late simply misses those loads, which
  // fall back to the network — the same outcome as no cache at all.
  //
  // Not awaited by anything and not gated on. `syncWorldCache` never throws
  // and never rejects: a browser without OPFS, a missing session key, an
  // unreachable server or a malformed plan all resolve to a "degraded"
  // summary and leave the page on exactly today's behaviour. A cache problem
  // must not be able to stop a world from opening.
  useEffect(() => {
    const userId = user?.id;
    if (!id || !userId) {
      return;
    }

    void import("@/engine/bevy").then(({ syncWorldCache }) =>
      syncWorldCache(id, userId).then((summary) => {
        // The only observation this side makes of the cache, and it is
        // diagnostic: what was held, fetched and evicted, never *which*
        // items. See `WorldCacheSyncSummary`.
        console.debug("[world-cache] sync", summary);
      }),
    );
  }, [id, user?.id]);

  // FR-010: tell the engine whether this session may author walls/shapes.
  // Without this, WallPlugin/ShapePlugin's IsGameMaster resource stays at
  // its `false` default forever and no click/keyboard authoring input is
  // ever accepted, regardless of `isSceneOwner`. Re-sent whenever
  // ownership resolves/changes or the engine (re)becomes ready, since a
  // fresh `start()` always resets the wasm side back to the default.
  useEffect(() => {
    if (!engineReady) {
      return;
    }

    void import("@/engine/bevy").then(({ setIsGameMaster }) =>
      setIsGameMaster(isSceneOwner),
    );
  }, [engineReady, isSceneOwner]);

  useEffect(() => {
    if (!id || !bridgeReady) {
      return;
    }

    worldStore.dispatch({ type: "set_world", worldId: id }, "ui");

    void import("@/engine/bevy").then(({ setActiveWorld }) =>
      setActiveWorld(id),
    );
  }, [id, worldStore, bridgeReady]);

  useEffect(() => {
    if (!bridgeReady) {
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
    worldStore.dispatch(
      {
        type: "set_scene_background",
        backgroundImagePath: selectedScene.backgroundUrl,
        width: selectedScene.width,
        height: selectedScene.height,
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
        mapWidth: selectedScene.width,
        mapHeight: selectedScene.height,
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
    if (!selectedScene.backgroundUrl) {
      markSceneResourceLoaded("background", generation);
      return;
    }

    let cancelled = false;
    void fetch(selectedScene.backgroundUrl, {
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
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
  ]);

  useEffect(() => {
    if (!sceneId || !bridgeReady) {
      return;
    }

    // Switching scenes must clear the *previous* scene's walls first:
    // loadWallsIntoStore only ever upserts, so without this, a
    // previously-visited scene's walls (and a stale selection pointing
    // at one of them) stay rendered/selectable on top of whichever scene
    // is now active, indefinitely accumulating across scene switches for
    // the life of the session (found while investigating spec 002 T017 —
    // the equivalent gap for shapes below reproduced as a real, visible
    // bug, not just a stale-selection artifact: an old scene's shape was
    // still spawned and hit-testable in the engine after switching
    // scenes). `"sync"` source: a confirmed-state correction, not a
    // delete intent — startWallMutationBridge below ignores
    // `source: "sync"` dispatches, so this never calls deleteWall.
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
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
  ]);

  useEffect(() => {
    if (!sceneId || !bridgeReady) {
      return;
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
    isSceneOwner,
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
    // The world the token bridge queues offline edits against (spec 028 US7).
    id,
  ]);

  useEffect(() => {
    if (!sceneId || !bridgeReady) {
      return;
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
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
  ]);

  useEffect(() => {
    if (!sceneId || !bridgeReady) {
      return;
    }

    // Clear the previous scene's shapes before loading this scene's —
    // see the matching comment on the walls effect above for why. This
    // is the fix for the real bug behind spec 002 T017's failure: without
    // it, a shape drawn on scene A stayed spawned (and hit-testable) in
    // the engine after switching to scene B, so a click on B's empty
    // canvas at the same on-screen position re-selected A's shape.
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

    const iterator = subscribeToWorldEvents(id)[Symbol.asyncIterator]();
    let cancelled = false;

    (async () => {
      try {
        while (!cancelled) {
          const { value: event, done } = await iterator.next();
          if (done || cancelled || !event) break;
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

  // Live cross-client sync: one subscription per mounted scene, feeding
  // every apply*WorldEvent for this scene's canvas primitives — each of
  // those already filters by its own event code internally (walls=10,
  // tokens=14, etc, see their own doc comments), so a single shared
  // subscription driving all four is correct and avoids opening four
  // separate WebSocket subscriptions for the same event stream. The
  // backend transport (Postgres listener -> broadcast channel -> this
  // `worldEventsCreated` GraphQL subscription -> /api/ws) already existed
  // in full; this is the first thing in apps/web to actually open it.
  useEffect(() => {
    if (!id || !sceneId || !bridgeReady) {
      return;
    }

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
          await Promise.all([
            applyWallWorldEvent(worldStore, sceneId, event),
            applyTokenWorldEvent(worldStore, sceneId, event),
            applyShapeWorldEvent(worldStore, sceneId, event),
            applyLightWorldEvent(worldStore, sceneId, event),
          ]);
        }
      } catch (error) {
        console.error("World events live-sync error:", error);
      }
    })();

    return () => {
      cancelled = true;
      void iterator.return?.();
    };
  }, [id, sceneId, bridgeReady, worldStore]);

  // Spec 005 (T014-T016, data-model.md "LiveSyncState"): track this tab's
  // one shared subscription-transport connection state, so a persistent
  // "reconnecting" indicator can render (FR-009/FR-009a) and so a dropped
  // connection triggers a full scene re-fetch once it comes back — the
  // real gap this project's earlier audit found (subscriptionClient.ts
  // had automatic WebSocket retry via graphql-ws, but nothing reacted to
  // a reconnect to recover events missed during the outage).
  const [liveSyncState, setLiveSyncState] = useState<LiveSyncState>(() =>
    getLiveSyncState(),
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
      void reconcileWorld(worldIdNow)
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
      setLiveSyncState(state);
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
          void reconcileWorld(worldIdNow)
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
                void getScenes(worldIdNow).then(setScenes).catch(() => {});
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

  const handleMapImportComplete = useCallback(() => {
    if (!sceneId || !id) {
      return;
    }

    // Map import creates walls/doors/lights directly in Postgres (via the
    // REST endpoint, not the GraphQL mutation bridge), so re-run the same
    // loaders used on initial mount to pull the newly imported content into
    // the world store without requiring a manual page reload.
    void loadWallsIntoStore(worldStore, sceneId).catch((error) => {
      console.error("Failed to reload scene walls after map import:", error);
    });
    void loadLightsIntoStore(worldStore, sceneId).catch((error) => {
      console.error("Failed to reload scene lights after map import:", error);
    });
    void loadShapesIntoStore(worldStore, sceneId).catch((error) => {
      console.error("Failed to reload scene shapes after map import:", error);
    });
    // Import also sets the scene's background_asset_id — refetch scenes so
    // the background-dispatch effect above (reading `backgroundUrl`) picks
    // up the new art.
    void getScenes(id)
      .then(setScenes)
      .catch((error) => {
        console.error("Failed to reload scenes after map import:", error);
      });
  }, [sceneId, id, worldStore]);

  // T023/T030 (specs/002-canvas-authoring-asset-storage): a pasted image
  // is already persisted by the time AssetPasteTool calls this (the
  // GraphQL mutation already succeeded) — this just tells the engine to
  // spawn it. `path` points at the authenticated `/canvas-assets/{id}`
  // proxy route (src/server/src/canvas_assets_serve.rs), not RustFS
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
      content: <ActorsPanel worldId={id} />,
    },
    {
      id: "combat",
      label: "Combat",
      icon: "shield",
      content: (
        <CombatPanel worldId={id} sceneId={sceneId} isGm={isSceneOwner} />
      ),
    },
    {
      id: "clocks",
      label: "Clocks & Timers",
      icon: "moon",
      content: (
        <ClocksPanel
          worldId={id}
          isGm={isSceneOwner}
          gameSystemId={world?.gameSystemId ?? null}
        />
      ),
    },
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
  ];

  return (
    <>
      <SEO {...seo} />
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
      <div style={{ display: playView === "playing" ? "block" : "none" }}>
        <WorldLayout
          worldId={id}
          toolRail={
            isSceneOwner && sceneId ? (
              <GmToolRail
                tools={[
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
                ]}
              />
            ) : null
          }
          dock={<WorldDock sections={dockSections} />}
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
                  <EngineLoader progress={loadProgress} error={null} />
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
                    onRetry={retryEngine}
                  />
                </div>
              )}
              {sceneId && sceneLoadState.status === "loading" ? (
                <div
                  data-testid="scene-load-indicator"
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
                  <p>Loading scene…</p>
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
              {liveSyncState.status !== "live" ? (
                // Spec 005 (FR-009/FR-009a): a persistent, non-blocking
                // indicator — unlike sceneLoadState's centered overlay above,
                // this must never block interaction with an already-loaded
                // scene, and must never present a dead-end/terminal state:
                // it just keeps showing "Reconnecting…" for as long as
                // liveSyncState says so, since the underlying transport
                // retries indefinitely.
                <div
                  data-testid="live-sync-reconnecting-indicator"
                  data-sync-status={liveSyncState.status}
                  style={{
                    position: "absolute",
                    top: "1rem",
                    // Clear of the dock's icon rail (3rem) on the right.
                    right: "4rem",
                    zIndex: 1000,
                    padding: "0.4rem 0.75rem",
                    borderRadius: "0.375rem",
                    background: "rgba(0, 0, 0, 0.75)",
                    color: "white",
                    fontSize: "0.8rem",
                  }}
                >
                  {liveSyncState.status === "connecting"
                    ? "Connecting…"
                    : liveSyncState.status === "disconnected"
                      ? // Spec 028 US7 (T073). What matters to say here is not
                        // "something is wrong" — the user can see that — but
                        // that their work is being kept and that they may
                        // carry on. An indicator that only announced the
                        // failure would leave someone guessing whether to stop
                        // moving tokens, which is the opposite of the point.
                        liveSyncState.browserOffline
                        ? "Offline — your changes are saved here and will sync when you reconnect"
                        : "Can't reach the server — your changes are saved here and will sync when it's back"
                      : `Reconnecting… (attempt ${liveSyncState.attempt})`}
                </div>
              ) : null}
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
                <DiceRollerPanel worldId={id} engineReady={engineReady} />
              </div>
            </div>
          }
        />
      </div>
    </>
  );
}
