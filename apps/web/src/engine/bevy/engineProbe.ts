import type { BevyWasmModule } from "./wasmModule";

/** Read by `__engineProbe.dicePlayed`; never read by the app itself. */
export const dicePlayed: number[][] = [];

/**
 * A read-only window onto the engine, for tests and for when the canvas says
 * nothing — the engine's counterpart to `window.__worldProbe`, which can only
 * see the store. Playtest 2026-09-10 P3: a pan happens in the engine and
 * nowhere else, so a test has to be able to ask the engine where the camera
 * is. Development only, like the world probe: `import.meta.env.DEV` is a
 * constant, so none of this reaches a production bundle. The one exception
 * is a build made with `VITE_ENGINE_PROBE=1`, which the demo's e2e asks for
 * (spec 076 FR-014) because the demo is only ever a production build; the
 * image builds the demo itself, so that build never ships.
 */
export function installEngineProbe(wasm: BevyWasmModule): void {
  const wanted =
    import.meta.env.DEV || import.meta.env.VITE_ENGINE_PROBE === "1";
  if (!wanted || typeof window === "undefined") return;
  const cameraState = (wasm as { camera_state?: () => string }).camera_state;
  const tokenNameplates = (wasm as { token_nameplates?: () => string })
    .token_nameplates;
  const hiddenTokens = (wasm as { hidden_tokens?: () => string }).hidden_tokens;
  const markedTokens = (wasm as { marked_tokens?: () => string }).marked_tokens;
  const dimTokens = (wasm as { dim_tokens?: () => string }).dim_tokens;
  const tokenVision = (wasm as { token_vision?: () => string }).token_vision;
  const carriedLights = (wasm as { carried_lights?: () => string })
    .carried_lights;
  const placedLights = (wasm as { placed_lights?: () => string }).placed_lights;
  const doorIcons = (wasm as { door_icons?: () => string }).door_icons;
  const movementState = (wasm as { movement_state?: () => string })
    .movement_state;
  const tokenFootprints = (wasm as { token_footprints?: () => string })
    .token_footprints;
  const focusState = (wasm as { focus_state?: () => string }).focus_state;
  const tokenConditions = (wasm as { token_conditions?: () => string })
    .token_conditions;
  const sightProbe = (
    wasm as { sight_probe?: (x: number, y: number) => string }
  ).sight_probe;
  const diceLanded = (wasm as { dice_landed?: () => string }).dice_landed;
  const drawnWallIds = (wasm as { drawn_wall_ids?: () => string })
    .drawn_wall_ids;
  const selectionState = (wasm as { selection_state?: () => string })
    .selection_state;
  const diceEntityCount = (wasm as { dice_entity_count?: () => number })
    .dice_entity_count;
  const timeOfDayState = (wasm as { time_of_day_state?: () => string })
    .time_of_day_state;
  (window as unknown as Record<string, unknown>).__engineProbe = {
    // Spec 099 SC-006: the table's clock — its hour and the darkness drawn
    // from it — or `null` when the scene has none.
    timeOfDay: (): { hour: number; darkness: number } | null =>
      timeOfDayState
        ? (JSON.parse(timeOfDayState()) as {
            hour: number;
            darkness: number;
          } | null)
        : null,
    // Spec 085: the walls this canvas draws a sprite for, sorted. A wall
    // hidden from the table is absent on a player's board and present on the
    // Game Master's; it blocks on both.
    drawnWalls: (): string[] =>
      drawnWallIds ? (JSON.parse(drawnWallIds()) as string[]) : [],
    // Spec 085: what this board holds selected, per kind, read from the
    // engine's own group.
    selection: (): {
      tokens: string[];
      walls: string[];
      lights: string[];
      shapes: string[];
    } =>
      selectionState
        ? (JSON.parse(selectionState()) as {
            tokens: string[];
            walls: string[];
            lights: string[];
            shapes: string[];
          })
        : { tokens: [], walls: [], lights: [], shapes: [] },
    // Spec 083: the throws this board finished or skipped, oldest first
    // (contracts/engine-dice.md), and how many throw entities are alive.
    diceLanded: (): unknown[] =>
      diceLanded ? (JSON.parse(diceLanded()) as unknown[]) : [],
    diceEntities: (): number => (diceEntityCount ? diceEntityCount() : 0),
    // Spec 081: every set of dice this board was asked to animate, oldest
    // first. Recorded once the engine took the command, so a roll that never
    // reached the board — masked, hidden, or caught up — is not in it.
    dicePlayed: (): number[][] => dicePlayed.map((dice) => dice.slice()),
    // Spec 076 FR-013: what the darkness sheet shows of the map at a world
    // point. `looking` is whether this canvas sees through a token at all;
    // `seen` is whether that token's line of sight reaches the point. The
    // engine answers from the row it uploaded, with the shader's arithmetic.
    sight: (x: number, y: number): { looking: boolean; seen: boolean } =>
      sightProbe
        ? (JSON.parse(sightProbe(x, y)) as { looking: boolean; seen: boolean })
        : { looking: false, seen: true },
    // Owner decision 2026-09-15: what became of the last "look at this
    // creature" — moved, or refused and why. A refusal draws nothing by
    // design, so without this it cannot be told from a bug.
    focusState: (): { tokenId?: string; outcome?: string } =>
      focusState
        ? (JSON.parse(focusState()) as { tokenId?: string; outcome?: string })
        : {},
    camera: (): { x: number; y: number; scale: number } | null =>
      cameraState
        ? (JSON.parse(cameraState()) as { x: number; y: number; scale: number })
        : null,
    // Spec 067: the condition markers this canvas draws, by token. A token
    // with none — or one this viewer was never sent — is absent.
    tokenConditions: (): {
      tokenId: string;
      conditions: { id: string; glyph: string; color: string }[];
    }[] =>
      tokenConditions
        ? (JSON.parse(tokenConditions()) as {
            tokenId: string;
            conditions: { id: string; glyph: string; color: string }[];
          }[])
        : [],
    // Playtest 2026-09-10 P7: the names this canvas draws — what a hidden
    // name's test has to ask a player's engine, rather than infer from pixels.
    nameplates: (): { tokenId: string; text: string; dimmed: boolean }[] =>
      tokenNameplates
        ? (JSON.parse(tokenNameplates()) as {
            tokenId: string;
            text: string;
            dimmed: boolean;
          }[])
        : [],
    // Playtest 2026-09-10 P9: the tokens this canvas hides — out of a
    // player's sight, or in the dark.
    hiddenTokens: (): string[] =>
      hiddenTokens ? (JSON.parse(hiddenTokens()) as string[]) : [],
    // Spec 045 T066: the tokens this canvas draws dimly — in dim light, or
    // made out in the dark by darkvision. Shown, but only as a suggestion.
    dimTokens: (): string[] =>
      dimTokens ? (JSON.parse(dimTokens()) as string[]) : [],
    // Spec 045 FR-033: the same question from the Game Master's chair — which
    // tokens are marked because at least one player cannot see them. Always
    // empty on a player's board.
    markedTokens: (): string[] =>
      markedTokens ? (JSON.parse(markedTokens()) as string[]) : [],
    // Spec 045 US6: how far each token sees in darkness, in world units.
    // The only honest end of the chain — a system declares darkvision, the
    // server resolves it from a sheet, the web hands it over, and this says
    // what the engine actually received.
    tokenVision: (tokenId: string): number | null => {
      if (!tokenVision) {
        return null;
      }
      const all = JSON.parse(tokenVision()) as Record<string, number>;
      return all[tokenId] ?? null;
    },
    // Spec 045 T065: the lights tokens carry, and where this engine is
    // lighting each from this frame — its token's position, if it has one.
    carriedLights: (): {
      tokenId: string;
      x: number;
      y: number;
      bright: number;
      dim: number;
    }[] =>
      carriedLights
        ? (JSON.parse(carriedLights()) as {
            tokenId: string;
            x: number;
            y: number;
            bright: number;
            dim: number;
          }[])
        : [],
    // Spec 045 FR-061: every placed light's bright and dim reach, as this
    // engine lights the board by them — a reach set in the Lights panel read
    // back from each seat. Empty in a lit scene with nothing to light.
    placedLights: (): { lightId: string; bright: number; dim: number }[] =>
      placedLights
        ? (JSON.parse(placedLights()) as {
            lightId: string;
            bright: number;
            dim: number;
          }[])
        : [],
    // Spec 071 US3: the door icons this board draws now, and where — a
    // sprite cannot be read back from the canvas.
    doorIcons: (): {
      wallId: string;
      icon: "open" | "close" | "padlock";
      x: number;
      y: number;
      side: number;
    }[] =>
      doorIcons
        ? (JSON.parse(doorIcons()) as {
            wallId: string;
            icon: "open" | "close" | "padlock";
            x: number;
            y: number;
            side: number;
          }[])
        : [],
    // Spec 045: what this client believes about moving — which token its
    // player may move, whether the engine has found it yet, how many are
    // tagged, and whether the scene has a grid to step on. A keypress that
    // moves nothing has several possible causes; this says which.
    movementState: (): unknown =>
      movementState ? (JSON.parse(movementState()) as unknown) : null,
    // Spec 046 US4: what each token fills as this board draws it — footprint
    // in squares, centre, sprite size, the name's height above the centre and
    // the bars' width, all in world units. Asked of the engine, because a
    // footprint the command carried but the drawing ignored is the bug.
    tokenFootprints: (): {
      tokenId: string;
      footprint: number;
      x: number;
      y: number;
      width: number;
      height: number;
      nameY: number | null;
      barWidth: number | null;
    }[] =>
      tokenFootprints
        ? (JSON.parse(tokenFootprints()) as {
            tokenId: string;
            footprint: number;
            x: number;
            y: number;
            width: number;
            height: number;
            nameY: number | null;
            barWidth: number | null;
          }[])
        : [],
  };
}
