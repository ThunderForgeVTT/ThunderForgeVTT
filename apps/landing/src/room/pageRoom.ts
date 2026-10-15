import { HERO_ID, type EngineEvent } from "@thunderforge/encounter";

// Spec 098 FR-008: the dungeon on the page's own table.
//
// On a wide page the room does not take the canvas: it is drawn where the
// page lays it, on the page's grid, and the hero walking the page is the
// room's hero. The room's referee and its commands stay as they are; this
// translates them on the way to the engine, and the engine's answers on
// the way back, so the referee never knows it is on a page.
//
// Both worlds are y up. A room unit is `k` page units: the room's square
// is the page's cell.

type Command = Record<string, unknown>;
type Pt = { x: number; y: number };

export interface Place {
  /** The room's world origin, in the page's world units. */
  x: number;
  y: number;
  /** Page units per room unit. */
  k: number;
  /** The page's walker, which plays the room's hero. */
  walker: string;
}

const out = (p: Pt, at: Place): Pt => ({ x: at.x + p.x * at.k, y: at.y + p.y * at.k });
// Rounded: the room compares squares exactly, and a scale leaves dust.
const unit = (v: number) => Math.round(v * 1000) / 1000;
const back = (p: Pt, at: Place): Pt => ({ x: unit((p.x - at.x) / at.k), y: unit((p.y - at.y) / at.k) });
const tokenOut = (id: string, at: Place) => (id === HERO_ID ? at.walker : id);
const tokenBack = (id: string | undefined, at: Place) => (id === at.walker ? HERO_ID : id);

/**
 * The room's command as the page sends it; none for what the page owns:
 * its grid, its background, its camera, its scene playing, its walker.
 */
export function toPage(c: Command, at: Place): Command[] {
  switch (c.type) {
    case "set_scene_grid":
    case "set_scene_background":
    case "fit_camera_to":
    case "set_scene_playing":
      return [];
    case "upsert_wall": {
      const w = c.wall as Command & { x1: number; y1: number; x2: number; y2: number };
      const a = out({ x: w.x1, y: w.y1 }, at);
      const b = out({ x: w.x2, y: w.y2 }, at);
      return [{ ...c, wall: { ...w, x1: a.x, y1: a.y, x2: b.x, y2: b.y } }];
    }
    case "upsert_light": {
      const l = c.light as Command & { x: number; y: number; radius: number; brightRadius: number };
      const p = out(l, at);
      const light = { ...l, ...p, radius: l.radius * at.k, brightRadius: l.brightRadius * at.k };
      return [{ ...c, light }];
    }
    case "upsert_token": {
      const t = c.token as Command & { id: string; x: number; y: number };
      return [{ ...c, token: { ...t, ...out(t, at), id: tokenOut(t.id, at) } }];
    }
    case "remove_token":
      // The walker stays on the page when the room comes down.
      return c.tokenId === HERO_ID ? [] : [c];
    case "upsert_interactive": {
      const i = c.interactive as Command & { geometry: Pt & { width: number; height: number } };
      const g = i.geometry;
      const geometry = { ...g, ...out(g, at), width: g.width * at.k, height: g.height * at.k };
      return [{ ...c, interactive: { ...i, geometry } }];
    }
    case "set_carried_light":
      return [
        {
          ...c,
          tokenId: tokenOut(c.tokenId as string, at),
          bright: (c.bright as number) * at.k,
          dim: (c.dim as number) * at.k,
        },
      ];
    case "set_token_vision":
      return [
        {
          ...c,
          tokenId: tokenOut(c.tokenId as string, at),
          darkvision: (c.darkvision as number) * at.k,
        },
      ];
    case "set_controlled_token":
      // The page's hand stays on its walker.
      return [{ ...c, tokenId: at.walker }];
    default:
      return [c];
  }
}

/** The engine's event as the room hears it; none for what is the page's own. */
export function fromPage(e: EngineEvent, at: Place): EngineEvent | null {
  switch (e.type) {
    case "cameraFitted":
      return null;
    case "upsert_token": {
      const token = { ...e.token, ...back(e.token, at), id: tokenBack(e.token.id, at)! };
      return { ...e, token, ...(e.path ? { path: e.path.map((p) => back(p, at)) } : {}) };
    }
    case "movement_blocked":
      return { ...e, tokenId: tokenBack(e.tokenId, at), at: back(e.at, at) };
    case "interactionTriggered":
      return { ...e, tokenId: tokenBack(e.tokenId, at) };
    default:
      return e;
  }
}
