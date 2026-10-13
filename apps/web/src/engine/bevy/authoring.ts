import { getWasmModule } from "./wasmModule";

/**
 * Tell the engine which authoring tool is armed.
 *
 * The engine is the authority for this (Constitution I): every canvas input
 * system is engine-side, and until now none of them knew which tool the Game
 * Master had chosen — `openGmToolId` lived in React and only picked a flyout.
 * That is why a click could be claimed by wall drawing while the rail showed
 * Lights.
 *
 * Optional by design. A bundle that predates the mode simply does not export
 * it, and the caller carries on: the tool rail still renders, and the engine
 * behaves as it did before. Never throws — a failed mode change must not take
 * the canvas down with it.
 *
 * Returns whether the engine recognised the tool.
 */
export async function setAuthoringMode(toolId: string): Promise<boolean> {
  try {
    const module = await getWasmModule();
    if (!module.set_authoring_mode) return false;
    return module.set_authoring_mode(toolId);
  } catch {
    return false;
  }
}

/**
 * Tell the engine which shape a drag in the Shapes tool draws — `"freehand"`,
 * `"rect"`, `"ellipse"`, `"line"`, or `"none"` to select and move instead.
 *
 * Playtest 2026-09-10 P10: the Shapes panel's buttons only ever changed React
 * state, and the engine's shape kind was reachable from the number keys alone,
 * so a drag after clicking "Rectangle" drew nothing. `"text"` is `"none"` to
 * the engine: text is placed by `ShapeTool` itself, in the DOM.
 *
 * Returns whether the engine recognised the kind. Never throws, like
 * `setAuthoringMode`.
 */
export async function setActiveShapeTool(kind: string): Promise<boolean> {
  try {
    const module = await getWasmModule();
    const set = (
      module as { set_active_shape_tool?: (kind: string) => boolean }
    ).set_active_shape_tool;
    if (!set) return false;
    return set(kind);
  } catch {
    return false;
  }
}

/**
 * Turn the engine's snapping switch on or off (spec 077 FR-001). Session
 * state like `setIsGameMaster`, never synced; the engine answers with
 * `grid_snap_changed`, which is where the rail reads the result. Never
 * throws.
 */
export async function setGridSnap(enabled: boolean): Promise<void> {
  try {
    const module = await getWasmModule();
    module.apply_world_command?.(
      JSON.stringify({ type: "set_grid_snap", enabled }),
    );
  } catch {
    // A bundle without the switch keeps snapping on, as it always has.
  }
}

/**
 * Tell the engine what a drag with the wall tool draws — `"segment"`,
 * `"room"` (four walls between two corners), `"circle"` (a ring from centre
 * to radius) or `"door"` (a closed door; spec 077 FR-011).
 *
 * The engine has offered all three since spec 031 FR-026, but nothing in the
 * web app asked for anything but the default segment, so a Game Master can
 * only reach a room or a door by drawing segments and pressing `O`. This is the
 * seam a control in the Walls panel calls; the e2e that proves a drawn room
 * holds a player in uses it today.
 *
 * Returns whether the engine recognised the primitive. Never throws, like
 * `setAuthoringMode`.
 */
export async function setWallPrimitive(primitive: string): Promise<boolean> {
  try {
    const module = await getWasmModule();
    const set = (
      module as { set_wall_primitive?: (primitive: string) => boolean }
    ).set_wall_primitive;
    if (!set) return false;
    return set(primitive);
  } catch {
    return false;
  }
}

/**
 * Tell the engine which tools this viewer may use.
 *
 * Not a hint. The engine refuses a mode request outside this set and disarms
 * the input systems for anything outside it, so a client that skips this call
 * gets the unrestricted default — which is why the caller must send `""` for a
 * person with no tools rather than simply not calling (FR-045: the default the
 * *server* declares is Game-Master-only, and an engine told nothing cannot
 * know that).
 *
 * Optional by design, like `setAuthoringMode`: a bundle predating tool
 * permissions does not export it, and the role checks that gated authoring
 * before still apply.
 */
export async function setAllowedAuthoringTools(
  toolIds: readonly string[],
): Promise<void> {
  try {
    const module = await getWasmModule();
    module.set_allowed_authoring_tools?.(toolIds.join(","));
  } catch {
    // A failed restriction must not take the canvas down. The server refused
    // the write regardless; this call only decides what is offered.
  }
}

/**
 * Remove any tool restriction from the engine.
 *
 * For a viewer who holds every tool. Distinct from sending the full list so
 * the engine's "no declaration" default and "granted everything" state stay
 * distinguishable when reading it back.
 */
export async function clearAllowedAuthoringTools(): Promise<void> {
  try {
    const module = await getWasmModule();
    module.clear_allowed_authoring_tools?.();
  } catch {
    // As above.
  }
}

/**
 * Which tool the engine currently has armed, or `null` if it cannot say.
 *
 * Read-only, and here so the boundary can be observed directly rather than
 * inferred from what a click did.
 */
export async function getAuthoringMode(): Promise<string | null> {
  try {
    const module = await getWasmModule();
    if (!module.authoring_mode) return null;
    return module.authoring_mode();
  } catch {
    return null;
  }
}

/**
 * Tell the engine which kinds the Select tool acts on.
 *
 * Selection is engine state; this only carries the person's preference across
 * the boundary. Optional like the rest — a bundle without it leaves selection
 * unfiltered, which is the pre-existing behaviour and a safe place to land.
 */
export async function setSelectionFilter(
  tokens: boolean,
  walls: boolean,
  lights: boolean,
  shapes: boolean,
): Promise<void> {
  try {
    const module = await getWasmModule();
    module.set_selection_filter?.(tokens, walls, lights, shapes);
  } catch {
    // A filter that cannot be applied is a tool that selects everything, which
    // is what it did before this existed.
  }
}

/**
 * Attach an actor's token to the cursor, to be placed by a left click.
 *
 * The engine owns the carry: the preview, the snapping and the cancel all live
 * there, because screen-to-world is the camera's business and a React element
 * following the mouse would drift against it.
 *
 * Returns whether the carry began.
 */
export async function beginTokenPlacement(actorId: string): Promise<boolean> {
  try {
    const module = await getWasmModule();
    return module.begin_token_placement?.(actorId) ?? false;
  } catch {
    return false;
  }
}

/**
 * Carry something that is not an actor's token, to be placed by a left click.
 *
 * The same gesture as `beginTokenPlacement`, and deliberately the same machine
 * (`crates/thunderforge-engine/src/plugins/placement.rs`): a second one would have to re-derive
 * snapping, the preview, and abandoning the carry when the tool or the scene
 * changes. `reference` is opaque to the engine and comes back on the drop.
 *
 * Returns whether the carry began. `false` means this bundle predates
 * `begin_placement`, so no drop will ever be reported and a caller waiting for
 * one would wait forever.
 */
export async function beginPropPlacement(reference = ""): Promise<boolean> {
  try {
    const module = await getWasmModule();
    return module.begin_placement?.("prop", reference) ?? false;
  } catch {
    return false;
  }
}

/** Abandon a placement in progress. Leaves nothing behind. */
export async function cancelTokenPlacement(): Promise<void> {
  try {
    const module = await getWasmModule();
    module.cancel_token_placement?.();
  } catch {
    // Nothing to abandon.
  }
}

/**
 * Begin swapping the canvas over to `sceneId`.
 *
 * The engine takes the previous scene's tokens, walls, lights, shapes,
 * interactives and selection off the canvas itself (FR-018) and then emits
 * `scene_load_requested`. Subscribe with `onSceneLoadRequested` and fetch
 * *then*: the despawn happens on a later frame, so anything fetched from here
 * is racing it.
 *
 * Returns whether the engine took the request. `false` means the machine is
 * absent from this bundle, the wasm layer is unreachable, or the id was
 * empty — in every one of those cases nothing will be despawned and no
 * request will ever be emitted, so a caller waiting for one would wait
 * forever and must load by itself instead.
 */
export async function beginSceneTransition(sceneId: string): Promise<boolean> {
  try {
    const module = await getWasmModule();
    return module.begin_scene_transition?.(sceneId) ?? false;
  } catch {
    return false;
  }
}

/**
 * Tell the engine that every command for the new scene has been sent.
 *
 * This is not optional politeness. `Loading` has no timeout on purpose — the
 * engine has no way to judge whether chrome has failed or is merely slow — so
 * a load path that reaches its end without calling this leaves the machine
 * waiting for the rest of a scene that is already fully here. Call it on
 * every exit from the load, the failed ones included: "chrome has stopped
 * sending" is the honest report, and a partly-loaded scene the person can see
 * is better than a transition that never ends.
 */
export async function completeSceneTransition(): Promise<void> {
  try {
    const module = await getWasmModule();
    module.complete_scene_transition?.();
  } catch {
    // A bundle without the machine was never in a transition to finish.
  }
}
