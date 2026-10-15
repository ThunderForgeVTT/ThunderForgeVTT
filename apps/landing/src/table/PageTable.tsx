import { useCallback, useEffect, useRef, useState } from "react";
import { holderId, sendAs, take, type Send } from "../engine/host.ts";
import type { Progress } from "../engine/loadEngine.ts";
import { PARTY, type PartyHero } from "../room/party.ts";
import "./page-table.css";
import { along, fraction, pathDown, toWorld, wallsOf, type Box, type Doc, type Pt } from "./journey.ts";

// Prototype: the whole page on the engine's table.
//
// One engine canvas fills the viewport behind the page. The sections are
// walled blocks on it; the hero, picked or drawn by lot, walks a path down
// the page as the visitor scrolls, and goes where a tap on the table says.
// The camera is locked to the page, so the engine's grid is the page's grid.
//
// Touch first: a tap on the mat walks the hero there, on any screen.

const PAGE = "page-table";
const WALKER = "walker";
const REDUCE = "(prefers-reduced-motion: reduce)";
/** What a tap on counts as the table, not the page's words. */
const SURFACES = "html, body, main, header.hero, .hero-body, .section";

type Phase = "idle" | "held" | "loading" | "running" | "away" | "failed";

const mb = (n: number) => (n / 1e6).toFixed(0);

function cellSize(): number {
  const v = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--cell"));
  return Number.isFinite(v) && v > 0 ? v : 48;
}

function measure(): { boxes: Box[]; doc: Doc } {
  const doc = { w: document.documentElement.clientWidth, h: document.documentElement.scrollHeight };
  const boxes: Box[] = [];
  document.querySelectorAll<HTMLElement>("[data-section], #room").forEach((el) => {
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

function heroToken(hero: PartyHero, at: Pt) {
  return {
    type: "upsert_token",
    token: { id: WALKER, x: at.x, y: at.y, z: 10, label: hero.name, tokenType: "character", photoUrl: hero.art },
  };
}

export default function PageTable() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [why, setWhy] = useState("");
  const [progress, setProgress] = useState<Progress | null>(null);
  const [hero, setHero] = useState<PartyHero>(() => PARTY[Math.floor(Math.random() * PARTY.length)]!);
  const [pct, setPct] = useState(0);
  const [open, setOpen] = useState(false);
  const stage = useRef<HTMLDivElement>(null);

  // What the frame loop and the listeners read; never a stale render's copy.
  const live = useRef({
    phase: "idle" as Phase,
    hero: PARTY[0]!,
    doc: { w: 1, h: 1 } as Doc,
    path: [] as Pt[],
    wallIds: [] as string[],
    pos: { x: 0, y: 0 } as Pt,
    target: { x: 0, y: 0 } as Pt,
    free: false,
    cell: 48,
  });
  live.current.hero = hero;

  const send = useRef<Send>(sendAs(PAGE)).current;

  const go = useCallback((next: Phase) => {
    live.current.phase = next;
    setPhase(next);
    document.body.classList.toggle("in-engine", next === "running");
  }, []);

  /** The camera over the viewport, exactly: the grid is the page's grid. */
  const aim = useCallback(() => {
    const el = stage.current;
    if (!el) return;
    const { doc } = live.current;
    const c = toWorld({ x: el.clientWidth / 2, y: window.scrollY + el.clientHeight / 2 }, doc);
    send({
      type: "fit_camera_to",
      centerX: c.x,
      centerY: c.y,
      width: el.clientWidth,
      height: el.clientHeight,
      immediate: true,
    });
  }, [send]);

  /** Measures the page and sends it as a scene: grid, mat, walls. */
  const layout = useCallback(() => {
    const L = live.current;
    const { boxes, doc } = measure();
    L.cell = cellSize();
    L.doc = doc;
    L.path = pathDown(boxes, doc, L.cell);
    for (const id of L.wallIds) send({ type: "remove_wall", wallId: id });
    const walls = wallsOf(boxes, doc);
    L.wallIds = walls.map((w) => w.id);
    send({
      type: "set_scene_grid",
      gridType: "square",
      size: L.cell,
      mapWidth: doc.w,
      mapHeight: doc.h,
      visible: true,
    });
    send({ type: "set_scene_background", backgroundImagePath: "/table/mat.png", width: doc.w, height: doc.h });
    for (const wall of walls) send({ type: "upsert_wall", wall });
    if (!L.free) L.target = along(L.path, fraction(window.scrollY, doc, window.innerHeight));
    aim();
  }, [send, aim]);

  const setup = useCallback(
    (s: Send) => {
      const L = live.current;
      layout();
      L.pos = { ...L.target };
      s(heroToken(L.hero, toWorld(L.pos, L.doc)));
      s({ type: "set_controlled_token", tokenId: WALKER });
      s({ type: "set_scene_playing", playing: true });
      aim();
    },
    [layout, aim],
  );

  const teardown = useCallback((s: Send) => {
    const L = live.current;
    s({ type: "set_scene_playing", playing: false });
    s({ type: "remove_token", tokenId: WALKER });
    for (const id of L.wallIds) s({ type: "remove_wall", wallId: id });
    L.wallIds = [];
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
    const onScroll = () => {
      const L = live.current;
      const doc = { w: document.documentElement.clientWidth, h: document.documentElement.scrollHeight };
      setPct(Math.round(fraction(window.scrollY, doc, window.innerHeight) * 100));
      const t = fraction(window.scrollY, L.doc, window.innerHeight);
      if (L.phase === "away") {
        const holder = holderId();
        const el = holder ? document.getElementById(holder) : null;
        const r = el?.getBoundingClientRect();
        if (r && (r.bottom < -window.innerHeight || r.top > 2 * window.innerHeight)) void boot();
        return;
      }
      if (L.phase !== "running") return;
      L.free = false;
      L.target = along(L.path, t);
      aim();
    };
    window.addEventListener("scroll", onScroll, { passive: true });
    onScroll();
    return () => window.removeEventListener("scroll", onScroll);
  }, [aim, boot]);

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
      const speed = Math.max(3, Math.min(dist, dist * 0.12));
      L.pos = { x: L.pos.x + (dx / dist) * speed, y: L.pos.y + (dy / dist) * speed };
      send(heroToken(L.hero, toWorld(L.pos, L.doc)));
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, [send]);

  const pick = (h: PartyHero) => {
    setHero(h);
    live.current.hero = h;
    if (live.current.phase === "running") send(heroToken(h, toWorld(live.current.pos, live.current.doc)));
  };

  const status =
    phase === "loading"
      ? progress
        ? `Setting the table: ${mb(progress.loaded)} of ${mb(progress.total)} MB`
        : "Setting the table"
      : phase === "running"
        ? "Tap the mat to walk there."
        : phase === "away"
          ? "The table is in the frame above."
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
        className={`journey${open ? " journey--open" : ""}`}
        role="progressbar"
        aria-label="How far down the page"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={pct}
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
          <div className="journey-track">
            <div className="journey-ink" />
          </div>
          <p className="journey-status">{status}</p>
        </div>
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
        </ul>
      </aside>
    </>
  );
}
