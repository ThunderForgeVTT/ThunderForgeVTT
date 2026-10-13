import { getWasmModule } from "./wasmModule";

/**
 * Tell the engine who is looking (spec 082), so it lets a player edit only
 * the shapes they drew. Session state like `setIsGameMaster`, never synced;
 * re-sent whenever the engine becomes ready. Never throws: a bundle that
 * predates the command ignores it.
 */
export async function setViewerUser(userId: string | null): Promise<void> {
  try {
    const module = await getWasmModule();
    module.apply_world_command?.(
      JSON.stringify({ type: "set_viewer_user", userId }),
    );
  } catch {
    // An engine that is not there yet is told again when it is ready.
  }
}

/**
 * FR-010: tells the engine whether the local session may author
 * walls/shapes (`WallPlugin`/`ShapePlugin` gate all pointer/keyboard
 * authoring input on this). Not modeled as a `WorldCommand` — it's local
 * session state, not something to sync into `WorldState` or broadcast —
 * so this calls the wasm bridge directly rather than going through
 * `worldStore.dispatch`, same shape as `bindWorldStore`'s direct
 * `apply_world_command` calls.
 */
export async function setIsGameMaster(isGameMaster: boolean): Promise<void> {
  const module = await getWasmModule();
  if (!module.apply_world_command) {
    return;
  }
  module.apply_world_command(
    JSON.stringify({ type: "set_is_game_master", isGameMaster }),
  );
}

/**
 * Playtest 2026-09-10 P9: name the token this client sees the board through —
 * a player's own — or `null` for none, which is a Game Master's case. Walls
 * then hide from a player what their token cannot see.
 *
 * Local session state, like `setIsGameMaster`: which token is "mine" is a
 * fact about this viewer, never synced. Never throws; a bundle predating it
 * simply keeps the board-wide view.
 */
export async function setViewerToken(tokenId: string | null): Promise<void> {
  try {
    const module = await getWasmModule();
    const set = (module as { set_viewer_token?: (id: string) => boolean })
      .set_viewer_token;
    set?.(tokenId ?? "");
  } catch {
    // A viewer that could not be named leaves the board-wide view.
  }
}

/**
 * Spec 045: name the token this client's own player may move — their primary
 * one, else any they own — or `null` for none.
 *
 * Not the same question as `setViewerToken`, and deliberately a second call:
 * a Game Master sees through no token and may still move any of them. Until
 * this existed, the engine's movement keys drove the one entity it tags at
 * startup — a placeholder — so a player pressing a movement key moved
 * nothing of theirs, on any scene, ever.
 *
 * Local session state, like the viewer: never synced, never broadcast.
 */
export async function setControlledToken(
  tokenId: string | null,
): Promise<void> {
  try {
    const module = await getWasmModule();
    const set = (module as { set_controlled_token?: (id: string) => boolean })
      .set_controlled_token;
    set?.(tokenId ?? "");
  } catch {
    // A bundle predating this controls nothing, exactly as before.
  }
}

/**
 * Spec 045 US7: whether this scene remembers where a player has been.
 *
 * The Game Master's setting, read from the server. Off is the default and the
 * safe answer: a scene that has never been asked to remember must not start
 * hiding its map from the table.
 */
export async function setExploration(enabled: boolean): Promise<void> {
  try {
    const module = await getWasmModule();
    const set = (module as { set_exploration?: (on: boolean) => boolean })
      .set_exploration;
    set?.(enabled);
  } catch {
    // A bundle predating this remembers nothing, exactly as before.
  }
}

/**
 * Hand the engine what this player's browser remembered.
 *
 * An empty list is a **reset**, not a silence: the engine distinguishes the
 * two, because "the Game Master cleared your map" has to survive a frame in
 * which nothing else happens.
 */
export async function setExploredCells(
  cells: [number, number][],
): Promise<void> {
  try {
    const module = await getWasmModule();
    const set = (module as { set_explored_cells?: (json: string) => boolean })
      .set_explored_cells;
    set?.(cells.length === 0 ? "" : JSON.stringify(cells));
  } catch {
    // As above.
  }
}

/** What the engine has accumulated, for the browser to keep. */
export async function exploredCells(): Promise<[number, number][]> {
  try {
    const module = await getWasmModule();
    const read = (module as { explored_cells?: () => string }).explored_cells;
    if (!read) {
      return [];
    }
    return JSON.parse(read()) as [number, number][];
  } catch {
    return [];
  }
}

/**
 * Spec 045 FR-033: name the tokens the table's players see through, so a Game
 * Master's board can mark what the party cannot see.
 *
 * Only a Game Master's client sends this. A player is never told what anyone
 * else can see — their own board answers one question, about their own token.
 *
 * The engine is given ids rather than left to work them out: "whose token is
 * this" is a question about accounts and world membership, which the engine
 * has never known and should not learn.
 */
export async function setPartyEyes(tokenIds: string[]): Promise<void> {
  try {
    const module = await getWasmModule();
    const set = (module as { set_party_eyes?: (json: string) => boolean })
      .set_party_eyes;
    set?.(JSON.stringify(tokenIds));
  } catch {
    // A bundle predating this marks nothing, exactly as before.
  }
}

/**
 * Spec 032: hands the world's interface pack its canvas half.
 *
 * The engine has accepted `set_display_appearance` since spec 029 — the
 * command parses, `StatusDisplayPlugin` owns the `Appearance` resource it
 * writes, and the TypeScript SDK types it in `sdk/commands.ts`. Until now
 * nothing called it. Every layer existed and nothing joined them, which is the
 * same shape as the two dead paths found during spec 031.
 *
 * Like `setIsGameMaster`, this goes to the wasm bridge directly rather than
 * through `worldStore.dispatch`: an appearance is presentation, not world
 * state to persist or broadcast. The server already told everyone the pack
 * changed; this is each client applying it to its own canvas.
 *
 * A pack with no `canvas` block sends nothing, and the engine keeps its own
 * defaults — an empty override would say "reset everything to nothing", which
 * is a different claim.
 */
export async function setDisplayAppearance(
  appearance: Record<string, unknown> | null,
): Promise<void> {
  if (!appearance) {
    return;
  }
  const module = await getWasmModule();
  if (!module.apply_world_command) {
    return;
  }
  module.apply_world_command(
    JSON.stringify({ type: "set_display_appearance", appearance }),
  );
}
