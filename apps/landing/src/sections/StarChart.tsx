import { useEffect, useMemo, useRef, useState } from "react";
import { avatarAt, fetchSky, type Person, type Sky } from "../github.ts";
import { stroke } from "../ink/marker.ts";
import { StarMark } from "../Icons.tsx";
import { REPO } from "../links.ts";
import { useStars } from "../useStars.ts";

// Everyone who starred the repository, drawn as a star chart on the mat:
// a small star for each stargazer, a big one for each person who built it.
// Each person keeps their square from visit to visit (it comes from their
// login), and each new star is joined to the nearest one already in the sky.

type Grid = { cell: number; cols: number; offX: number; offY: number };
type Placed = Person & { c: number; r: number; big: boolean; seed: number };
type Chart = { placed: Placed[]; links: [Placed, Placed][]; rows: number };

/** Join a new star to its nearest neighbour only when it is this close (in squares). */
const REACH = 6;

function hash(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

function tryPlace(big: Person[], small: Person[], cols: number, rows: number): Placed[] | null {
  const taken = new Uint8Array(cols * rows);
  const total = cols * rows;
  const placed: Placed[] = [];
  const free = (c: number, r: number, k: number) => {
    for (let y = r - k; y <= r + k; y++)
      for (let x = c - k; x <= c + k; x++)
        if (x >= 0 && x < cols && y >= 0 && y < rows && taken[y * cols + x]) return false;
    return true;
  };
  const put = (p: Person, isBig: boolean): boolean => {
    // A big star covers 3 x 3 squares, a small one 1; each keeps one clear
    // square around it so no two stars touch.
    const k = isBig ? 1 : 0;
    const h = hash(p.login);
    for (let i = 0; i < total; i++) {
      const idx = (h + i * 7919) % total;
      const c = idx % cols;
      const r = Math.floor(idx / cols);
      if (c - k < 0 || c + k >= cols || r - k < 0 || r + k >= rows) continue;
      if (!free(c, r, k + 1)) continue;
      for (let y = r - k; y <= r + k; y++) for (let x = c - k; x <= c + k; x++) taken[y * cols + x] = 1;
      placed.push({ ...p, c, r, big: isBig, seed: h });
      return true;
    }
    return false;
  };
  for (const p of big) if (!put(p, true)) return null;
  for (const p of small) if (!put(p, false)) return null;
  return placed;
}

function chart(sky: Sky, cols: number): Chart | null {
  const big = sky.contributors ?? [];
  const builders = new Set(big.map((p) => p.login));
  const small = (sky.stargazers ?? []).filter((p) => !builders.has(p.login));
  if (big.length + small.length === 0 || cols < 3) return null;

  // Room for every star with its clear ring, then grow until they all fit.
  const need = small.length * 4 + big.length * 16;
  let rows = Math.max(big.length ? 5 : 3, Math.ceil(need / cols) + 2);
  let placed = tryPlace(big, small, cols, rows);
  while (!placed) {
    rows += 2;
    placed = tryPlace(big, small, cols, rows);
  }

  const links: [Placed, Placed][] = [];
  const bigs = placed.filter((p) => p.big);
  for (let i = 1; i < bigs.length; i++) links.push([bigs[i - 1], bigs[i]]);
  for (let i = bigs.length; i < placed.length; i++) {
    const a = placed[i];
    let best: Placed | null = null;
    let bestD = Infinity;
    for (let j = 0; j < i; j++) {
      const b = placed[j];
      const d = Math.hypot(a.c - b.c, a.r - b.r);
      if (d < bestD) {
        bestD = d;
        best = b;
      }
    }
    if (best && bestD <= REACH) links.push([a, best]);
  }
  return { placed, links, rows };
}

/** A hand-drawn five-pointed star: ten marker strokes, point to notch. */
function starPath(cx: number, cy: number, R: number, seed: number): string {
  const pts: [number, number][] = [];
  for (let i = 0; i < 10; i++) {
    const a = -Math.PI / 2 + (i * Math.PI) / 5;
    const rad = i % 2 === 0 ? R : R * 0.46;
    pts.push([cx + Math.cos(a) * rad, cy + Math.sin(a) * rad]);
  }
  const lift = Math.max(0.6, R * 0.04);
  return pts
    .map(([x, y], i) => {
      const [nx, ny] = pts[(i + 1) % pts.length];
      return stroke(x, y, nx, ny, seed + i, { overshoot: lift, bow: lift });
    })
    .join(" ");
}

function measure(field: HTMLElement): Grid {
  const cell = parseFloat(getComputedStyle(field).getPropertyValue("--cell")) || 48;
  const rect = field.getBoundingClientRect();
  const left = rect.left + window.scrollX;
  const top = rect.top + window.scrollY;
  // The mat's grid starts at the page origin; land every star on a square.
  const offX = (cell - (left % cell)) % cell;
  const offY = (cell - (top % cell)) % cell;
  return { cell, cols: Math.floor((rect.width - offX) / cell), offX, offY };
}

function Star({ p, g }: { p: Placed; g: Grid }) {
  const { cell } = g;
  const cx = g.offX + (p.c + 0.5) * cell;
  const cy = g.offY + (p.r + 0.5) * cell;
  const R = p.big ? cell * 1.45 : cell * 0.5;
  const a = p.big ? cell * 0.72 : cell * 0.27;
  // Keep the name on the chart: a rough width from its length is enough to
  // pull it in from either edge.
  const half = (p.login.length * (p.big ? 11 : 8)) / 2;
  const right = g.offX + g.cols * cell;
  const nameX = Math.min(Math.max(cx, half), right - half);
  // Below the star's lowest points (cos 36° of its radius).
  const nameY = cy + (p.big ? R * 0.81 + cell * 0.5 : a + cell * 0.42);
  return (
    <a
      href={p.url}
      className={p.big ? "sc-star sc-star--big" : "sc-star"}
      aria-label={p.big ? `${p.login}, who helped build ThunderForge` : `${p.login}, who starred ThunderForge`}
    >
      <path
        className={p.big ? "marker sc-point" : "marker marker--thin sc-point"}
        d={starPath(cx, cy, R, p.seed)}
      />
      <circle className="sc-plate" cx={cx} cy={cy} r={a} />
      <image
        href={avatarAt(p.avatar, a * 2 * 2)}
        x={cx - a}
        y={cy - a}
        width={a * 2}
        height={a * 2}
        clipPath="url(#sc-round)"
        preserveAspectRatio="xMidYMid slice"
      />
      <circle className="sc-ring" cx={cx} cy={cy} r={a} />
      <text className={p.big ? "sc-name sc-name--big" : "sc-name"} x={nameX} y={nameY}>
        {p.login}
      </text>
    </a>
  );
}

export default function StarChart() {
  const stars = useStars();
  const fieldRef = useRef<HTMLDivElement>(null);
  const [sky, setSky] = useState<Sky | null>(null);
  const [grid, setGrid] = useState<Grid | null>(null);

  useEffect(() => {
    let live = true;
    void fetchSky().then((s) => live && setSky(s));
    return () => {
      live = false;
    };
  }, []);

  useEffect(() => {
    const field = fieldRef.current;
    if (!field) return;
    let frame = 0;
    const update = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const next = measure(field);
        setGrid((g) =>
          g && g.cell === next.cell && g.cols === next.cols && g.offX === next.offX && g.offY === next.offY
            ? g
            : next,
        );
      });
    };
    update();
    // The dice and the map above load late and move this section down, so
    // watch the whole page as well as the field.
    const ro = new ResizeObserver(update);
    ro.observe(field);
    ro.observe(document.body);
    return () => {
      cancelAnimationFrame(frame);
      ro.disconnect();
    };
  }, []);

  const drawn = useMemo(() => (sky && grid ? chart(sky, grid.cols) : null), [sky, grid]);
  const lost = sky !== null && sky.stargazers === null;
  const failed = sky !== null && sky.stargazers === null && sky.contributors === null;

  return (
    <section id="stars" className="section stars" aria-labelledby="stars-title">
      <h2 id="stars-title" className="marker-head">
        Written in the stars.
      </h2>
      <p className="lede">
        {stars !== null ? (
          <>
            <span className="stars-count">{stars.toLocaleString("en-US")}</span>{" "}
            {stars === 1 ? "person has" : "people have"} starred ThunderForge so far.{" "}
          </>
        ) : null}
        Every one of them is up there. The big stars are the people who helped build it. Thank you,
        all of you.
      </p>
      <div
        className="star-field"
        ref={fieldRef}
        style={drawn && grid ? { height: grid.offY + drawn.rows * grid.cell } : undefined}
      >
        {drawn && grid && (
          <svg className="star-chart" width="100%" height="100%" role="group" aria-labelledby="stars-title">
            <defs>
              <clipPath id="sc-round" clipPathUnits="objectBoundingBox">
                <circle cx="0.5" cy="0.5" r="0.5" />
              </clipPath>
            </defs>
            <g className="sc-links" aria-hidden="true">
              {drawn.links.map(([a, b]) => (
                <path
                  key={`${a.login}>${b.login}`}
                  className="marker marker--hair"
                  d={stroke(
                    grid.offX + (a.c + 0.5) * grid.cell,
                    grid.offY + (a.r + 0.5) * grid.cell,
                    grid.offX + (b.c + 0.5) * grid.cell,
                    grid.offY + (b.r + 0.5) * grid.cell,
                    a.seed ^ b.seed,
                    { overshoot: -grid.cell * 0.45, bow: 3 },
                  )}
                />
              ))}
            </g>
            {drawn.placed.map((p) => (
              <Star key={p.login} p={p} g={grid} />
            ))}
          </svg>
        )}
      </div>
      {failed ? (
        <p className="stars-note">GitHub didn't answer just now, so the chart is dark. It will be back.</p>
      ) : lost ? (
        <p className="stars-note">GitHub didn't share its list of stargazers just now; the builders are up there.</p>
      ) : null}
      <div className="stars-foot">
        <a className="ink-btn ink-btn--black" href={REPO}>
          <StarMark />
          Add your star
        </a>
        <p className="stars-note">A new star shows up here within about ten minutes.</p>
      </div>
    </section>
  );
}
