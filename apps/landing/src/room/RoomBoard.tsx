import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { HERO_ID, type RollTry, type RoomScript, type RoomState } from "@thunderforge/encounter";
import { canvasOf, seatGuest, sendAs, type Send } from "../engine/host.ts";
import { PAGE_TABLE } from "../table/onPage.ts";
import { leaveAt, publishBoard, watchSeat } from "./pageBoard.ts";
import { fromPage, toPage } from "./pageRoom.ts";
import type { useRoom } from "./useRoom.ts";

// Spec 098 FR-008: the dungeon on a wide page's table. The board lies on the
// page's grid, and the page's table draws the map there (`pageBoard.ts`).
// While the table seats the room, its commands go to the table, on the
// page's walker, and the walker's moves come back as the room's hero's.
// The still under it is for the moment before the engine draws.

const FRAME_ID = "room-table";
const REDUCE = "(prefers-reduced-motion: reduce)";
/** The engine's dice land this long after the throw (`dice_throw::TIMINGS`). */
const LANDS_MS = 1200;
const LANDS_REDUCED_MS = 150;
/** How long the total stays over the chest. */
const TOLD_MS = 3500;

type Room = ReturnType<typeof useRoom>;

export default function RoomBoard({ script, room }: { script: RoomScript; room: Room }) {
  const box = useRef<HTMLDivElement>(null);
  const map = useRef<HTMLDivElement>(null);
  const [seated, setSeated] = useState(false);
  const { gridSize, width, height } = script.map;
  const cols = width / gridSize;
  const rows = height / gridSize;
  /** The board's top-left on the page, and the page's cell. */
  const at = useRef({ x: 0, y: 0, cell: 48 });
  const now = useRef({ room, state: room.state });
  now.current = { room, state: room.state };

  /** A press in the room's controls: its scroll lands before its state does. */
  const touched = useRef(false);
  const played = (s: RoomState) =>
    touched.current ||
    s.goals.length > 0 || s.hero.x !== script.start.x || s.hero.y !== script.start.y;
  /** A square's centre on the page. */
  const onPage = (p: { x: number; y: number }) => {
    const { x, y, cell } = at.current;
    return { x: x + (p.x + cols / 2) * cell, y: y + (rows / 2 - p.y) * cell };
  };
  /** The room's controls go with the board: a click on them scrolls. */
  const lowest = () => {
    const r = box.current?.closest(".room-layout")?.getBoundingClientRect();
    return r ? r.bottom + window.scrollY : at.current.y + rows * at.current.cell;
  };
  const publish = () => {
    const { x, y, cell } = at.current;
    const start = onPage(script.start);
    publishBoard({
      src: script.scene.background,
      x,
      y,
      w: cols * cell,
      h: rows * cell,
      grid: gridSize,
      start,
      entry: { x: start.x, y: y - cell / 2 },
      bottom: lowest(),
      played: played(now.current.state),
    });
  };
  const latest = useRef(publish);
  latest.current = publish;

  // The board snaps to the page's grid lines, then says where it lies.
  useLayoutEffect(() => {
    const el = map.current;
    const outer = box.current;
    if (!el || !outer) return;
    const update = () => {
      const cell = parseFloat(getComputedStyle(el).getPropertyValue("--cell")) || 48;
      outer.style.translate = "";
      const r = el.getBoundingClientRect();
      const left = r.left + window.scrollX;
      const top = r.top + window.scrollY;
      const x = Math.round(left / cell) * cell;
      const y = Math.round(top / cell) * cell;
      outer.style.translate = `${x - left}px ${y - top}px`;
      at.current = { x, y, cell };
      el.style.width = `${cols * cell}px`;
      el.style.height = `${rows * cell}px`;
      latest.current();
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(document.body);
    void document.fonts?.ready.then(update);
    return () => {
      ro.disconnect();
      publishBoard(null);
    };
  }, [cols, rows]);

  // The page's table walks the hero on by when the room has not been played.
  const wasPlayed = played(room.state);
  useEffect(() => latest.current(), [wasPlayed]);

  // A visitor at the controls has played: said at once, before the press
  // scrolls the page and the table asks whether to walk the hero on.
  useEffect(() => {
    const layout = box.current?.closest(".room-layout");
    if (!layout) return;
    const press = () => {
      if (touched.current) return;
      touched.current = true;
      latest.current();
    };
    layout.addEventListener("pointerdown", press, true);
    layout.addEventListener("keydown", press, true);
    return () => {
      layout.removeEventListener("pointerdown", press, true);
      layout.removeEventListener("keydown", press, true);
    };
  }, []);

  // Seated and stood up by the page's table.
  useEffect(() => {
    let send: Send | null = null;
    const stand = () => {
      if (!send) return;
      const { room, state } = now.current;
      leaveAt(onPage(state.hero));
      room.frame.teardown(send);
      room.frame.onState("away", null);
      seatGuest(null);
      send = null;
      setSeated(false);
    };
    const off = watchSeat((place) => {
      stand();
      if (!place) return;
      const table = sendAs(PAGE_TABLE);
      const s: Send = (c) => {
        for (const out of toPage(c, place)) table(out);
      };
      send = s;
      seatGuest({
        id: FRAME_ID,
        hear: (e) => fromPage(e, place),
        token: (id) => (id === HERO_ID ? place.walker : id),
      });
      const { room } = now.current;
      room.frame.setup(s);
      room.frame.onState("running", s);
      setSeated(true);
    });
    return () => {
      off();
      stand();
    };
    // The room's callbacks are read through `now`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // The table lies behind the page, so the board hands its pointer to the
  // engine's canvas: a press on a door's icon, a drag of the hero, a hover.
  // Only the hero's square holds a finger; a swipe anywhere else scrolls.
  const pass = (e: React.PointerEvent<HTMLDivElement>) => {
    const canvas = canvasOf(FRAME_ID);
    // The board's own buttons keep their presses.
    if (!canvas || (e.target as Element).closest("button")) return;
    if (e.type === "pointerdown") e.currentTarget.setPointerCapture(e.pointerId);
    canvas.dispatchEvent(new PointerEvent(e.type, e.nativeEvent));
  };
  const { hero, goals } = room.state;
  // A found chest asks for its roll where it stands, so a phone need not
  // scroll down to the controls and back to see the throw.
  const chest = script.chest;
  const asks = seated && chest && goals.includes("chest-found") && !goals.includes("chest-opened");
  // The throw's total stands over the chest once the dice have landed, and
  // a miss asks again after it.
  const tries = room.state.rolls.length;
  const [told, setTold] = useState<RollTry | null>(null);
  useEffect(() => {
    const last = now.current.state.rolls[tries - 1];
    setTold(null);
    if (!last) return;
    const lands = matchMedia(REDUCE).matches ? LANDS_REDUCED_MS : LANDS_MS;
    const show = setTimeout(() => setTold(last), lands);
    const hide = setTimeout(() => setTold(null), lands + TOLD_MS);
    return () => {
      clearTimeout(show);
      clearTimeout(hide);
    };
  }, [tries]);
  const overChest = chest && {
    top: `calc(var(--cell) * ${rows / 2 - chest.square.y - 1})`,
    ...(chest.square.x > 0
      ? { right: `calc(var(--cell) * ${cols / 2 - chest.square.x - 0.5})` }
      : { left: `calc(var(--cell) * ${chest.square.x + cols / 2 - 0.5})` }),
  };

  return (
    <div className="room-board-box" ref={box}>
      <div
        id={FRAME_ID}
        ref={map}
        className={`room-board${seated ? " engine-frame--running" : ""}`}
        data-testid="room-board"
        {...(seated && {
          onPointerDown: pass,
          onPointerMove: pass,
          onPointerUp: pass,
          onPointerCancel: pass,
        })}
      >
        <img className="room-board-art" src={script.scene.background} alt="" width={width} height={height} />
        {seated && overChest && told && (
          // The room's words say it too; this is the picture of it.
          <p
            className={`room-board-told room-board-told--${told.success ? "hit" : "miss"}`}
            data-testid="room-board-told"
            aria-hidden="true"
            style={overChest}
          >
            <strong>{told.total}</strong> vs DC {told.dc} · {told.success ? "it opens!" : "not enough"}
          </p>
        )}
        {asks && !told && (
          <button
            type="button"
            className="ink-btn ink-btn--small ink-btn--red room-board-roll"
            // Over the chest's name, held to the board's side the chest is on.
            style={overChest || undefined}
            aria-disabled={room.rolling}
            onClick={() => !room.rolling && void room.roll()}
          >
            {tries ? "Roll again" : "Roll to open"}
          </button>
        )}
        {seated && (
          <div
            className="room-board-hero"
            data-testid="room-board-hero"
            aria-hidden="true"
            style={{
              left: `calc(var(--cell) * ${hero.x + cols / 2 - 0.5})`,
              top: `calc(var(--cell) * ${rows / 2 - hero.y - 0.5})`,
            }}
          />
        )}
      </div>
    </div>
  );
}
