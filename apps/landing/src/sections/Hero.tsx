import {
  useCallback,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent,
} from "react";
import { createHero, createMonster, monsterPack, PRESET_HEROES } from "@thunderforge/heroes";
import { Nav } from "../Nav.tsx";
import { DEMO, REPO, SPONSORS } from "../links.ts";
import { ArrowMark, HeartMark, StarMark } from "../Icons.tsx";
import { stroke } from "../ink/marker.ts";

// The first viewport is a stretch of battle mat. The room is drawn in marker
// around the headline, snapped outward to the printed grid; the party waits
// outside its door. Tokens drag and snap like they do at the table, and the
// walls block them the way the server's walls do: a move whose straight line
// crosses a wall is refused.

interface Piece {
  id: string;
  name: string;
  src: string;
  size: number;
  foe: boolean;
}

interface Spot {
  c: number;
  r: number;
}

interface Seg {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

interface Layout {
  cell: number;
  cols: number;
  rows: number;
  wide: boolean;
  walls: Seg[];
  door: { hinge: { x: number; y: number }; leaf: { x: number; y: number }; arc: string };
  spots: Record<string, Spot>;
  note: { x: number; y: number; w: number };
}

const FEET_PER_SQUARE = 5;
const PARTY = ["sir-pip", "mira-starweave", "fenna-swiftbow", "brother-oak"];

function svgSrc(svg: string) {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

function usePieces(): Piece[] {
  return useMemo(() => {
    const heroes = PARTY.map((slug) => PRESET_HEROES.find((p) => p.slug === slug))
      .filter((p) => p !== undefined)
      .map((p) => {
        const hero = createHero(p.spec);
        return {
          id: p.slug,
          name: p.spec.name ?? p.slug,
          src: svgSrc(hero.token()),
          size: 1,
          foe: false,
        };
      });
    const goblins = monsterPack({ name: "Goblin", descriptor: "Small humanoid" }, 3).map(
      (g, i) => ({
        id: `goblin-${i}`,
        name: `Goblin ${i + 1}`,
        src: svgSrc(g.token({ idPrefix: `gob${i}` })),
        size: 1,
        foe: true,
      }),
    );
    const ogre = createMonster({ name: "Ogre", descriptor: "Large giant" });
    return [
      ...heroes,
      ...goblins,
      {
        id: "ogre",
        name: "Ogre (Large, two squares across)",
        src: svgSrc(ogre.token()),
        size: Math.max(1, Math.round(ogre.footprint)),
        foe: true,
      },
    ];
  }, []);
}

function orient(ax: number, ay: number, bx: number, by: number, cx: number, cy: number) {
  return Math.sign((bx - ax) * (cy - ay) - (by - ay) * (cx - ax));
}

function crossesWall(walls: Seg[], x1: number, y1: number, x2: number, y2: number) {
  return walls.some(
    (w) =>
      orient(x1, y1, x2, y2, w.x1, w.y1) !== orient(x1, y1, x2, y2, w.x2, w.y2) &&
      orient(w.x1, w.y1, w.x2, w.y2, x1, y1) !== orient(w.x1, w.y1, w.x2, w.y2, x2, y2),
  );
}

function overlaps(a: Spot, as: number, b: Spot, bs: number) {
  return a.c < b.c + bs && b.c < a.c + as && a.r < b.r + bs && b.r < a.r + as;
}

function measure(
  hero: HTMLElement,
  room: HTMLElement,
  field: HTMLElement,
  pieces: Piece[],
): Layout {
  const cell = parseFloat(getComputedStyle(hero).getPropertyValue("--cell")) || 48;
  const h = hero.getBoundingClientRect();
  const rm = room.getBoundingClientRect();
  const fd = field.getBoundingClientRect();
  const cols = Math.floor(h.width / cell);
  const rows = Math.floor(h.height / cell);
  const wide = h.width >= 900;

  // The room, snapped outward to grid lines.
  const x0 = Math.floor((rm.left - h.left) / cell);
  const y0 = Math.floor((rm.top - h.top) / cell);
  const x1 = Math.ceil((rm.right - h.left) / cell);
  const y1 = Math.ceil((rm.bottom - h.top) / cell);
  const P = (n: number) => n * cell;

  const walls: Seg[] = [];
  let door: Layout["door"];
  const leafLen = P(2);
  if (wide) {
    // A double door low in the east wall, swung out toward the party.
    const d0 = Math.max(y0 + 1, y1 - 4);
    walls.push(
      { x1: P(x0), y1: P(y0), x2: P(x1), y2: P(y0) },
      { x1: P(x0), y1: P(y0), x2: P(x0), y2: P(y1) },
      { x1: P(x0), y1: P(y1), x2: P(x1), y2: P(y1) },
      { x1: P(x1), y1: P(y0), x2: P(x1), y2: P(d0) },
      { x1: P(x1), y1: P(d0 + 2), x2: P(x1), y2: P(y1) },
    );
    const a = (-38 * Math.PI) / 180;
    const hinge = { x: P(x1), y: P(d0 + 2) };
    const leaf = { x: hinge.x + Math.sin(-a) * leafLen, y: hinge.y - Math.cos(a) * leafLen };
    door = {
      hinge,
      leaf,
      arc: `M${hinge.x} ${hinge.y - leafLen} A${leafLen} ${leafLen} 0 0 1 ${leaf.x.toFixed(1)} ${leaf.y.toFixed(1)}`,
    };
  } else {
    // On a phone the party waits below, so the door is in the south wall.
    const d0 = Math.min(x1 - 3, x0 + 2);
    walls.push(
      { x1: P(x0), y1: P(y0), x2: P(x1), y2: P(y0) },
      { x1: P(x0), y1: P(y0), x2: P(x0), y2: P(y1) },
      { x1: P(x1), y1: P(y0), x2: P(x1), y2: P(y1) },
      { x1: P(x0), y1: P(y1), x2: P(d0), y2: P(y1) },
      { x1: P(d0 + 2), y1: P(y1), x2: P(x1), y2: P(y1) },
    );
    const a = (38 * Math.PI) / 180;
    const hinge = { x: P(d0 + 2), y: P(y1) };
    const leaf = { x: hinge.x - Math.cos(a) * leafLen, y: hinge.y + Math.sin(a) * leafLen };
    door = {
      hinge,
      leaf,
      arc: `M${hinge.x - leafLen} ${hinge.y} A${leafLen} ${leafLen} 0 0 0 ${leaf.x.toFixed(1)} ${leaf.y.toFixed(1)}`,
    };
  }

  // The party and the foes, laid out in the field beside (or below) the room.
  const fx = Math.ceil((fd.left - h.left) / cell);
  const fy = Math.ceil((fd.top - h.top) / cell);
  const fw = Math.floor((fd.right - h.left) / cell) - fx;
  const spots: Record<string, Spot> = {};
  const heroes = pieces.filter((p) => !p.foe);
  const foes = pieces.filter((p) => p.foe);
  let note: Layout["note"];
  if (wide) {
    const base = Math.max(y1 - 4, fy + 3);
    heroes.forEach((p, i) => {
      spots[p.id] = { c: x1 + 1 + (i % 2), r: base + Math.floor(i / 2) + (i % 2) };
    });
    const fc = Math.min(cols - 4, x1 + 5);
    const fr = Math.max(fy, base - 4);
    foes.forEach((p, i) => {
      spots[p.id] =
        p.size > 1 ? { c: fc + 1, r: fr + 3 } : { c: fc + (i % 3), r: fr + (i === 1 ? 1 : 0) };
    });
    note = { x: P(x1 + 1), y: P(base - 3), w: P(Math.max(4, fc - x1 - 1)) };
  } else {
    const base = y1 + 1;
    heroes.forEach((p, i) => {
      spots[p.id] = { c: x0 + (i % 2) + 1, r: base + 1 + Math.floor(i / 2) };
    });
    const fc = Math.min(fx + fw - 4, x0 + 6);
    foes.forEach((p, i) => {
      spots[p.id] = p.size > 1 ? { c: fc + 1, r: base + 2 } : { c: fc + i, r: base };
    });
    note = { x: P(x0 + 1), y: P(base + 4) + cell * 0.25, w: P(Math.max(6, x1 - x0 - 2)) };
  }
  return { cell, cols, rows, wide, walls, door, spots, note };
}

interface Drag {
  id: string;
  from: Spot;
  x: number;
  y: number;
  dx: number;
  dy: number;
}

export function Hero() {
  const pieces = usePieces();
  const heroRef = useRef<HTMLElement>(null);
  const roomRef = useRef<HTMLDivElement>(null);
  const fieldRef = useRef<HTMLDivElement>(null);
  const [layout, setLayout] = useState<Layout | null>(null);
  const [spots, setSpots] = useState<Record<string, Spot>>({});
  const [drag, setDrag] = useState<Drag | null>(null);
  const [said, setSaid] = useState("");
  const layoutKey = useRef("");

  useLayoutEffect(() => {
    const hero = heroRef.current;
    const room = roomRef.current;
    const field = fieldRef.current;
    if (!hero || !room || !field) return;
    const update = () => {
      const next = measure(hero, room, field, pieces);
      setLayout(next);
      // A new shape of mat sets the pieces out again; a same-shaped redraw
      // leaves them where the visitor pushed them.
      const key = `${next.cell}|${next.cols}|${next.wide}|${next.walls.map((w) => w.y2).join()}`;
      if (key !== layoutKey.current) {
        layoutKey.current = key;
        setSpots(next.spots);
      }
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(hero);
    ro.observe(room);
    void document.fonts?.ready.then(update);
    return () => ro.disconnect();
  }, [pieces]);

  const sizeOf = useCallback((id: string) => pieces.find((p) => p.id === id)?.size ?? 1, [pieces]);

  const verdict = useCallback(
    (id: string, from: Spot, to: Spot): "ok" | "wall" | "taken" | "edge" => {
      if (!layout) return "edge";
      const s = sizeOf(id);
      if (to.c < 0 || to.r < 2 || to.c + s > layout.cols || to.r + s > layout.rows) return "edge";
      for (const other of pieces) {
        if (other.id === id) continue;
        const at = spots[other.id];
        if (at && overlaps(to, s, at, other.size)) return "taken";
      }
      const half = (s * layout.cell) / 2;
      const c = layout.cell;
      if (
        crossesWall(
          layout.walls,
          from.c * c + half,
          from.r * c + half,
          to.c * c + half,
          to.r * c + half,
        )
      )
        return "wall";
      return "ok";
    },
    [layout, pieces, sizeOf, spots],
  );

  const target = useMemo(() => {
    if (!drag || !layout) return null;
    const to = {
      c: Math.round(drag.x / layout.cell),
      r: Math.round(drag.y / layout.cell),
    };
    return { to, verdict: verdict(drag.id, drag.from, to) };
  }, [drag, layout, verdict]);

  function nameOf(id: string) {
    return pieces.find((p) => p.id === id)?.name ?? id;
  }

  function settle(id: string, from: Spot, to: Spot) {
    const v = verdict(id, from, to);
    const feet = Math.max(Math.abs(to.c - from.c), Math.abs(to.r - from.r)) * FEET_PER_SQUARE;
    if (v === "ok") {
      if (feet > 0) {
        setSpots((s) => ({ ...s, [id]: to }));
        setSaid(`${nameOf(id)} moved ${feet} feet.`);
      }
    } else {
      setSaid(
        v === "wall"
          ? `${nameOf(id)} can't move there: a wall is in the way.`
          : v === "taken"
            ? `${nameOf(id)} can't move there: that square is taken.`
            : `${nameOf(id)} can't leave the mat.`,
      );
    }
  }

  function onPointerDown(e: PointerEvent<HTMLButtonElement>, id: string) {
    if (!layout || e.button !== 0) return;
    const at = spots[id];
    if (!at) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    const hero = heroRef.current!.getBoundingClientRect();
    const x = at.c * layout.cell;
    const y = at.r * layout.cell;
    setDrag({ id, from: at, x, y, dx: e.clientX - hero.left - x, dy: e.clientY - hero.top - y });
  }

  function onPointerMove(e: PointerEvent<HTMLButtonElement>) {
    if (!drag) return;
    const hero = heroRef.current!.getBoundingClientRect();
    setDrag({ ...drag, x: e.clientX - hero.left - drag.dx, y: e.clientY - hero.top - drag.dy });
  }

  function onPointerUp() {
    if (!drag || !target) return setDrag(null);
    settle(drag.id, drag.from, target.to);
    setDrag(null);
  }

  function onKeyDown(e: KeyboardEvent<HTMLButtonElement>, id: string) {
    const step: Record<string, [number, number]> = {
      ArrowUp: [0, -1],
      ArrowDown: [0, 1],
      ArrowLeft: [-1, 0],
      ArrowRight: [1, 0],
    };
    const d = step[e.key];
    const at = spots[id];
    if (!d || !at) return;
    e.preventDefault();
    settle(id, at, { c: at.c + d[0], r: at.r + d[1] });
  }

  const c = layout?.cell ?? 48;
  const ruler =
    drag && target && layout
      ? (() => {
          const s = sizeOf(drag.id);
          const half = (s * c) / 2;
          const ax = drag.from.c * c + half;
          const ay = drag.from.r * c + half;
          const bx = target.to.c * c + half;
          const by = target.to.r * c + half;
          const feet =
            Math.max(Math.abs(target.to.c - drag.from.c), Math.abs(target.to.r - drag.from.r)) *
            FEET_PER_SQUARE;
          return { ax, ay, bx, by, feet, s };
        })()
      : null;

  return (
    <header className="hero" ref={heroRef}>
      <Nav />
      <div className="hero-body">
        <div className="room" ref={roomRef}>
          <h1>
            Get back to the <span className="ink-blue">fantasy.</span>
          </h1>
          <p className="offer">
            ThunderForge is a free, open source virtual tabletop you host yourself.{" "}
            <span className="offer-more">
              Built for tables that meet in person, online, or both at once, and built to stay out
              of the story's way.
            </span>
          </p>
          <div className="actions">
            <a className="ink-btn ink-btn--blue" href={DEMO}>
              Play the demo world
              <ArrowMark />
            </a>
            <div className="actions-pair">
              <a className="ink-btn ink-btn--red-line" href={SPONSORS}>
                <HeartMark />
                Donate
              </a>
              <a className="ink-btn ink-btn--line" href={REPO}>
                <StarMark />
                Star<span className="wide-only"> on GitHub</span>
              </a>
            </div>
          </div>
          <p className="aside">
            Not released yet. Built in the open, every day, by one person who loves this hobby.
          </p>
        </div>
        <div className="field" ref={fieldRef} />
      </div>

      {layout && (
        <svg
          className="mat-ink"
          width="100%"
          height="100%"
          aria-hidden="true"
          style={{ ["--cell" as string]: `${c}px` }}
        >
          <g className="walls">
            {layout.walls.map((w, i) => (
              <path
                key={i}
                pathLength={1}
                style={{ ["--i" as string]: i }}
                className="marker marker--wall"
                d={stroke(w.x1, w.y1, w.x2, w.y2, i + 7, { overshoot: 6, bow: 3 })}
              />
            ))}
            <path
              pathLength={1}
              style={{ ["--i" as string]: layout.walls.length }}
              className="marker marker--door"
              d={stroke(
                layout.door.hinge.x,
                layout.door.hinge.y,
                layout.door.leaf.x,
                layout.door.leaf.y,
                41,
                { overshoot: 0, bow: 1 },
              )}
            />
            <path
              pathLength={1}
              style={{ ["--i" as string]: layout.walls.length + 1 }}
              className="marker marker--swing"
              d={layout.door.arc}
            />
          </g>
          {ruler && target && (
            <g className={`ruler ruler--${target.verdict}`}>
              <rect
                x={target.to.c * c + 2}
                y={target.to.r * c + 2}
                width={ruler.s * c - 4}
                height={ruler.s * c - 4}
                rx={4}
                className="marker marker--thin"
              />
              <path d={`M${ruler.ax} ${ruler.ay} L${ruler.bx} ${ruler.by}`} className="marker marker--thin" />
              <circle cx={ruler.ax} cy={ruler.ay} r={4} />
            </g>
          )}
        </svg>
      )}

      {layout && (
        <p
          className="drag-note"
          style={{ left: layout.note.x, top: layout.note.y, maxWidth: layout.note.w }}
        >
          Drag a token. It snaps to the squares, and the walls block it. Use the door.
        </p>
      )}

      {layout &&
        pieces.map((p) => {
          const at = spots[p.id];
          if (!at) return null;
          const dragging = drag?.id === p.id;
          const x = dragging ? drag.x : at.c * c;
          const y = dragging ? drag.y : at.r * c;
          return (
            <button
              key={p.id}
              type="button"
              className={`token${p.foe ? " token--foe" : ""}${dragging ? " is-dragging" : ""}`}
              style={{
                width: p.size * c,
                height: p.size * c,
                transform: `translate(${x}px, ${y}px)`,
              }}
              aria-label={`${p.name}. Arrow keys move one square.`}
              onPointerDown={(e) => onPointerDown(e, p.id)}
              onPointerMove={onPointerMove}
              onPointerUp={onPointerUp}
              onPointerCancel={() => setDrag(null)}
              onKeyDown={(e) => onKeyDown(e, p.id)}
            >
              <img src={p.src} alt="" draggable={false} />
              <span className="token-name" aria-hidden="true">
                {p.name.split(" (")[0]}
              </span>
            </button>
          );
        })}

      {ruler && target && (
        <span
          className={`ruler-label ruler-label--${target.verdict}`}
          style={{ left: ruler.bx, top: target.to.r * c - 8 }}
          aria-hidden="true"
        >
          {target.verdict === "wall"
            ? "wall"
            : target.verdict === "taken"
              ? "taken"
              : target.verdict === "edge"
                ? "off the mat"
                : `${ruler.feet} ft`}
        </span>
      )}

      <p className="sr-only" aria-live="polite">
        {said}
      </p>
    </header>
  );
}
