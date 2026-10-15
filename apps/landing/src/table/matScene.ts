import type { Send } from "../engine/host.ts";
import { rasterize } from "./heroArt.ts";
import type { Mat, MatPiece } from "./heroMat.ts";
import { toWorld, type Doc, type Pt } from "./journey.ts";

// The first screen's mat on the engine: its walls as walls, its door as a
// door, its pieces as tokens. `Hero.tsx` still rules the moves; this only
// draws them, so a drag on the page is a token sliding on the table.

/** The pixel size a mat token is drawn at: crisp in a cell on a 3x screen. */
const ART_PX = 160;

type Token = Record<string, unknown>;

const svgOf = (src: string) => decodeURIComponent(src.slice(src.indexOf(",") + 1));

/** Where a piece stands, by its centre, in page pixels. */
export const centreOf = (p: MatPiece): Pt => ({ x: p.x + p.side / 2, y: p.y + p.side / 2 });

export class MatScene {
  private walls = new Set<string>();
  /** Each token as it should stand, without its art. */
  private tokens = new Map<string, { token: Token; footprint: number }>();
  /** The tokens on the table. */
  private sent = new Set<string>();
  private art = new Map<string, string>();
  private drawing = new Set<string>();
  /** The pieces that are down (spec 096), by token id. */
  private down = new Set<string>();
  /** The map image on the table, by an id that changes when it moves: the
   * engine places an image once and never moves it. */
  private map: string | null = null;

  /** `live`: the engine runs and the page holds it. While it does not, sends
   * are dropped, so nothing may count as on the table. */
  private send: Send;
  private live: () => boolean;
  /** Its ids' prefix: one scene per map on the page. */
  private kind: string;

  constructor(send: Send, live: () => boolean, kind = "mat") {
    this.send = send;
    this.live = live;
    this.kind = kind;
  }

  /** Sends what changed. `skip`: the piece the page's walker stands in for. */
  draw(mat: Mat, doc: Doc, cell: number, skip: string | null) {
    const walls = mat.walls.map((w, i) => ({ id: `${this.kind}-wall-${i}`, seg: w, doorState: "none" }));
    if (mat.door) walls.push({ id: `${this.kind}-door`, seg: mat.door, doorState: mat.doorOpen ? "open" : "closed" });
    mat.doors?.forEach((d, i) => walls.push({ id: `${this.kind}-door-${i}`, seg: d.seg, doorState: d.open ? "open" : "closed" }));
    const seen = new Set<string>();
    for (const { id, seg, doorState } of walls) {
      const a = toWorld({ x: seg.x1, y: seg.y1 }, doc);
      const b = toWorld({ x: seg.x2, y: seg.y2 }, doc);
      seen.add(id);
      this.send({
        type: "upsert_wall",
        wall: { id, x1: a.x, y1: a.y, x2: b.x, y2: b.y, blocksVision: true, blocksMovement: true, doorState },
      });
    }
    for (const id of this.walls) if (!seen.has(id)) this.send({ type: "remove_wall", wallId: id });
    this.walls = seen;

    const m = mat.map;
    const mapId = m ? `${this.kind}-map-${[m.x, m.y, m.w, m.h, doc.w, doc.h].map(Math.round).join("-")}` : null;
    if (this.map && this.map !== mapId) {
      this.send({ type: "remove_canvas_image_asset", assetId: this.map });
      this.map = null;
    }
    if (m && mapId && !this.map && this.live()) {
      const c = toWorld({ x: m.x + m.w / 2, y: m.y + m.h / 2 }, doc);
      this.send({ type: "upsert_canvas_image_asset", assetId: mapId, path: m.src, x: c.x, y: c.y, width: m.w, height: m.h });
      this.map = mapId;
    }

    const kept = new Set<string>();
    for (const p of mat.pieces) {
      if (p.id === skip) continue;
      const id = `${this.kind}-${p.id}`;
      const at = toWorld(centreOf(p), doc);
      kept.add(id);
      this.tokens.set(id, {
        token: {
          id,
          x: at.x,
          y: at.y,
          z: 5,
          label: p.name,
          tokenType: p.foe ? "npc" : "character",
          ...(p.foe ? { defeated: this.down.has(id) } : {}),
        },
        footprint: p.side / cell,
      });
      if (this.art.has(id)) this.put(id);
      else this.paint(id, p);
    }
    for (const id of this.tokens.keys()) {
      if (kept.has(id)) continue;
      if (this.sent.delete(id)) this.send({ type: "remove_token", tokenId: id });
      this.tokens.delete(id);
    }
  }

  /** The token where it stands now; on first sight with its art, so the
   * table never shows it as a bare square. */
  private put(id: string) {
    const want = this.tokens.get(id);
    if (!want || !this.live()) return;
    if (this.sent.has(id)) {
      this.send({ type: "upsert_token", token: want.token });
      return;
    }
    this.sent.add(id);
    this.send({ type: "upsert_token", token: { ...want.token, photoData: this.art.get(id) } });
    this.send({ type: "set_token_grid", tokenId: id, footprint: want.footprint, snap: false });
  }

  /** The token's art, drawn once; the token goes on the table when it is. */
  private paint(id: string, p: MatPiece) {
    if (this.drawing.has(id)) return;
    this.drawing.add(id);
    void rasterize(svgOf(p.src), ART_PX).then(
      (data) => {
        this.drawing.delete(id);
        this.art.set(id, data);
        this.put(id);
      },
      () => this.drawing.delete(id),
    );
  }

  /** Puts the pieces `ids` (their mat ids) down and every other foe up. */
  setDown(ids: Set<string>) {
    for (const [id, want] of this.tokens) {
      if (!("defeated" in want.token)) continue;
      const down = ids.has(id.slice(this.kind.length + 1));
      if (want.token.defeated === down) continue;
      want.token = { ...want.token, defeated: down };
      if (down) this.down.add(id);
      else this.down.delete(id);
      if (this.art.has(id)) this.put(id);
    }
  }

  /** Takes all of it off the table. */
  clear() {
    for (const id of this.walls) this.send({ type: "remove_wall", wallId: id });
    for (const id of this.sent) this.send({ type: "remove_token", tokenId: id });
    if (this.map) this.send({ type: "remove_canvas_image_asset", assetId: this.map });
    this.map = null;
    this.down.clear();
    this.walls.clear();
    this.tokens.clear();
    this.sent.clear();
  }
}
