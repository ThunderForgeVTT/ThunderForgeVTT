import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import { createHero, PRESET_HEROES, type HeroSpec } from "@thunderforge/heroes";
import { canvasOf, holderId, running, sendAs, take, type Send } from "../engine/host.ts";
import { listen } from "../engine/dispatch.ts";
import type { Progress } from "../engine/loadEngine.ts";
import { PARTY } from "../room/party.ts";
import { journeyHero, setJourneyHero, watchJourneyHero, type Walker } from "./journeyHero.ts";
import { rasterize, svgUrl } from "./heroArt.ts";
import { announcePage, PAGE_TABLE } from "./onPage.ts";
import "./page-table.css";
import { currentMat, watchMat } from "./heroMat.ts";
import {
  along,
  fraction,
  hourAt,
  pathDown,
  toWorld,
  torchesAlong,
  type Box,
  type Doc,
  type Pt,
} from "./journey.ts";
import { darknessAt } from "./daylight.ts";
import { downAt } from "./goblins.ts";
import { centreOf, MatScene } from "./matScene.ts";
import { pickedTile, watchPick } from "./mapPick.ts";
import { currentBoard, leftAt, seat, seatedAt, watchBoard, type Board } from "../room/pageBoard.ts";
import { BoardArt, boardHold, blockedCells, boardStops, inside, placeOf, standUp } from "./roomOnPage.ts";
import {
  beforeCamp,
  CAMP_FIRE,
  CAMP_GLOW,
  campFire,
  campMat,
  campStops,
  currentCamp,
  watchCamp,
} from "./campMat.ts";
import {
  buildRoute,
  pageStations,
  pointAt,
  sAt,
  stepToward,
  type Planner,
  type Route,
  type Station,
} from "./route.ts";

// Prototype: the whole page on the engine's table.
//
// One engine canvas fills the viewport behind the page. The first screen's
// room is on it with real walls and a real door (`matScene.ts`); the hero,
// picked or drawn by lot, steps off the mat and walks a path down the page
// as the visitor scrolls, and goes where a tap on the table says.
// The camera is locked to the page, so the engine's grid is the page's grid.
//
// Touch first: a tap on the mat walks the hero there, on any screen.
//
// The hero builder is on the page too: a hero built in the note is drawn
// to pixels here and handed to the engine as bytes (`photoData`), so the
// walker can be anyone, not only the four with art on disk.

const HeroBuilder = lazy(() => import("./HeroBuilder.tsx"));

/** A full torch's intensity. */
const TORCH = 0.3;
const PAGE = PAGE_TABLE;
const WALKER = "walker";
const LANTERN = "walker-lantern";
/** The walker's lantern: enough to find the hero by, never a glare. */
const LANTERN_GLOW = 0.35;
const REDUCE = "(prefers-reduced-motion: reduce)";
/** The grid: a grey that reads as ink on the day mat and as chalk on the night one,
 * so it never flips while the engine crossfades them (spec 099 US3). */
const GRID_INK = "#5f665f38";
/** Past this much dark the page's words go light (`html.dusk`). */
const LIGHTS_ON = 0.8;
/** How long the engine's clock takes to reach the hour the scroll asks for, in
 * seconds: a flick on a phone crosses the evening over time, not in a frame. */
const CLOCK_SECS = 0.3;
/** What a tap on counts as the table, not the page's words. */
const SURFACES = "html, body, main, header.hero, .hero-body, .section";

/** How long the engine takes to hold walls just sent, before a plan round them. */
const PLAN_AFTER = 150;

/** How long the scroll rests before the page takes the table back. */
const SETTLE = 300;

type Phase = "idle" | "held" | "loading" | "running" | "away" | "failed";

const mb = (n: number) => (n / 1e6).toFixed(0);

function cellSize(): number {
  const v = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--cell"));
  return Number.isFinite(v) && v > 0 ? v : 48;
}

function measure(): { boxes: Box[]; doc: Doc } {
  const doc = { w: document.documentElement.clientWidth, h: document.documentElement.scrollHeight };
  const boxes: Box[] = [];
  document.querySelectorAll<HTMLElement>("[data-section], #tavern, #room").forEach((el) => {
    const r = el.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) return;
    boxes.push({
      id: el.dataset.section ?? el.id,
      x: r.left,
      y: r.top + window.scrollY,
      w: r.width,
      h: r.height,
    });
  });
  boxes.sort((a, b) => a.y - b.y);
  return { boxes, doc };
}

/** Spec 098 FR-007: the cell under the picked atlas tile, held a while. */
function underPick(cell: number, hold: number): Station[] {
  const r = pickedTile()?.getBoundingClientRect();
  if (!r || r.width === 0) return [];
  const x = Math.floor((r.left + r.width / 2) / cell) * cell + cell / 2;
  const y = Math.ceil((r.bottom + window.scrollY) / cell) * cell + cell / 2;
  // On a narrow page the legend lies under the tiles: the hero would stand
  // behind it, and keeps to its gutter instead.
  const card = document.querySelector(".legend")?.getBoundingClientRect();
  const vy = y - window.scrollY;
  if (card && x > card.left && x < card.right && vy > card.top - cell && vy < card.bottom) return [];
  return [{ at: { x, y }, hold }];
}

/** The pixel size the built hero is drawn at: crisp in a cell on a 3x screen. */
const ART_PX = 192;

/** `withArt` only when the art changed: the move loop sends this every frame,
 * and a `photoData` the engine must decode is not for every frame. */
function heroToken(hero: Walker, at: Pt, withArt = false) {
  const art = !withArt ? {} : hero.data ? { photoData: hero.data } : { photoUrl: hero.art };
  return {
    type: "upsert_token",
    token: { id: WALKER, x: at.x, y: at.y, z: 10, label: hero.name, tokenType: "character", ...art },
  };
}

function presetSpec(slug: string): HeroSpec {
  return PRESET_HEROES.find((p) => p.slug === slug)?.spec ?? { name: "Someone" };
}

export default function PageTable() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [why, setWhy] = useState("");
  const [progress, setProgress] = useState<Progress | null>(null);
  const [hero, setHero] = useState<Walker>(journeyHero);
  const [pct, setPct] = useState(0);
  const [open, setOpen] = useState(false);
  const [mode, setMode] = useState<"party" | "build">("party");
  const [spec, setSpec] = useState<HeroSpec>(() => presetSpec(hero.slug));
  const [busy, setBusy] = useState(false);
  const stage = useRef<HTMLDivElement>(null);
  const note = useRef<HTMLElement>(null);

  // What the frame loop and the listeners read; never a stale render's copy.
  const live = useRef({
    phase: "idle" as Phase,
    hero: PARTY[0]! as Walker,
    doc: { w: 1, h: 1 } as Doc,
    path: [] as Pt[],
    /** The path below the first screen; `path` is this from the hero's mat spot. */
    rest: [] as Pt[],
    pos: { x: 0, y: 0 } as Pt,
    target: { x: 0, y: 0 } as Pt,
    free: false,
    cell: 48,
    /** Where the stage stands on the page, and its height, in page pixels. */
    top: 0,
    stageW: 0,
    stageH: 0,
    /** Where it moves once the engine draws the camera sent for it. */
    moving: null as number | null,
    /** Where day ends and the night is deepest, in scroll pixels. */
    dayEnds: 0,
    nightAt: 1,
    /** The torches, in page pixels. */
    torchAt: [] as Pt[],
    /** The hour last sent to the engine's clock (-1: none yet), and how many
     * torches are on the table. */
    hour: -1,
    torches: 0,
    /** A wide page's route (spec 098), the hero's distance along it, the
     * distance the scroll asks for, and whether the hero is on it yet. */
    route: null as Route | null,
    s: 0,
    goalS: 0,
    joined: true,
    /** Whether the route waits where it is until the hero is back on it. */
    rejoin: false,
    /** The foes that are down, as last sent. */
    down: "",
    /** Where the hero stood on the screen when the note last looked. */
    behind: "",
  });
  live.current.hero = hero;

  const sendAll = useRef<Send>(sendAs(PAGE)).current;
  /** Sends, and says on the stage where the walker was sent: the engine
   * snaps it to its square, and the walk's tests want the way it took. */
  const send = useRef<Send>((command) => {
    const token = command.token as { id?: string; x?: number; y?: number } | undefined;
    if (command.type === "upsert_token" && token?.id === WALKER && stage.current) {
      const { doc } = live.current;
      stage.current.dataset.walker = `${Math.round(token.x! + doc.w / 2)},${Math.round(doc.h / 2 - token.y!)}`;
    }
    sendAll(command);
  }).current;
  const mat = useRef<MatScene | null>(null);
  mat.current ??= new MatScene(send, () => canvasOf(PAGE) !== null);
  const camp = useRef<MatScene | null>(null);
  camp.current ??= new MatScene(send, () => canvasOf(PAGE) !== null, "camp");
  const boardArt = useRef<BoardArt | null>(null);
  boardArt.current ??= new BoardArt(send);

  const go = useCallback((next: Phase) => {
    live.current.phase = next;
    setPhase(next);
    document.body.classList.toggle("in-engine", next === "running");
    announcePage();
  }, []);

  /** The camera over the stage, exactly: the grid is the page's grid. */
  const aim = useCallback((top = live.current.top) => {
    const el = stage.current;
    if (!el) return;
    const { doc } = live.current;
    // Kept from `size`: a layout read here would force one on every jump.
    const width = live.current.stageW || el.clientWidth;
    const height = live.current.stageH || el.clientHeight;
    const c = toWorld({ x: width / 2, y: top + height / 2 }, doc);
    send({
      type: "fit_camera_to",
      centerX: c.x,
      centerY: c.y,
      width,
      height,
      immediate: true,
    });
  }, [send]);

  /**
   * The stage is a window onto the table that scrolls with the page, a margin
   * taller than the screen each way, so the browser moves it with the words
   * and the grid never trails them. As the screen nears its edge, the window
   * and the camera jump together, by whole cells, where the grid looks the
   * same either side of the jump.
   *
   * Together means in one painted frame. The torches and the hero do not
   * repeat like the grid: a window that moved a frame before the engine drew
   * the camera for it showed them a jump away, and on a phone, where frames
   * are slow, they blinked. So the camera goes first, and the window follows
   * from inside the engine's update that draws it (`cameraFitted`). The jump
   * starts a half margin early, while the old window still covers the
   * screen, so the frames it waits are never bare.
   */
  const moveTo = useCallback((top: number) => {
    const L = live.current;
    L.moving = null;
    L.top = top;
    if (stage.current) stage.current.style.top = `${top}px`;
  }, []);

  const place = useCallback(
    (force = false) => {
      const el = stage.current;
      if (!el) return;
      const L = live.current;
      const y = window.scrollY;
      const vh = window.innerHeight;
      // Kept from `size`: a layout read here would force one on every scroll.
      const h = L.stageH || el.clientHeight;
      const margin = (h - vh) / 2;
      const top = Math.max(0, Math.min(Math.floor((y - margin) / L.cell) * L.cell, L.doc.h - h));
      if (force) {
        moveTo(top);
        aim(top);
        return;
      }
      const guard = margin / 2;
      if (y >= L.top + guard && y + vh <= L.top + h - guard) return;
      if (top === L.top || top === L.moving) return;
      L.moving = top;
      aim(top);
      // An engine that never says so (an older build) still gets its window.
      setTimeout(() => {
        if (live.current.moving === top) moveTo(top);
      }, 250);
    },
    [aim, moveTo],
  );

  // The engine drew the camera a jump asked for: the window joins it now.
  useEffect(
    () =>
      listen(PAGE, (e) => {
        const L = live.current;
        const el = stage.current;
        // The words go light with the dark the engine draws, not ahead of it.
        if (e.type === "time_of_day" && e.hour !== null && !seatedAt())
          document.documentElement.classList.toggle("dusk", e.darkness > LIGHTS_ON);
        if (e.type !== "cameraFitted" || L.moving === null || !el) return;
        const want = toWorld({ x: 0, y: L.moving + (L.stageH || el.clientHeight) / 2 }, L.doc).y;
        if (Math.abs(e.centerY - want) < 0.5) moveTo(L.moving);
      }),
    [moveTo],
  );

  /** The stage's height: the screen and a margin of whole cells each way. */
  const size = useCallback(() => {
    const el = stage.current;
    if (!el) return;
    const L = live.current;
    const vh = window.innerHeight;
    const margin = Math.max(1, Math.round(vh / 3 / L.cell)) * L.cell;
    L.stageH = Math.min(vh + 2 * margin, L.doc.h);
    el.style.height = `${L.stageH}px`;
    L.stageW = el.clientWidth;
  }, []);

  /** Where the scroll puts the hero on the route. */
  const goalOf = useCallback((r: Route) => {
    const L = live.current;
    const vh = window.innerHeight;
    return sAt(r, window.scrollY, vh, Math.max(0, L.doc.h - vh));
  }, []);

  /** The engine's way round the walls on the table, in page pixels. */
  const planner = useCallback((): Planner | null => {
    const e = running();
    if (!e?.plan_path || canvasOf(PAGE) === null) return null;
    const L = live.current;
    const { doc, cell } = L;
    const grid = { size: cell, originX: -doc.w / 2, originY: doc.h / 2 };
    const bounds = { min: { q: 0, r: -Math.ceil(doc.h / cell) }, max: { q: Math.ceil(doc.w / cell) - 1, r: -1 } };
    // Round the dungeon's board and the camp, but straight into them by their top.
    const maps = [currentBoard(), currentCamp()].filter((m) => m !== null);
    const blocked = maps.flatMap((m) => blockedCells(m, cell));
    return (a, b) => {
      if (maps.some((m) => inside(m, a) || inside(m, b))) return null;
      const ask = { from: toWorld(a, doc), to: toWorld(b, doc), grid, bounds, blocked, maxNodes: 40000 };
      const way = (JSON.parse(e.plan_path!(JSON.stringify(ask))) as { path: Pt[] | null }).path;
      return way?.map((p) => ({ x: p.x + doc.w / 2, y: doc.h / 2 - p.y })) ?? null;
    };
  }, []);

  /**
   * A wide page's route (spec 098): straight at once, then round the walls
   * once the engine holds them. The hero walks onto each new route from
   * where it stands, never jumps.
   */
  const planned = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const route = useCallback(
    (seat: Pt | null, exit: Pt) => {
      const L = live.current;
      const hold = Math.round(window.innerHeight * 0.2);
      const stops = () => [
        ...underPick(L.cell, hold),
        ...boardStops(currentBoard(), boardHold(window.innerHeight, L.cell)),
        ...campStops(currentCamp()),
      ];
      const stations = () => pageStations(seat, exit, beforeCamp(L.rest, currentCamp()), L.cell, hold, stops());
      const use = (r: Route) => {
        L.route = r;
        L.goalS = goalOf(r);
        L.s = L.goalS;
        L.joined = false;
        L.rejoin = false;
      };
      // Straight lines cross the dungeon's board: a planned route stays
      // until the new plan comes.
      if (!L.route || !currentBoard()) use(buildRoute(stations(), null));
      clearTimeout(planned.current);
      planned.current = setTimeout(() => {
        const plan = planner();
        if (plan && live.current.route) use(buildRoute(stations(), plan));
      }, PLAN_AFTER);
    },
    [goalOf, planner],
  );

  /** The first screen's room on the table, and the walker's start in it. */
  const drawMat = useCallback(() => {
    const L = live.current;
    const m = currentMat();
    if (!m || L.phase === "idle") return;
    const own = L.hero.data ? null : L.hero.slug;
    const spot = m.pieces.find((p) => p.id === own);
    mat.current!.draw(m, L.doc, L.cell, spot ? own : null);
    boardArt.current!.draw(currentBoard(), L.doc, canvasOf(PAGE) !== null);
    const c = currentCamp();
    if (c) camp.current!.draw(campMat(c, own), L.doc, L.cell, null);
    else camp.current!.clear();
    if (c) campFire(c, L.doc).forEach(send);
    else for (const lightId of [CAMP_FIRE, ...CAMP_GLOW]) send({ type: "remove_light", lightId });
    const from = spot ? [centreOf(spot)] : [];
    L.path = [...from, ...(m.exit ? [m.exit] : []), ...L.rest];
    if (!L.free) L.target = along(L.path, fraction(window.scrollY, L.doc, window.innerHeight));
    if (m.map && m.exit) route(spot ? centreOf(spot) : null, m.exit);
    else L.route = null;
  }, [route]);

  /** The page's grid and its mat, by day and by night. */
  const grid = useCallback(() => {
    const L = live.current;
    send({
      type: "set_scene_grid",
      gridType: "square",
      size: L.cell,
      mapWidth: L.doc.w,
      mapHeight: L.doc.h,
      visible: true,
      color: GRID_INK,
    });
    // By night the mat is lit only where a flame reaches it: firelight. The
    // engine crossfades the two with its clock (spec 099 US3).
    send({
      type: "set_scene_background",
      backgroundImagePath: "/table/mat.png",
      nightImagePath: "/table/mat-night.png",
      width: L.doc.w,
      height: L.doc.h,
    });
  }, [send]);

  /** Torch `i`; the engine lights it with the dark and makes it flicker. */
  const torch = useCallback(
    (i: number) => {
      const L = live.current;
      const at = toWorld(L.torchAt[i]!, L.doc);
      send({
        type: "upsert_light",
        light: {
          id: `torch-${i}`,
          x: at.x,
          y: at.y,
          radius: L.cell * 1.5,
          brightRadius: L.cell * 0.2,
          color: "#ffd9a8",
          // Firelight to read by, not to squint at: a full torch burns at
          // `TORCH`, and the words beside it keep their contrast.
          intensity: TORCH,
          // Out by day, catching as the dark falls (spec 099 US3).
          byNight: true,
          flicker: 0.5,
          castsShadows: false,
        },
      });
    },
    [send],
  );

  /** Torches at every section's head, and a lantern in the walker's hand. */
  const light = useCallback(
    (boxes: Box[]) => {
      const L = live.current;
      const torches = torchesAlong(boxes, L.doc, L.cell);
      L.torchAt = torches;
      torches.forEach((_, i) => torch(i));
      for (let i = torches.length; i < L.torches; i++) send({ type: "remove_light", lightId: `torch-${i}` });
      L.torches = torches.length;
      // Not a carried light: the engine burns those at full strength, which
      // on the page is a glare. This one goes where the walker goes and
      // burns as low as the torches.
      send({
        type: "upsert_light",
        light: {
          id: LANTERN,
          x: 0,
          y: 0,
          attachedTokenId: WALKER,
          radius: L.cell * 1.2,
          brightRadius: 0,
          color: "#ffd9a8",
          intensity: LANTERN_GLOW,
          flicker: 0.3,
          castsShadows: false,
        },
      });
    },
    [send, torch],
  );

  /**
   * Sets the engine's clock to the hour for where the visitor has scrolled
   * to (spec 099 US4). The engine eases the dark, its tint, the mat and the
   * torches toward that hour itself, a frame at a time: the page only says
   * what time it is.
   */
  const clock = useCallback(() => {
    const L = live.current;
    // The seated dungeon keeps its own dark (spec 098 FR-008).
    if (seatedAt()) return;
    const hour = Math.round(hourAt(window.scrollY, L.dayEnds, L.nightAt, window.innerHeight) * 1000) / 1000;
    if (hour === L.hour) return;
    const first = L.hour < 0;
    L.hour = hour;
    send({
      type: "set_time_of_day",
      hour,
      ...(first || matchMedia(REDUCE).matches ? {} : { overSecs: CLOCK_SECS }),
    });
  }, [send]);

  /** Measures the page and sends it as a scene: grid, mat, the room. */
  const layout = useCallback(() => {
    const L = live.current;
    // Out of the page's way while it is measured, so it never adds to it.
    stage.current?.style.setProperty("top", "0px");
    stage.current?.style.setProperty("height", "0px");
    const { boxes, doc } = measure();
    L.cell = cellSize();
    L.doc = doc;
    size();
    L.rest = pathDown(boxes, doc, L.cell);
    L.path = L.rest;
    const vh = window.innerHeight;
    const first = boxes[0];
    const room = boxes.find((b) => b.id === "tavern") ?? boxes.find((b) => b.id === "room");
    L.dayEnds = first ? first.y + first.h - vh / 2 : 0;
    L.nightAt = room ? room.y - vh / 4 : doc.h;
    grid();
    light(boxes);
    if (!L.free) L.target = along(L.path, fraction(window.scrollY, doc, window.innerHeight));
    drawMat();
    // The world's origin is the page's middle, so a page that grew moved
    // everything on it: a hero standing still must be told where it is now.
    if (L.phase === "running" && !seatedAt()) send(heroToken(L.hero, toWorld(L.pos, doc)));
    L.hour = -1;
    clock();
    place(true);
  }, [send, place, size, drawMat, grid, light, clock]);

  const setup = useCallback(
    (s: Send) => {
      const L = live.current;
      layout();
      L.pos = L.route ? pointAt(L.route, L.s) : { ...L.target };
      L.joined = true;
      s(heroToken(L.hero, toWorld(L.pos, L.doc), true));
      s({ type: "set_controlled_token", tokenId: WALKER });
      s({ type: "set_scene_playing", playing: true });
      aim();
    },
    [layout, aim],
  );

  const teardown = useCallback((s: Send) => {
    const L = live.current;
    seat(null);
    boardArt.current!.clear();
    for (let i = 0; i < L.torches; i++) s({ type: "remove_light", lightId: `torch-${i}` });
    L.torches = 0;
    s({ type: "remove_light", lightId: LANTERN });
    s({ type: "remove_light", lightId: CAMP_FIRE });
    for (const lightId of CAMP_GLOW) s({ type: "remove_light", lightId });
    s({ type: "set_time_of_day", hour: null });
    s({ type: "set_ambient_light", level: "bright" });
    L.hour = -1;
    s({ type: "set_scene_playing", playing: false });
    s({ type: "remove_token", tokenId: WALKER });
    mat.current!.clear();
    camp.current!.clear();
    L.down = "";
    s({ type: "set_scene_background", backgroundImagePath: null, width: 0, height: 0 });
  }, []);

  const boot = useCallback(async () => {
    if (!stage.current) return;
    go("loading");
    try {
      await take(
        { id: PAGE, stage: stage.current, setup, teardown, onTaken: () => go("away") },
        setProgress,
      );
      go("running");
      // The engine measures its canvas a few frames after it starts, and a
      // fit made before that is held at the old size: aim again as it settles.
      for (const ms of [100, 400, 1000]) setTimeout(aim, ms);
    } catch (err) {
      setWhy(err instanceof Error ? err.message : String(err));
      go("failed");
    }
  }, [go, setup, teardown, aim]);

  // The engine after the first paint, when the browser can run it.
  useEffect(() => {
    if (matchMedia(REDUCE).matches) {
      setWhy("reduced motion: the page stays still");
      go("held");
      return;
    }
    const gl = document.createElement("canvas").getContext("webgl2");
    if (!gl) {
      setWhy("no WebGL2 here");
      go("held");
      return;
    }
    gl.getExtension("WEBGL_lose_context")?.loseContext();
    const t = setTimeout(() => void boot(), 400);
    return () => clearTimeout(t);
    // Once, on mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Scroll drives the journey and the camera; a frame that took the table
  // gives it back once it is well out of view.
  useEffect(() => {
    let settle: ReturnType<typeof setTimeout> | undefined;
    const onScroll = () => {
      const L = live.current;
      const t = fraction(window.scrollY, L.doc, window.innerHeight);
      setPct(Math.round(t * 100));
      const shade = darknessAt(hourAt(window.scrollY, L.dayEnds, L.nightAt, window.innerHeight));
      // For the dusk's tests (SC-006): the dark the page is heading for.
      if (stage.current) stage.current.dataset.dusk = shade.toFixed(4);
      if (L.phase === "away") {
        // The room holds the table, but the page's night still follows.
        document.documentElement.classList.toggle("dusk", shade > LIGHTS_ON);
        // Taken back once the scroll settles, not in the middle of a flick.
        clearTimeout(settle);
        settle = setTimeout(() => {
          if (live.current.phase !== "away") return;
          const holder = holderId();
          const el = holder ? document.getElementById(holder) : null;
          const r = el?.getBoundingClientRect();
          if (r && (r.bottom < -window.innerHeight || r.top > 2 * window.innerHeight)) void boot();
        }, SETTLE);
        return;
      }
      if (L.phase !== "running") return;
      if (L.free) L.joined = false;
      L.free = false;
      L.target = along(L.path, t);
      if (L.route) L.goalS = goalOf(L.route);
      clock();
      place();
    };
    window.addEventListener("scroll", onScroll, { passive: true });
    onScroll();
    return () => {
      clearTimeout(settle);
      window.removeEventListener("scroll", onScroll);
    };
  }, [place, boot, clock, goalOf]);

  // A tap on the mat walks the hero there.
  useEffect(() => {
    const onClick = (e: MouseEvent) => {
      const L = live.current;
      if (L.phase !== "running") return;
      const el = e.target as Element | null;
      if (!el || !el.matches(SURFACES)) return;
      L.free = true;
      L.joined = false;
      L.target = { x: e.pageX, y: e.pageY };
    };
    document.addEventListener("click", onClick);
    return () => document.removeEventListener("click", onClick);
  }, []);

  // The page's size changes: fonts, images, a frame opening.
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | null = null;
    const ro = new ResizeObserver(() => {
      if (live.current.phase !== "running") return;
      if (timer) clearTimeout(timer);
      timer = setTimeout(layout, 150);
    });
    ro.observe(document.body);
    return () => {
      ro.disconnect();
      if (timer) clearTimeout(timer);
    };
  }, [layout]);

  /** The dungeon's first square on the route, while it has a board. */
  const startMark = (b: Board | null) =>
    b && live.current.route?.marks.find((m) => m.hold > 0 && m.at.x === b.start.x && m.at.y === b.start.y);

  /** Spec 098 FR-008: the hero stands still on the dungeon's first square,
   * and the room sits down to play. */
  const sit = useCallback(() => {
    const L = live.current;
    const b = currentBoard();
    const m = startMark(b);
    if (!b || !m || L.free || L.s !== m.s || L.goalS !== m.s || canvasOf(PAGE) === null) return;
    seat(placeOf(b, L.doc, L.cell, WALKER));
    // The clock stops: the room's own dark rules until it stands up.
    send({ type: "set_time_of_day", hour: null });
    L.hour = -1;
  }, [send]);

  /** Whether the room stays seated; when not, it stands up, and the walker
   * walks on from where the room left it, under the page's own dusk. */
  const stand = useCallback(() => {
    const L = live.current;
    const b = currentBoard();
    const m = startMark(b);
    if (b && m && !standUp(b, window.scrollY, window.innerHeight, L.cell)) return false;
    seat(null);
    const at = leftAt();
    if (at) L.pos = at;
    // Back to the first square before on along the route, out by the top:
    // never straight across the board.
    if (m) {
      L.s = m.s;
      L.rejoin = true;
    }
    L.joined = false;
    L.hour = -1;
    clock();
    return true;
  }, [clock]);

  // The dungeon's board laid or moved: drawn again, and a room seated on
  // the old place stands up. Played or not changes neither.
  const boardKey = useRef("");
  useEffect(
    () =>
      watchBoard((b) => {
        const key = b ? [b.x, b.y, b.w, b.h].join() : "";
        if (key === boardKey.current) return;
        boardKey.current = key;
        if (seatedAt()) {
          seat(null);
          live.current.joined = false;
        }
        if (live.current.phase === "running") drawMat();
      }),
    [drawMat],
  );

  // The walk: the hero closes on the target at a believable pace.
  useEffect(() => {
    let raf = 0;
    const step = () => {
      raf = requestAnimationFrame(step);
      const L = live.current;
      if (L.phase !== "running") return;
      // The note steps back while the hero stands behind it, unless open.
      const vy = L.pos.y - window.scrollY;
      const key = `${Math.round(L.pos.x)},${Math.round(vy)}`;
      if (note.current && key !== L.behind) {
        L.behind = key;
        const r = note.current.getBoundingClientRect();
        const half = L.cell / 2;
        const under =
          L.pos.x + half > r.left && L.pos.x - half < r.right && vy + half > r.top && vy - half < r.bottom;
        if (under) note.current.dataset.behind = "";
        else delete note.current.dataset.behind;
      }
      if (seatedAt() && !stand()) return;
      if (L.route && !L.free) {
        // Along the route, never across it (FR-004).
        if (!L.rejoin) L.s = stepToward(L.s, L.goalS);
        L.target = pointAt(L.route, L.s);
        // The goblins fall as the hero passes them (spec 098 US2).
        const foes = (currentMat()?.pieces ?? []).filter((p) => p.foe).map((p) => ({ id: p.id, at: centreOf(p) }));
        const down = downAt(L.route, foes, L.s);
        const key = [...down].join();
        if (key !== L.down) {
          L.down = key;
          mat.current!.setDown(down);
        }
        if (L.joined) {
          if (L.target.x === L.pos.x && L.target.y === L.pos.y) {
            sit();
            return;
          }
          L.pos = L.target;
          send(heroToken(L.hero, toWorld(L.pos, L.doc)));
          return;
        }
      }
      const dx = L.target.x - L.pos.x;
      const dy = L.target.y - L.pos.y;
      const dist = Math.hypot(dx, dy);
      if (dist < 0.5) {
        L.joined = true;
        L.rejoin = false;
        return;
      }
      // Never past the target: a step that overshot would send a move every
      // frame for ever, and each one wakes the engine before it can draw.
      const speed = Math.min(dist, Math.max(3, dist * 0.12));
      L.pos = { x: L.pos.x + (dx / dist) * speed, y: L.pos.y + (dy / dist) * speed };
      send(heroToken(L.hero, toWorld(L.pos, L.doc)));
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, [send, sit, stand]);

  /** `h` walks the page from here on. */
  const walkAs = (h: Walker) => {
    const L = live.current;
    // Still on their seat in the tavern, the two trade seats at once: no
    // walk across the room, no two on one square. The engine glides a moved
    // token, so the walker is lifted and set down instead.
    const seatOf = (w: Walker) => (w.data ? undefined : currentMat()?.pieces.find((p) => p.id === w.slug));
    const was = seatOf(L.hero);
    const next = seatOf(h);
    const seated = was && Math.hypot(L.pos.x - centreOf(was).x, L.pos.y - centreOf(was).y) < L.cell / 2;
    setHero(h);
    L.hero = h;
    if (!h.data) setSpec(presetSpec(h.slug));
    if (L.phase === "running") {
      const swap = seated && next;
      if (swap) {
        L.pos = L.target = centreOf(next);
        // The old route starts on the old seat: a new one, from this one.
        L.route = null;
        send({ type: "remove_token", tokenId: WALKER });
      }
      send(heroToken(h, toWorld(L.pos, L.doc), true));
      if (swap) send({ type: "set_controlled_token", tokenId: WALKER });
      drawMat();
    }
  };
  const pick = (h: Walker) => {
    walkAs(h);
    setJourneyHero(h);
  };
  // One of the party tapped in the tavern.
  const walking = useRef(walkAs);
  walking.current = walkAs;
  useEffect(() => watchJourneyHero((h) => h !== live.current.hero && walking.current(h)), []);

  // A token dragged or a door swung on the first screen.
  useEffect(() => watchMat(() => live.current.phase === "running" && drawMat()), [drawMat]);
  // Another map picked in the atlas: the hero goes to stand under it.
  useEffect(() => watchPick(() => live.current.phase === "running" && drawMat()), [drawMat]);
  // The camp laid or moved, or its party known (spec 098 US6).
  useEffect(() => watchCamp(() => live.current.phase === "running" && drawMat()), [drawMat]);

  /** The built hero, drawn to pixels and sent to walk. */
  const apply = async () => {
    setBusy(true);
    try {
      const svg = createHero(spec).token();
      const data = await rasterize(svg, ART_PX);
      pick({ slug: "built", name: spec.name.trim() || "Someone", art: svgUrl(svg), data });
      setOpen(false);
    } catch (err) {
      setWhy(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const status =
    phase === "loading"
      ? progress
        ? `Setting the table: ${mb(progress.loaded)} of ${mb(progress.total)} MB`
        : "Setting the table"
      : phase === "running"
        ? "Tap the mat to walk there."
        : phase === "away"
          ? "The table is in the room."
          : phase === "failed"
            ? `The table could not be set: ${why}`
            : phase === "held"
              ? why
              : "";

  return (
    <>
      <div
        id={PAGE}
        ref={stage}
        className={`page-table${phase === "running" ? " page-table--running" : ""}`}
        aria-hidden="true"
      />
      <aside
        ref={note}
        className={`journey${open ? " journey--open" : ""}${open && mode === "build" ? " journey--build" : ""}`}
        aria-label="Your hero's walk"
        style={{ ["--journey" as string]: pct / 100 }}
      >
        <button
          type="button"
          className="journey-face"
          aria-label={open ? "Hide the party" : "Pick another hero"}
          aria-expanded={open}
          onClick={() => setOpen((o) => !o)}
        >
          <img src={hero.art} alt="" width={52} height={52} />
        </button>
        <div className="journey-words">
          <p className="journey-title">{pct >= 99 ? "That's the lot." : `${hero.name.split(" ")[0]} scrolls on`}</p>
          <div
            className="journey-track"
            role="progressbar"
            aria-label="How far down the page"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={pct}
          >
            <div className="journey-ink" />
          </div>
          <p className="journey-status">{status}</p>
        </div>
        {open && (
          <div className="journey-tools" role="group" aria-label="Who walks">
            <button type="button" aria-pressed={mode === "party"} onClick={() => setMode("party")}>
              The party
            </button>
            <button type="button" aria-pressed={mode === "build"} onClick={() => setMode("build")}>
              Build a hero
            </button>
          </div>
        )}
        {mode === "party" && (
          <ul className="journey-party" aria-label="Your hero">
            {PARTY.map((h) => (
              <li key={h.slug}>
                <button
                  type="button"
                  aria-pressed={h.slug === hero.slug}
                  title={h.name}
                  onClick={() => {
                    pick(h);
                    setOpen(false);
                  }}
                >
                  <img src={h.art} alt={h.name} width={36} height={36} />
                </button>
              </li>
            ))}
            {hero.data && (
              <li>
                <button type="button" aria-pressed title={hero.name}>
                  <img src={hero.art} alt={hero.name} width={36} height={36} />
                </button>
              </li>
            )}
          </ul>
        )}
        {open && mode === "build" && (
          <Suspense fallback={<p className="journey-status">Fetching the parts…</p>}>
            <HeroBuilder spec={spec} onChange={setSpec} onApply={() => void apply()} busy={busy} />
          </Suspense>
        )}
      </aside>
    </>
  );
}

