import { useEffect, useRef, useState } from "react";
import { describePlace, goalsOf } from "@thunderforge/encounter";
import EngineFrame from "../engine/EngineFrame.tsx";
import { CC_BY_SA, DEMO, MAPS_CATALOG, MAPS_SOURCE } from "../links.ts";
import ChestGlitter from "../room/ChestGlitter.tsx";
import RoomBoard from "../room/RoomBoard.tsx";
import RoomControls from "../room/RoomControls.tsx";
import GoalList from "../room/GoalList.tsx";
import { loadRoomScript, type LoadedRoom, type RoomName } from "../room/script.ts";
import { useRoom } from "../room/useRoom.ts";
import { AWAY, ENDING, ROAD_ON } from "../room/words.ts";
import { watchPage } from "../table/onPage.ts";
import "../room/room.css";

// Spec 091 US1 and US2: a tavern the hero walks through, then one small
// dungeon, on the engine when in view and in words always. The page plays
// the server's part with a scripted referee; nothing here is a game master.
// One engine serves both: the frame in view holds the table.

/** Each room's frame, and the still it shows before the engine draws it. */
const FRAMES: Record<RoomName, { id: string; poster: string; goals: string }> = {
  tavern: { id: "tavern-table", poster: "/tavern-table.webp", goals: "The tavern's goals" },
  dungeon: { id: "room-table", poster: "/room-table.webp", goals: "The dungeon's goals" },
};
/** FR-022: no room map is wider than this. */
const MAX_TEXTURE = 4096;

const mb = (n: number) => (n / 1e6).toFixed(1);

function Credit() {
  return (
    <>
      Map by MBRound18 (<a href={MAPS_SOURCE}>vtt-maps</a>,{" "}
      <a href={MAPS_CATALOG}>catalog</a>), <a href={CC_BY_SA}>CC BY-SA 4.0</a>.
    </>
  );
}

/** Down the page to the dungeon, the way the hero walks. */
function followDown() {
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  document.getElementById("room")?.scrollIntoView({ behavior: reduce ? "auto" : "smooth", block: "start" });
}

/** Spec 098 FR-008: a page whose table runs lays the dungeon on it, at any
 * width: a frame of its own would take the canvas, and the page's walker
 * with it, for screens either side of the room. */
function useOnPage(name: RoomName): boolean {
  const [on, setOn] = useState(false);
  useEffect(() => watchPage(setOn), []);
  return name === "dungeon" && on;
}

function RoomBody({ name, script, imageBytes }: LoadedRoom & { name: RoomName }) {
  const frame = FRAMES[name];
  const room = useRoom(script, frame.id);
  const onPage = useOnPage(name);
  const { state, said } = room;
  const goals = goalsOf(script);
  const done = state.goals.includes(goals[goals.length - 1]!);
  // FR-021: the keys act from the room's controls; a press on the table puts
  // focus there, so the arrows and WASD work right after a click on it.
  const controls = useRef<HTMLDivElement>(null);

  return (
    <div className="room-layout" data-square={`${state.hero.x},${state.hero.y}`}>
      <figure className="room-table" onPointerDown={() => controls.current?.focus({ preventScroll: true })}>
        {onPage ? (
          <RoomBoard script={script} room={room} />
        ) : (
          <EngineFrame
            id={frame.id}
            poster={{ src: frame.poster, width: 1200, height: 675 }}
            action="Play the room"
            auto
            away={AWAY}
            interactive
            maxTextureSize={MAX_TEXTURE}
            setup={room.frame.setup}
            teardown={room.frame.teardown}
            onState={room.frame.onState}
          >
            {room.glitter && <ChestGlitter {...room.glitter} />}
          </EngineFrame>
        )}
        <figcaption className="room-caption">
          {imageBytes !== null && <>The map is {mb(imageBytes)} MB. You play the hero walking the page. </>}
          <Credit />
        </figcaption>
      </figure>

      {/* A strip under the table in the page's own ink, small: the
          quest log on one side, the controls in the middle, the referee's
          words on the other. */}
      <div className="room-hud">
        <div className="room-hud-panel room-hud-quest">
          <GoalList goals={goals} met={state.goals} label={frame.goals} />
          <p className="room-place">{describePlace(script, state)}</p>
        </div>
        <div className="room-hud-panel room-hud-controls">
          <RoomControls
            ref={controls}
            script={script}
            state={state}
            doorPreview={room.doorPreview}
            rolling={room.rolling}
            onStep={room.step}
            onDoor={room.door}
            onRoll={() => void room.roll()}
            onStartOver={room.startOver}
          />
        </div>
        <div className="room-hud-panel room-hud-log">
          <h3 className="room-subhead">The referee says</h3>
          {/* Its own panel, so a first announcement moves nothing under the
              pointer. The alternate is a no-break space: the same words said
              twice still change the node. */}
          <p className="room-said" aria-live="polite">
            {said ? said.text + (said.n % 2 ? " " : "") : ""}
          </p>
          {!said && <p className="room-hint">Make a move, and the referee's answer lands here.</p>}
          {done && !script.chest && (
            <div className="room-ending">
              <p>{ROAD_ON}</p>
              <p>
                <button type="button" className="ink-btn ink-btn--small ink-btn--blue" onClick={followDown}>
                  Follow the path down
                </button>
              </p>
            </div>
          )}
          {done && script.chest && (
            <div className="room-ending">
              <p>{ENDING}</p>
              <p>
                <a className="ink-btn ink-btn--small ink-btn--blue" href={DEMO}>
                  Open the demo
                </a>
              </p>
              <p className="room-hint">
                <Credit />
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default function Room({ name = "dungeon" }: { name?: RoomName }) {
  const [loaded, setLoaded] = useState<LoadedRoom | null>(null);
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let live = true;
    setFailed(false);
    loadRoomScript(name).then(
      (r) => live && setLoaded(r),
      () => live && setFailed(true),
    );
    return () => {
      live = false;
    };
  }, [name, attempt]);

  if (failed) {
    return (
      <p className="room-hint">
        The room's map didn't load.{" "}
        <button type="button" className="ink-btn ink-btn--small ink-btn--line" onClick={() => setAttempt((n) => n + 1)}>
          Try again
        </button>{" "}
        Or <a href={DEMO}>play the whole map in the demo</a>.
      </p>
    );
  }
  if (!loaded) return <div className="room-layout room-layout--loading" />;
  return <RoomBody name={name} {...loaded} />;
}
