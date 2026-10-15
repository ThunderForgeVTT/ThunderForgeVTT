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

  constructor(private send: Send) {}

  /** Sends what changed. `skip`: the piece the page's walker stands in for. */
  draw(mat: Mat, doc: Doc, cell: number, skip: string | null) {
    const walls = mat.walls.map((w, i) => ({ id: `mat-wall-${i}`, seg: w, doorState: "none" }));
    walls.push({ id: "mat-door", seg: mat.door, doorState: mat.doorOpen ? "open" : "closed" });
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

    const kept = new Set<string>();
    for (const p of mat.pieces) {
      if (p.id === skip) continue;
      const id = `mat-${p.id}`;
      const at = toWorld(centreOf(p), doc);
      kept.add(id);
      this.tokens.set(id, {
        token: { id, x: at.x, y: at.y, z: 5, label: p.name, tokenType: p.foe ? "npc" : "character" },
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
    if (!want) return;
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

  /** Takes all of it off the table. */
  clear() {
    for (const id of this.walls) this.send({ type: "remove_wall", wallId: id });
    for (const id of this.sent) this.send({ type: "remove_token", tokenId: id });
    this.walls.clear();
    this.tokens.clear();
    this.sent.clear();
  }
}
