import { useLayoutEffect, useRef, useState } from "react";
import { CC_BY_SA, MAPS_CATALOG, MAPS_SOURCE } from "../links.ts";
import { publishMat, type MatPiece } from "../table/heroMat.ts";

// A wide page opens in the tavern, beside the headline: every adventure
// starts in one. The map lies on the page's own grid, one square to a cell,
// with the party inside. While the page holds the table, `PageTable` draws
// the map and the party on the engine and these pictures step aside; the
// hero walks out of the back door and down the page.

const MAP = "/maps/tavern-sample.webp";
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
/** The back door, in the south wall: the way out to the page. */
const EXIT = { c: 5.5, r: 7.5 };

export interface TavernPiece {
  id: string;
  name: string;
  src: string;
}

export function HeroTavern({ party }: { party: TavernPiece[] }) {
  const boxRef = useRef<HTMLDivElement>(null);
  const mapRef = useRef<HTMLDivElement>(null);
  const [cell, setCell] = useState(0);

  // The map snaps to the page's grid lines, then says where it lies.
  useLayoutEffect(() => {
    const box = boxRef.current;
    const el = mapRef.current;
    if (!box || !el) return;
    const update = () => {
      const size = parseFloat(getComputedStyle(el).getPropertyValue("--cell")) || 48;
      box.style.translate = "";
      const r = el.getBoundingClientRect();
      const left = r.left + window.scrollX;
      const top = r.top + window.scrollY;
      const x = Math.round(left / size) * size;
      const y = Math.round(top / size) * size;
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
      publishMat({
        walls: [],
        door: null,
        doorOpen: false,
        pieces,
        map: { src: MAP, x, y, w: COLS * size, h: ROWS * size },
        exit: { x: x + EXIT.c * size, y: y + EXIT.r * size },
      });
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(document.body);
    void document.fonts?.ready.then(update);
    return () => ro.disconnect();
  }, [party]);

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
      </div>
      <p className="hero-tavern-credit">
        Every adventure starts in a tavern. Map by MBRound18 (<a href={MAPS_SOURCE}>vtt-maps</a>,{" "}
        <a href={MAPS_CATALOG}>catalog</a>), <a href={CC_BY_SA}>CC BY-SA 4.0</a>.
      </p>
    </div>
  );
}
