import { useEffect, useState } from "react";
import { describePlace } from "@thunderforge/encounter";
import EngineFrame from "../engine/EngineFrame.tsx";
import { CC_BY_SA, DEMO, MAPS_CATALOG, MAPS_SOURCE } from "../links.ts";
import RoomControls from "../room/RoomControls.tsx";
import GoalList from "../room/GoalList.tsx";
import HeroPicker from "../room/HeroPicker.tsx";
import { loadRoomScript, type LoadedRoom } from "../room/script.ts";
import { useRoom } from "../room/useRoom.ts";
import { AWAY, ENDING } from "../room/words.ts";
import "../room/room.css";

// Spec 091 US1 and US2: one room of the Proving Ground, on the engine when
// the visitor asks for it, and in words always. The page plays the server's
// part with a scripted referee; nothing here is a game master.

const FRAME_ID = "room-table";
/** FR-022: the Proving Ground's background is 4095 pixels wide. */
const MAX_TEXTURE = 4096;

const mb = (n: number) => (n / 1e6).toFixed(1);

function Credit() {
  return (
    <>
      The Proving Ground, by MBRound18 (<a href={MAPS_SOURCE}>vtt-maps</a>,{" "}
      <a href={MAPS_CATALOG}>catalog</a>), <a href={CC_BY_SA}>CC BY-SA 4.0</a>.
    </>
  );
}

function RoomBody({ script, imageBytes }: LoadedRoom) {
  const room = useRoom(script, FRAME_ID);
  const { state, said } = room;
  const done = state.goals.includes("chest-opened");

  return (
    <div className="room-layout" data-square={`${state.hero.x},${state.hero.y}`}>
      <figure className="room-table">
        <EngineFrame
          id={FRAME_ID}
          poster={{ src: "/room-table.webp", width: 1200, height: 675 }}
          action="Play the room"
          away={AWAY}
          interactive
          maxTextureSize={MAX_TEXTURE}
          setup={room.frame.setup}
          teardown={room.frame.teardown}
          onState={room.frame.onState}
        />
        <figcaption className="room-caption">
          {imageBytes !== null && <>Playing loads the map, {mb(imageBytes)} MB, and the table if the dice haven't. </>}
          <Credit />
        </figcaption>
      </figure>

      <div className="room-side">
        <HeroPicker hero={room.hero} onPick={room.pick} />
        <GoalList met={state.goals} />
        <p className="room-place">{describePlace(script, state)}</p>
        <RoomControls
          script={script}
          state={state}
          doorPreview={room.doorPreview}
          rolling={room.rolling}
          onStep={room.step}
          onDoor={room.door}
          onRoll={() => void room.roll()}
          onStartOver={room.startOver}
        />
        {/* Below the controls, so a first announcement moves nothing under the
            pointer. */}
        <p className="room-said" aria-live="polite">
          {said ? said.text + (said.n % 2 ? " " : "") : ""}
        </p>
        {done && (
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
  );
}

export default function Room() {
  const [loaded, setLoaded] = useState<LoadedRoom | null>(null);
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let live = true;
    setFailed(false);
    loadRoomScript().then(
      (r) => live && setLoaded(r),
      () => live && setFailed(true),
    );
    return () => {
      live = false;
    };
  }, [attempt]);

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
  return <RoomBody {...loaded} />;
}
