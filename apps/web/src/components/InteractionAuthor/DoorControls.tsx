import { useCallback, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Panel } from "@/components/ui/panel/Panel";
import {
  setDoorDesignation,
  setDoorLock,
  setDoorSecret,
} from "@/api/interactives";
import type { WorldWall } from "@/engine/world/types";

/**
 * The Game Master's controls for one door: shut it, lock it, hide it.
 *
 * Spec 030, US2 and US4 (FR-023). Meant to be shown from the canvas's
 * secondary interaction — a right-click on a door — which is where the spec
 * puts "shut and lock", because it is a thing the GM does mid-scene and not a
 * thing they open a panel for.
 *
 * # Why lock is a checkbox and not a third state
 *
 * Open, Closed and Locked as one choice makes "open, and players cannot close
 * it" — a spiked-open portcullis — inexpressible, and forces a decision about
 * what happens to the lock when a GM opens a locked door that a separate flag
 * simply never raises. So state and lock are two controls, and both are shown
 * at once because a GM needs to see both to know what the door will do.
 *
 * None of these are the security boundary. Every one is refused server-side
 * for anyone who does not run the world; this component only decides what to
 * draw.
 */

export interface DoorControlsProps {
  wall: WorldWall;
  /** Called after any change, so the caller can re-read the scene. */
  onChanged?: () => void;
}

export function DoorControls({ wall, onChanged }: DoorControlsProps) {
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);

  const isDoor = wall.doorState !== "none";
  const locked = wall.locked === true;
  const secret = wall.secret === true;

  const run = useCallback(
    async (action: () => Promise<unknown>) => {
      setBusy(true);
      setProblem(null);
      try {
        await action();
        onChanged?.();
      } catch {
        // Said out loud rather than swallowed: a control that silently did
        // nothing is indistinguishable from a broken one, and a GM mid-scene
        // has no way to tell which they are looking at.
        setProblem("That did not go through.");
      } finally {
        setBusy(false);
      }
    },
    [onChanged],
  );

  return (
    <Panel>
      <h3>Door</h3>

      <Button
        disabled={busy}
        onClick={() => run(() => setDoorDesignation(wall.id, !isDoor))}
      >
        {isDoor ? "Make it an ordinary wall" : "Make this a door"}
      </Button>

      {isDoor && (
        <>
          <Button
            disabled={busy}
            onClick={() => run(() => setDoorLock(wall.id, !locked))}
          >
            {locked ? "Unlock" : "Lock"}
          </Button>

          <Button
            disabled={busy}
            onClick={() => run(() => setDoorSecret(wall.id, !secret))}
          >
            {secret ? "Show it to the table" : "Hide it from the table"}
          </Button>

          <p>
            {wall.doorState === "open" ? "Open" : "Closed"}
            {locked ? ", locked" : ""}
            {secret ? ", hidden from the table" : ""}.
          </p>
        </>
      )}

      {problem && <p role="alert">{problem}</p>}
    </Panel>
  );
}
