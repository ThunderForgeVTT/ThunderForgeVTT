import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { CC_BY_SA, MAPS_CATALOG, MAPS_SOURCE } from "../links.ts";
import { publishCamp, type CampHero } from "../table/campMat.ts";
import { watchMat } from "../table/heroMat.ts";

// Spec 098 US6: where the walk ends, beside "Pull up a chair". The owner's
// camp map lies on the page's grid, one square to a cell, and the party that
// started in the tavern sits round the fire. While the page holds the
// table, `PageTable` draws it on the engine, these pictures step aside, and
// the visitor's hero walks in to the empty seat on the top log.

const MAP = "/maps/camp-sample.webp";
/** The map's size in squares (`camp-sample.json`: 1024×1024 at 128). */
const SIZE = 8;
/** The log seats, top-left squares, two apart so the names never touch:
 * the top log's end, the right log, and the top log's far end for a party
 * with no one left walking. */
const SEATS = [
  { c: 1, r: 1 },
  { c: 6, r: 3 },
  { c: 6, r: 5 },
  { c: 5, r: 1 },
];
/** The empty seat on the top log, above the fire: the walker's. */
const PARK = { c: 3, r: 1 };
/** The fire's centre, on a grid point. */
const FIRE = { c: 4, r: 4 };

const partyOf = (pieces: Array<CampHero & { foe: boolean }>) =>
  pieces.filter((p) => !p.foe).map(({ id, name, src }) => ({ id, name, src }));

export function Campfire() {
  const boxRef = useRef<HTMLDivElement>(null);
  const mapRef = useRef<HTMLDivElement>(null);
  const [cell, setCell] = useState(0);
  const [party, setParty] = useState<CampHero[]>([]);

  // The party is the tavern's, as its mat says.
  useEffect(
    () =>
      watchMat((m) => {
        const next = partyOf(m.pieces);
        setParty((was) => (was.map((p) => p.id).join() === next.map((p) => p.id).join() ? was : next));
      }),
    [],
  );

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
      box.style.translate = `${x - left}px ${y - top}px`;
      setCell(size);
      const at = (s: { c: number; r: number }) => ({ x: x + s.c * size, y: y + s.r * size });
      publishCamp({
        src: MAP,
        x,
        y,
        w: SIZE * size,
        h: SIZE * size,
        cell: size,
        party,
        seats: SEATS.map(at),
        park: at(PARK),
        fire: at(FIRE),
      });
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(document.body);
    void document.fonts?.ready.then(update);
    return () => {
      ro.disconnect();
      publishCamp(null);
    };
  }, [party]);

  return (
    <div className="camp" ref={boxRef}>
      <div className="camp-map" ref={mapRef} data-testid="camp">
        <img className="camp-art" src={MAP} alt="" width={1024} height={1024} />
        {cell > 0 &&
          party.slice(0, SEATS.length).map((p, i) => (
            <img
              key={p.id}
              className="camp-token"
              src={p.src}
              alt={p.name}
              style={{ left: SEATS[i]!.c * cell, top: SEATS[i]!.r * cell, width: cell, height: cell }}
            />
          ))}
      </div>
      <p className="camp-credit">
        The party makes camp. Map by MBRound18 (<a href={MAPS_SOURCE}>vtt-maps</a>,{" "}
        <a href={MAPS_CATALOG}>catalog</a>), <a href={CC_BY_SA}>CC BY-SA 4.0</a>.
      </p>
    </div>
  );
}
