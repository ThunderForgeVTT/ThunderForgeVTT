import type { KeyboardEvent } from "react";
import type { Direction, Outcome, RoomScript, RoomState } from "@thunderforge/encounter";
import { directionOf } from "../engine/keys.ts";

// Spec 091 FR-020 and FR-021: the room without the canvas. Every control
// works whether or not the engine runs; the arrows and WASD act only while
// focus is inside these controls, never anywhere else on the page.

interface Props {
  script: RoomScript;
  state: RoomState;
  doorPreview: Outcome;
  rolling: boolean;
  onStep: (dir: Direction) => void;
  onDoor: () => void;
  onRoll: () => void;
  onStartOver: () => void;
}

const PAD: Array<{ dir: Direction; label: string; glyph: string }> = [
  { dir: "north", label: "North", glyph: "↑" },
  { dir: "west", label: "West", glyph: "←" },
  { dir: "east", label: "East", glyph: "→" },
  { dir: "south", label: "South", glyph: "↓" },
];

export default function Controls(props: Props) {
  const { script, state, doorPreview, rolling } = props;
  const { formula, dc } = script.roll;
  const found = state.goals.includes("chest-found");
  const opened = state.goals.includes("chest-opened");
  const last = state.rolls[state.rolls.length - 1];

  function keys(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement) return;
    const dir = directionOf(e);
    if (!dir || e.altKey || e.ctrlKey || e.metaKey) return;
    e.preventDefault();
    props.onStep(dir);
  }

  const door = doorPreview.door;
  const doorName = door ? (script.doorNames[door.id] ?? "the door") : null;

  return (
    <div className="room-controls" onKeyDown={keys}>
      <div className="room-pad" role="group" aria-label="Move one square">
        {PAD.map((p) => (
          <button
            key={p.dir}
            type="button"
            className={`room-pad-btn room-pad-btn--${p.dir}`}
            onClick={() => props.onStep(p.dir)}
          >
            <span aria-hidden="true">{p.glyph}</span> {p.label}
          </button>
        ))}
      </div>
      <p className="room-hint">Arrow keys and WASD move too, while focus is on these controls.</p>

      <div className="room-actions">
        <button type="button" className="ink-btn ink-btn--small ink-btn--blue-line" disabled={!door} onClick={props.onDoor}
          aria-describedby={door ? undefined : "room-door-why"}>
          {door ? `${door.state === "open" ? "Open" : "Close"} ${doorName}` : "Open the door"}
        </button>
        {!door && (
          <p id="room-door-why" className="room-hint">
            {doorPreview.reason}
          </p>
        )}
      </div>

      {found && (
        <div className="room-roll">
          <p>
            The chest is locked. Roll <code>{formula}</code> and meet DC {dc} to open it.
          </p>
          {/* aria-disabled, not disabled: a disabled button drops focus to
              the page, so Enter after a throw would do nothing. */}
          <button
            type="button"
            className="ink-btn ink-btn--small ink-btn--red"
            aria-disabled={rolling || opened}
            onClick={() => !rolling && !opened && props.onRoll()}
          >
            Roll {formula}
          </button>
          {last && (
            <p className="room-roll-result">
              Last roll: <strong>{last.total}</strong> against DC {last.dc}, {last.success ? "opened" : "not enough"}.
            </p>
          )}
        </div>
      )}

      <button type="button" className="ink-btn ink-btn--small ink-btn--line" onClick={props.onStartOver}>
        Start over
      </button>
    </div>
  );
}
