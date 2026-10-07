import { useEffect, useState } from "react";
import { CC_BY_SA, DEMO, MAPS_CATALOG, MAPS_SOURCE } from "../links.ts";

// The demo world's own maps, each with its own walls, doors and lights laid
// over it in marker: the geometry the server enforces, read from the generated
// map catalogue, not traced by hand. Each legend entry toggles its layer, and
// the atlas below picks the map.

interface Wall {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  doorState: string;
}
interface Light {
  x: number;
  y: number;
  radius: number;
  brightRadius: number;
}
interface MapData {
  width: number;
  height: number;
  gridSize: number;
  walls: Wall[];
  lights: Light[];
}
/** One map in the atlas, from `public/maps/index.json`. */
interface Entry {
  name: string;
  title: string;
  width: number;
  height: number;
  walls: number;
  doors: number;
  lights: number;
}

type Layer = "walls" | "doors" | "lights" | "grid";

const LAYERS: { id: Layer; label: string; note: string }[] = [
  { id: "walls", label: "Walls", note: "block movement and sight, for every player at once" },
  { id: "doors", label: "Doors", note: "gaps in a wall that the GM opens and shuts" },
  { id: "lights", label: "Lights", note: "decide what each token can see" },
  { id: "grid", label: "Grid", note: "tokens, walls and drawings snap to it" },
];


function Swatch({ id }: { id: Layer }) {
  return (
    <svg className="swatch" viewBox="0 0 32 20" aria-hidden="true">
      {id === "walls" && <path className="marker" d="M3 10H29" stroke="var(--ink-black)" />}
      {id === "doors" && (
        <>
          <path className="marker" d="M3 10H11M21 10H29" stroke="var(--ink-black)" />
          <path className="marker marker--thin" d="M11 10H21" stroke="var(--ink-red)" />
        </>
      )}
      {id === "lights" && (
        <>
          <circle cx="16" cy="10" r="8" className="marker marker--thin" stroke="var(--ink-blue)" strokeDasharray="2 3" />
          <circle cx="16" cy="10" r="3" fill="var(--ink-blue)" />
        </>
      )}
      {id === "grid" && (
        <path className="marker marker--hair" d="M2 4H30M2 16H30M8 1V19M16 1V19M24 1V19" stroke="var(--ink-green)" />
      )}
    </svg>
  );
}

const geometry = new Map<string, Promise<MapData>>();
function loadGeometry(name: string): Promise<MapData> {
  let p = geometry.get(name);
  if (!p) {
    p = fetch(`/maps/${name}.json`).then((r) => (r.ok ? r.json() : Promise.reject()));
    p.catch(() => geometry.delete(name));
    geometry.set(name, p);
  }
  return p;
}

function tally(e: Entry): string[] {
  const parts = [
    e.walls ? `${e.walls.toLocaleString("en-US")} walls` : "",
    e.doors ? `${e.doors} doors` : "",
    e.lights ? `${e.lights} lights` : "",
  ].filter(Boolean);
  return parts.length ? parts : ["Art and grid, ready to draw on"];
}

function Board({ entry, map, on }: { entry: Entry; map: MapData | null; on: Record<Layer, boolean> }) {
  const { width: w, height: h } = entry;
  const x0 = -w / 2;
  const y0 = -h / 2;
  return (
    <svg
      viewBox={`${x0} ${y0} ${w} ${h}`}
      role="img"
      aria-label={`${entry.title}, with its walls, doors and lights drawn over it`}
    >
      {/* The thumbnail stands in until the full map arrives. */}
      <image href={`/maps/${entry.name}.thumb.webp`} x={x0} y={y0} width={w} height={h} />
      <image href={`/maps/${entry.name}.webp`} x={x0} y={y0} width={w} height={h} />
      <rect x={x0} y={y0} width={w} height={h} className="map-wash" />
      {map && on.grid && (
        <g className="map-grid">
          {Array.from({ length: Math.ceil(w / map.gridSize) }, (_, i) => (
            <line key={`v${i}`} x1={x0 + i * map.gridSize} x2={x0 + i * map.gridSize} y1={y0} y2={-y0} />
          ))}
          {Array.from({ length: Math.ceil(h / map.gridSize) }, (_, i) => (
            <line key={`h${i}`} y1={y0 + i * map.gridSize} y2={y0 + i * map.gridSize} x1={x0} x2={-x0} />
          ))}
        </g>
      )}
      {map && on.lights && (
        <g className="map-lights">
          {map.lights.map((l, i) => (
            <g key={i}>
              <circle cx={l.x} cy={l.y} r={l.radius} className="light-dim" />
              <circle cx={l.x} cy={l.y} r={l.brightRadius} className="light-bright" />
              <circle cx={l.x} cy={l.y} r={22} className="light-dot" />
            </g>
          ))}
        </g>
      )}
      {map && on.walls && (
        <g className="map-walls">
          {map.walls
            .filter((wall) => wall.doorState === "none")
            .map((wall, i) => (
              <line key={i} x1={wall.x1} y1={wall.y1} x2={wall.x2} y2={wall.y2} />
            ))}
        </g>
      )}
      {map && on.doors && (
        <g className="map-doors">
          {map.walls
            .filter((wall) => wall.doorState !== "none")
            .map((wall, i) => (
              <line key={i} x1={wall.x1} y1={wall.y1} x2={wall.x2} y2={wall.y2} />
            ))}
        </g>
      )}
    </svg>
  );
}

export default function MapLegend() {
  const [atlas, setAtlas] = useState<Entry[] | null>(null);
  const [missing, setMissing] = useState(false);
  const [pick, setPick] = useState(0);
  const [maps, setMaps] = useState<Record<string, MapData>>({});
  const [on, setOn] = useState<Record<Layer, boolean>>({
    walls: true,
    doors: true,
    lights: true,
    grid: true,
  });

  useEffect(() => {
    fetch("/maps/index.json")
      .then((r) => (r.ok ? r.json() : Promise.reject()))
      .then((list: Entry[]) => (list.length ? setAtlas(list) : setMissing(true)))
      .catch(() => setMissing(true));
  }, []);

  const entry = atlas?.[pick] ?? null;
  useEffect(() => {
    if (!entry || maps[entry.name]) return;
    let live = true;
    loadGeometry(entry.name)
      .then((m) => live && setMaps((all) => ({ ...all, [entry.name]: m })))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [entry, maps]);

  const toggle = (id: Layer) => setOn((s) => ({ ...s, [id]: !s[id] }));
  const count: Record<Layer, number | null> = {
    walls: entry?.walls ?? null,
    doors: entry?.doors ?? null,
    lights: entry?.lights ?? null,
    grid: null,
  };

  return (
    <div className="legend-layout">
      <figure className="map-frame">
        {entry ? (
          <Board key={entry.name} entry={entry} map={maps[entry.name] ?? null} on={on} />
        ) : (
          <div className="map-missing">
            {missing ? "The maps live in the demo world." : "Unrolling the map…"}
          </div>
        )}
        <figcaption>
          {entry ? <strong>{entry.title}</strong> : "The demo world's maps"}, with the walls, doors
          and lights the server enforces drawn over it. Maps by MBRound18 (
          <a href={MAPS_SOURCE}>vtt-maps</a>, <a href={MAPS_CATALOG}>catalog</a>),{" "}
          <a href={CC_BY_SA}>CC BY-SA 4.0</a>.
        </figcaption>
      </figure>

      <div className="legend">
        <h3 className="legend-title">Legend</h3>
        <ul className="legend-layers">
          {LAYERS.map((l) => (
            <li key={l.id}>
              <button
                type="button"
                className={count[l.id] === 0 ? "legend-toggle legend-toggle--none" : "legend-toggle"}
                aria-pressed={on[l.id]}
                onClick={() => toggle(l.id)}
              >
                <Swatch id={l.id} />
                <span>
                  <strong>{l.label}</strong>
                  {count[l.id] !== null && (
                    <span className="legend-count">{count[l.id]!.toLocaleString("en-US")}</span>
                  )}{" "}
                  {l.note}
                </span>
              </button>
            </li>
          ))}
        </ul>
        <p className="legend-hint">Tap a line of the legend to lift that layer off the map.</p>
        <a className="ink-btn ink-btn--blue" href={DEMO}>
          Walk these maps in the demo
        </a>
      </div>

      {atlas && atlas.length > 1 && (
        <div className="atlas">
          <h3 id="atlas-title" className="atlas-title">
            {atlas.length} maps in the demo, all of them free to play.
          </h3>
          <ul className="atlas-list" role="group" aria-labelledby="atlas-title">
            {atlas.map((e, i) => (
              <li key={e.name}>
                <button
                  type="button"
                  className="atlas-tile"
                  aria-pressed={i === pick}
                  onClick={() => setPick(i)}
                  onPointerEnter={() => void loadGeometry(e.name).catch(() => undefined)}
                >
                  <img
                    src={`/maps/${e.name}.thumb.webp`}
                    alt=""
                    width={256}
                    height={Math.round((256 * e.height) / e.width)}
                    loading="lazy"
                    decoding="async"
                  />
                  <span className="atlas-name">{e.title}</span>
                  <span className="atlas-tally">
                    {tally(e).map((t, j) => (
                      <span key={t}>
                        {j > 0 && " · "}
                        <span className={/^\d/.test(t) ? "atlas-tally-item" : undefined}>{t}</span>
                      </span>
                    ))}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
