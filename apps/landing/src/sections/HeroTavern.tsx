import { useLayoutEffect, useRef, useState } from "react";
import { CC_BY_SA, MAPS_CATALOG, MAPS_SOURCE } from "../links.ts";
import { publishMat, type MatPiece, type MatSeg } from "../table/heroMat.ts";

// The page opens in the tavern, beside the headline or under it: every
// adventure starts in one. The map lies on the page's own grid, one square to a cell,
// with the party inside. While the page holds the table, `PageTable` draws
// the map and the party on the engine and these pictures step aside; the
// hero walks out of the back door and down the page.

const MAP = "/maps/tavern-sample.webp";
/** `styles.css`: under this the map shrinks to fit the screen. */
const PHONE = "(max-width: 559px)";
/** `styles.css`: under this the credit runs below the map, under the goblins. */
const STACKED = "(max-width: 1199px)";
/** The map's size in squares (`tavern-sample.json`: 1408×1024 at 128). */
const COLS = 11;
const ROWS = 8;
/** Where the party stands, top-left squares. By the bar, by the tables. */
const SPOTS = [
  { c: 4, r: 5 },
  { c: 4, r: 2 },
  { c: 7, r: 2 },
  { c: 8, r: 5 },
];
/** Three goblins on the grass below, either side of the hero's way down
 * from the back door (spec 098 US2). Squares from the map's corner. */
const FOE_SPOTS = [
  { c: 4, r: 10 },
  { c: 6, r: 12 },
  { c: 4, r: 14 },
];
/** The back door, in the south wall: the way out to the page. */
const EXIT = { c: 5.5, r: 7.5 };

/** A wall of the map's own (`tavern-sample.json`): map pixels, centred, +y up. */
interface MapWall {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  doorState: string;
  perimeter?: boolean;
}

/** The tavern's walls, fetched once: the engine plans the hero's way round
 * them (spec 098 FR-005). */
let mapWalls: Promise<MapWall[]> | null = null;
const wallsOfMap = () =>
  (mapWalls ??= fetch(MAP.replace(".webp", ".json"))
    .then((r) => (r.ok ? r.json() : { walls: [] }))
    .then((m: { walls?: MapWall[] }) => (m.walls ?? []).filter((w) => !w.perimeter))
    .catch(() => []));

export interface TavernPiece {
  id: string;
  name: string;
  src: string;
}

export function HeroTavern({ party, foes = [] }: { party: TavernPiece[]; foes?: TavernPiece[] }) {
  const boxRef = useRef<HTMLDivElement>(null);
  const mapRef = useRef<HTMLDivElement>(null);
  const [cell, setCell] = useState(0);
  const [below, setBelow] = useState(0);

  // The map snaps to the page's grid lines, then says where it lies.
  useLayoutEffect(() => {
    const box = boxRef.current;
    const el = mapRef.current;
    if (!box || !el) return;
    let walls: MapWall[] = [];
    const update = () => {
      box.style.translate = "";
      const r = el.getBoundingClientRect();
      // A square is a page cell, or less on a phone, where the map fits
      // between the gutters and lies where it falls.
      const size = r.width / COLS || 48;
      const snap = !matchMedia(PHONE).matches;
      // Stacked, the credit wraps under the map: the goblins stand a row
      // lower, clear of it.
      const below = matchMedia(STACKED).matches ? 1 : 0;
      setBelow(below);
      const left = r.left + window.scrollX;
      const top = r.top + window.scrollY;
      const x = snap ? Math.round(left / size) * size : left;
      const y = snap ? Math.round(top / size) * size : top;
      // The credit moves with it.
      box.style.translate = `${x - left}px ${y - top}px`;
      setCell(size);
      const pieces: MatPiece[] = party.slice(0, SPOTS.length).map((p, i) => ({
        id: p.id,
        name: p.name.split(" (")[0]!,
        src: p.src,
        x: x + SPOTS[i]!.c * size,
        y: y + SPOTS[i]!.r * size,
        side: size,
        foe: false,
      }));
      foes.slice(0, FOE_SPOTS.length).forEach((p, i) =>
        pieces.push({
          id: p.id,
          name: p.name,
          src: p.src,
          x: x + FOE_SPOTS[i]!.c * size,
          y: y + (FOE_SPOTS[i]!.r + below) * size,
          side: size,
          foe: true,
        }),
      );
      // The map's pixels to the page's: 128 to a square there, `size` here.
      const seg = (w: MapWall): MatSeg => ({
        x1: x + (w.x1 / 128 + COLS / 2) * size,
        y1: y + (ROWS / 2 - w.y1 / 128) * size,
        x2: x + (w.x2 / 128 + COLS / 2) * size,
        y2: y + (ROWS / 2 - w.y2 / 128) * size,
      });
      // The south door stands open: the hero leaves by it.
      const south = (w: MapWall) => w.y1 < 0 && w.y2 < 0;
      publishMat({
        walls: walls.filter((w) => w.doorState === "none").map(seg),
        doors: walls.filter((w) => w.doorState !== "none").map((w) => ({ seg: seg(w), open: south(w) })),
        door: null,
        doorOpen: false,
        pieces,
        map: { src: MAP, x, y, w: COLS * size, h: ROWS * size },
        exit: { x: x + EXIT.c * size, y: y + EXIT.r * size },
      });
    };
    update();
    let gone = false;
    void wallsOfMap().then((w) => {
      walls = w;
      if (!gone) update();
    });
    const ro = new ResizeObserver(update);
    ro.observe(document.body);
    void document.fonts?.ready.then(update);
    return () => {
      gone = true;
      ro.disconnect();
    };
  }, [party, foes]);

  return (
    <div className="hero-tavern" ref={boxRef}>
      <div className="hero-tavern-map" ref={mapRef} data-testid="hero-tavern">
        <img className="hero-tavern-art" src={MAP} alt="" width={1408} height={1024} />
        {cell > 0 &&
          party.slice(0, SPOTS.length).map((p, i) => (
            <img
              key={p.id}
              className="hero-tavern-token"
              src={p.src}
              alt={p.name}
              style={{ left: SPOTS[i]!.c * cell, top: SPOTS[i]!.r * cell, width: cell, height: cell }}
            />
          ))}
        {cell > 0 &&
          foes.slice(0, FOE_SPOTS.length).map((p, i) => (
            <img
              key={p.id}
              className="hero-tavern-token"
              src={p.src}
              alt={p.name}
              style={{
                left: FOE_SPOTS[i]!.c * cell,
                top: (FOE_SPOTS[i]!.r + below) * cell,
                width: cell,
                height: cell,
              }}
            />
          ))}
      </div>
      <p className="hero-tavern-credit">
        Every adventure starts in a tavern. Map by MBRound18 (<a href={MAPS_SOURCE}>vtt-maps</a>,{" "}
        <a href={MAPS_CATALOG}>catalog</a>), <a href={CC_BY_SA}>CC BY-SA 4.0</a>.
      </p>
    </div>
  );
}
