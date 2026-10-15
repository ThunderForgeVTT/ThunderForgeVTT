import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import { createHero, PRESET_HEROES, type HeroSpec } from "@thunderforge/heroes";
import { canvasOf, holderId, sendAs, take, type Send } from "../engine/host.ts";
import { listen } from "../engine/dispatch.ts";
import type { Progress } from "../engine/loadEngine.ts";
import { PARTY } from "../room/party.ts";
import { journeyHero, setJourneyHero, type Walker } from "./journeyHero.ts";
import { rasterize, svgUrl } from "./heroArt.ts";
import { announcePage, PAGE_TABLE } from "./onPage.ts";
import "./page-table.css";
import { currentMat, watchMat } from "./heroMat.ts";
import {
  along,
  dawning,
  duskAt,
  fraction,
  pathDown,
  toWorld,
  torchesAlong,
  type Box,
  type Doc,
  type Pt,
} from "./journey.ts";
import { centreOf, MatScene } from "./matScene.ts";

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
/** The grid's ink on the paper mat: the page's black, faint. */
const GRID_INK = "#1f2a2e2e";
/** The same grid after dark: chalk instead of ink. */
const GRID_CHALK = "#eceee924";
/** Past this much dusk the page's words go light (`html.dusk`). */
const LIGHTS_ON = 0.8;
/**
 * Past this much dusk the torches are lit and the mat is firelit; they burn
 * up to full by `LIGHTS_ON`. At dawn they go out before the day mat returns,
 * so a torch never pools white on paper.
 */
const EMBERS = 0.5;
/** The dark as dawn breaks: amber, warming the paper rather than greying it. */
const DAWN_TINT = "#4a2c14";
/** How long the dark takes to catch up with the scroll, in ms (a time constant). */
const DUSK_MS = 260;
/** What a tap on counts as the table, not the page's words. */
const SURFACES = "html, body, main, header.hero, .hero-body, .section";

/** The tavern a wide page opens on (`Hero.tsx`). */
const OPENING = ".hero-tavern";
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
    dawnAt: 2,
    /** The torches, in page pixels. */
    torchAt: [] as Pt[],
    /** The dusk last sent, and how many torches are on the table. */
    shade: -1,
    torches: 0,
    /** How brightly the torches burn, 0 to 1. */
    glow: 0,
    /** The dusk the table is fading towards, its tint, and the tint last sent. */
    goal: 0,
    tint: undefined as string | undefined,
    sentTint: undefined as string | undefined,
    /** Whether a fade is running. */
    fading: false,
  });
  live.current.hero = hero;

  const send = useRef<Send>(sendAs(PAGE)).current;
  const mat = useRef<MatScene | null>(null);
  mat.current ??= new MatScene(send, () => canvasOf(PAGE) !== null);

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

  /** The first screen's room on the table, and the walker's start in it. */
  const drawMat = useCallback(() => {
    const L = live.current;
    const m = currentMat();
    if (!m || L.phase === "idle") return;
    const own = L.hero.data ? null : L.hero.slug;
    const spot = m.pieces.find((p) => p.id === own);
    mat.current!.draw(m, L.doc, L.cell, spot ? own : null);
    L.path = spot ? [centreOf(spot), ...L.rest] : L.rest;
    if (!L.free) L.target = along(L.path, fraction(window.scrollY, L.doc, window.innerHeight));
  }, []);

  /** The page's grid, in ink by day and chalk by night. */
  const grid = useCallback(
    (night: boolean) => {
      const L = live.current;
      send({
        type: "set_scene_grid",
        gridType: "square",
        size: L.cell,
        mapWidth: L.doc.w,
        mapHeight: L.doc.h,
        visible: true,
        color: night ? GRID_CHALK : GRID_INK,
      });
      // By night the mat is lit only where a flame reaches it: firelight.
      send({
        type: "set_scene_background",
        backgroundImagePath: night ? "/table/mat-night.png" : "/table/mat.png",
        width: L.doc.w,
        height: L.doc.h,
      });
    },
    [send],
  );

  /** Torch `i`, burning at `glow` (0 out, 1 full); the engine makes it flicker. */
  const torch = useCallback(
    (i: number, glow: number) => {
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
          intensity: glow * TORCH,
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
      torches.forEach((_, i) => torch(i, L.glow));
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

  /** Darkens the table to `shade`: the ambient, the words, the mat and the torches. */
  const shadeTo = useCallback(
    (shade: number) => {
      const L = live.current;
      if (Math.abs(shade - L.shade) < 0.001 && L.tint === L.sentTint) return;
      const wasLit = L.shade > EMBERS;
      L.shade = shade;
      L.sentTint = L.tint;
      send({ type: "set_ambient_light", level: "bright", shade, color: L.tint });
      document.documentElement.classList.toggle("dusk", shade > LIGHTS_ON);
      const lit = shade > EMBERS;
      if (lit !== wasLit) grid(lit);
      // The torches catch and gutter with the dark, in steps fine enough
      // that no one sees them: each step is a changed light set.
      const glow = Math.round(Math.min(1, Math.max(0, (shade - EMBERS) / (LIGHTS_ON - EMBERS))) * 32) / 32;
      if (glow !== L.glow) {
        L.glow = glow;
        L.torchAt.forEach((_, i) => torch(i, glow));
      }
    },
    [send, grid, torch],
  );

  /**
   * The dusk for where the visitor has scrolled to; the words light up past
   * `LIGHTS_ON`. The dark follows the scroll over time, not in one step: a
   * flick on a phone crosses the evening within a frame, and the page went
   * from day to night, and the words from ink to chalk, in a blink.
   */
  const dusk = useCallback(() => {
    const L = live.current;
    const vh = window.innerHeight;
    L.goal = duskAt(window.scrollY, L.dayEnds, L.nightAt, L.dawnAt, vh);
    // The night is blue; as dawn breaks the dark warms to amber.
    L.tint = dawning(window.scrollY, L.dawnAt, vh) > 0 ? DAWN_TINT : undefined;
    if (L.shade < 0 || matchMedia(REDUCE).matches) {
      shadeTo(L.goal);
      return;
    }
    if (L.fading) return;
    L.fading = true;
    let last = performance.now();
    const step = (now: number) => {
      const L = live.current;
      if (L.phase !== "running") {
        L.fading = false;
        return;
      }
      const k = 1 - Math.exp(-(now - last) / DUSK_MS);
      last = now;
      const next = Math.abs(L.goal - L.shade) < 0.002 ? L.goal : L.shade + (L.goal - L.shade) * k;
      shadeTo(next);
      if (next === L.goal) L.fading = false;
      else requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  }, [shadeTo]);

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
    // The night holds to the page's end: the star chart is in a night sky.
    L.dawnAt = doc.h * 2;
    grid(L.shade > EMBERS);
    light(boxes);
    if (!L.free) L.target = along(L.path, fraction(window.scrollY, doc, window.innerHeight));
    drawMat();
    // The world's origin is the page's middle, so a page that grew moved
    // everything on it: a hero standing still must be told where it is now.
    if (L.phase === "running") send(heroToken(L.hero, toWorld(L.pos, doc)));
    L.shade = -1;
    dusk();
    place(true);
  }, [send, place, size, drawMat, grid, light, dusk]);

  const setup = useCallback(
    (s: Send) => {
      const L = live.current;
      layout();
      L.pos = { ...L.target };
      s(heroToken(L.hero, toWorld(L.pos, L.doc), true));
      s({ type: "set_controlled_token", tokenId: WALKER });
      s({ type: "set_scene_playing", playing: true });
      aim();
    },
    [layout, aim],
  );

  const teardown = useCallback((s: Send) => {
    const L = live.current;
    for (let i = 0; i < L.torches; i++) s({ type: "remove_light", lightId: `torch-${i}` });
    L.torches = 0;
    L.glow = 0;
    s({ type: "remove_light", lightId: LANTERN });
    s({ type: "set_ambient_light", level: "bright" });
    L.shade = -1;
    s({ type: "set_scene_playing", playing: false });
    s({ type: "remove_token", tokenId: WALKER });
    mat.current!.clear();
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
    // A wide page opens on the tavern beside the headline, which takes the
    // table itself; the page's table waits until the tavern is left behind.
    if (document.querySelector(OPENING)) {
      go("away");
      return;
    }
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
      if (L.phase === "away") {
        // The room holds the table, but the page's night still follows.
        const shade = duskAt(window.scrollY, L.dayEnds, L.nightAt, L.dawnAt, window.innerHeight);
        document.documentElement.classList.toggle("dusk", shade > LIGHTS_ON);
        // Taken back once the scroll settles, not in the middle of a flick.
        clearTimeout(settle);
        settle = setTimeout(() => {
          if (live.current.phase !== "away") return;
          const holder = holderId();
          const el = holder ? document.getElementById(holder) : document.querySelector(OPENING);
          const r = el?.getBoundingClientRect();
          if (r && (r.bottom < -window.innerHeight || r.top > 2 * window.innerHeight)) void boot();
        }, SETTLE);
        return;
      }
      if (L.phase !== "running") return;
      L.free = false;
      L.target = along(L.path, t);
      dusk();
      place();
    };
    window.addEventListener("scroll", onScroll, { passive: true });
    onScroll();
    return () => {
      clearTimeout(settle);
      window.removeEventListener("scroll", onScroll);
    };
  }, [place, boot, dusk]);

  // A tap on the mat walks the hero there.
  useEffect(() => {
    const onClick = (e: MouseEvent) => {
      const L = live.current;
      if (L.phase !== "running") return;
      const el = e.target as Element | null;
      if (!el || !el.matches(SURFACES)) return;
      L.free = true;
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

  // The walk: the hero closes on the target at a believable pace.
  useEffect(() => {
    let raf = 0;
    const step = () => {
      raf = requestAnimationFrame(step);
      const L = live.current;
      if (L.phase !== "running") return;
      const dx = L.target.x - L.pos.x;
      const dy = L.target.y - L.pos.y;
      const dist = Math.hypot(dx, dy);
      if (dist < 0.5) return;
      // Never past the target: a step that overshot would send a move every
      // frame for ever, and each one wakes the engine before it can draw.
      const speed = Math.min(dist, Math.max(3, dist * 0.12));
      L.pos = { x: L.pos.x + (dx / dist) * speed, y: L.pos.y + (dy / dist) * speed };
      send(heroToken(L.hero, toWorld(L.pos, L.doc)));
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, [send]);

  const pick = (h: Walker) => {
    setHero(h);
    setJourneyHero(h);
    live.current.hero = h;
    if (!h.data) setSpec(presetSpec(h.slug));
    if (live.current.phase === "running") {
      send(heroToken(h, toWorld(live.current.pos, live.current.doc), true));
      drawMat();
    }
  };

  // A token dragged or a door swung on the first screen.
  useEffect(() => watchMat(() => live.current.phase === "running" && drawMat()), [drawMat]);

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

