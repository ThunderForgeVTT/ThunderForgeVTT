/**
 * What a token takes from its character sheet, resolved as the server
 * resolves it from the world's game system.
 *
 * - `tokenGrid`: `graphql/queries/token_grid.rs` over `combat/size.rs`. A
 *   creature fills the squares its system's `combat.sizes` give its size; a
 *   token of one square is omitted.
 * - `tokenVision`: `graphql/queries/token_vision.rs` over
 *   `vision_profiles.rs` and `thunderforge-canvas-core/src/vision_declaration.rs`.
 *   A distance read from the sheet in the system's units becomes cells and
 *   then world units by the scene's grid; ordinary sight is omitted.
 * - `tokenAttributes`: `graphql/queries/token_attributes.rs` over
 *   `canvas-core/src/attributes.rs` and `movement_budget.rs`: the declared
 *   scores and speeds read from the sheet's ability slot. Its `values` (the
 *   system's derived values, `declared_values.rs` and the per-system rules)
 *   are not computed here and answer empty; nothing in the app reads them
 *   from this query.
 *
 * All three are file-system reads of the system's `system.json` on the
 * server; the demo has the one system its world plays, imported.
 */
import system from "../../../../../packs/systems/dnd5e/system.json";
import { demoState, type DemoState, type Row } from "../state";
import { sceneOf, type Handler } from "./common";

type Manifest = Record<string, unknown>;

/** The manifest a system id names, or none. */
function manifestFor(systemId: unknown): Manifest | null {
  return systemId === system.id ? (system as unknown as Manifest) : null;
}

function worldSystem(state: DemoState): unknown {
  return state.world.gameSystemId;
}

/** `{ "x": 3 }` and `{ "x": { "value": 3 } }` say the same. */
function raw(slot: unknown, field: string): unknown {
  if (!slot || typeof slot !== "object") return undefined;
  const value = (slot as Row)[field];
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return (value as Row).value;
  }
  return value;
}

/** The sheet of `actorId` in `systemId`, else whichever it has. */
function sheetOf(
  state: DemoState,
  actorId: string,
  systemId: unknown,
): Row | null {
  const rows = state.systemData.filter((row) => row.actorId === actorId);
  return rows.find((row) => row.gameSystemId === systemId) ?? rows[0] ?? null;
}

/** `combat::manifest::slot_key`: a manifest's slot name as a column. */
function slotName(slot: string): string {
  return slot.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase());
}

/** `combat::size::footprint_from`. */
function footprintFrom(sizes: Row, slot: unknown): number {
  const id = raw(slot, (sizes.source as Row).field as string);
  const category = (sizes.categories as Row[]).find((c) => c.id === id);
  const footprint = category?.footprint as number | undefined;
  return footprint !== undefined && Number.isFinite(footprint) && footprint > 0
    ? footprint
    : 1;
}

function tokensOf(state: DemoState, sceneId: string): Row[] {
  return state.tokens.filter((t) => t.sceneId === sceneId);
}

function footprintOf(state: DemoState, token: Row): number {
  const actorId = token.actorId as string | null;
  if (!actorId) return 1;
  const actor = state.actors.find((a) => a.id === actorId);
  const systemId = actor?.gameSystemId ?? worldSystem(state);
  const sizes = (manifestFor(systemId)?.combat as Row | undefined)?.sizes as
    | Row
    | undefined;
  if (!sizes) return 1;
  const key = slotName((sizes.source as Row).slot as string);
  if (key === "resourceData" && !token.linked) {
    // The one slot a copy holds itself.
    return footprintFrom(sizes, token.systemData);
  }
  return footprintFrom(sizes, sheetOf(state, actorId, systemId)?.[key]);
}

/** `vision_declaration::read_distance`, in the system's units. */
function distance(slot: unknown, source: string): number | null {
  const value = raw(slot, source);
  return typeof value === "number" && Number.isFinite(value) && value >= 0
    ? value
    : null;
}

/** `vision_declaration::resolve_one`, in cells. */
function cellsOf(
  sheet: Row,
  declared: Row | undefined,
  perCell: number,
): number {
  if (!declared) return 0;
  const slot = sheet[(declared.slot as string | undefined) ?? "traitData"];
  const inUnits =
    distance(slot, declared.source as string) ??
    (declared.default as number | undefined) ??
    0;
  return Number.isFinite(inUnits) && inUnits > 0 ? inUnits / perCell : 0;
}

/** `canvas-core::attributes::read_number`: whole numbers only. */
function score(slot: unknown, field: string): number | null {
  const value = raw(slot, field);
  return typeof value === "number" && Number.isInteger(value) ? value : null;
}

/** `movement_budget::read_speed`. */
function speed(slot: unknown, field: string): number | null {
  const value = raw(slot, field);
  return typeof value === "number" && Number.isFinite(value) && value >= 0
    ? value
    : null;
}

/** `abilities` or `movement`, in declaration order (`attributes.rs`). */
function declared(block: unknown): Array<[string, Row]> {
  if (!block || typeof block !== "object") return [];
  return Object.entries(block as Record<string, Row>)
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    .map(([id, entry], index): [string, Row, number] => [
      id,
      entry,
      typeof entry.order === "number" ? entry.order : index,
    ])
    .sort((a, b) => a[2] - b[2])
    .map(([id, entry]) => [id, entry]);
}

export const tokenSheetQueries: Record<string, Handler> = {
  tokenGrid: ({ sceneId }) => {
    const state = demoState();
    sceneOf(state, sceneId);
    return tokensOf(state, sceneId)
      .map((token) => ({
        tokenId: token.tokenId,
        footprint: footprintOf(state, token),
      }))
      .filter((grid) => grid.footprint !== 1)
      .sort((a, b) => String(a.tokenId).localeCompare(String(b.tokenId)));
  },

  tokenVision: ({ sceneId }) => {
    const state = demoState();
    const scene = sceneOf(state, sceneId);
    const vision = manifestFor(worldSystem(state))?.vision as Row | undefined;
    if (!vision) return [];
    const perCell =
      typeof vision.unitsPerCell === "number" && vision.unitsPerCell > 0
        ? vision.unitsPerCell
        : 5;
    const grid = Math.max(1, scene.gridSize as number);
    const carried = vision.carriedLight as Row | undefined;
    const out: Row[] = [];
    for (const token of tokensOf(state, sceneId)) {
      const actorId = token.actorId as string | null;
      const sheet = actorId
        ? state.systemData.find((row) => row.actorId === actorId)
        : null;
      if (!sheet) continue;
      const cells = [
        cellsOf(sheet, vision.darkvision as Row | undefined, perCell),
        cellsOf(sheet, carried?.bright as Row | undefined, perCell),
        cellsOf(sheet, carried?.dim as Row | undefined, perCell),
      ];
      if (cells.every((c) => c <= 0)) continue;
      const [darkvision, carriedBright, carriedDim] = cells.map((c) =>
        c > 0 ? c * grid : 0,
      );
      out.push({
        tokenId: token.tokenId,
        darkvision,
        carriedBright,
        carriedDim,
      });
    }
    return out;
  },

  tokenAttributes: ({ sceneId }) => {
    const state = demoState();
    sceneOf(state, sceneId);
    const manifest = manifestFor(worldSystem(state));
    const abilities = declared(manifest?.abilities);
    const movement = declared(manifest?.movement);
    if (abilities.length === 0 && movement.length === 0) return [];
    const out: Row[] = [];
    for (const token of tokensOf(state, sceneId)) {
      const actorId = token.actorId as string | null;
      const sheet = actorId
        ? state.systemData.find((row) => row.actorId === actorId)
        : null;
      const data = sheet?.abilityData;
      if (!data) continue;
      const attributes = abilities.flatMap(([id, entry]) => {
        const value = score(data, (entry.source as string | undefined) ?? id);
        return value === null
          ? []
          : [
              {
                id,
                label: (entry.label as string | undefined) ?? id,
                abbreviation:
                  (entry.abbreviation as string | undefined) ?? null,
                value,
              },
            ];
      });
      const speeds = movement.flatMap(([id, entry]) => {
        const value =
          speed(data, (entry.source as string | undefined) ?? id) ??
          (entry.default as number | undefined) ??
          null;
        return value === null
          ? []
          : [{ id, label: (entry.label as string | undefined) ?? id, value }];
      });
      if (attributes.length === 0 && speeds.length === 0) continue;
      out.push({ tokenId: token.tokenId, attributes, values: [], speeds });
    }
    return out;
  },
};
