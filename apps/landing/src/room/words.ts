import { describePlace, type Outcome, type RoomScript, type RoomState } from "@thunderforge/encounter";

// Spec 091: what the page says after each action, one line per outcome. The
// referee picks the outcome; this only puts it in words. Placeholder copy
// until the owner approves it (T061). FR-008: never "live", "connected",
// "online" or "multiplayer". The simulation sentence sits in `App.tsx`,
// outside the lazy chunk, so it is on the page before the room is.

export const ENDING =
  "That was the engine your table runs on. The rest of the Proving Ground is in the demo.";

/** While the dice tray has the table; the frame adds "Bring it back". */
export const AWAY = "The table is with the dice above.";

export const GOAL_LABELS = {
  "door-reached": "Walk to the door",
  "door-opened": "Open the door",
  "chest-found": "Find the chest",
  "chest-opened": "Open the chest",
} as const;

const doorName = (script: RoomScript, id: string) => script.doorNames[id] ?? "the door";

/** The announcement for an outcome, or null when nothing should be said. */
export function announce(script: RoomScript, state: RoomState, outcome: Outcome): string | null {
  const place = describePlace(script, state);
  const { formula, dc } = script.roll;
  const lastRoll = state.rolls[state.rolls.length - 1];
  switch (outcome.announcement) {
    case "moved":
      return place;
    case "blocked-by-wall":
      return "A wall. You stay where you are.";
    case "blocked-by-door":
      // The goal door teaches the rule; reaching it was counted on the way.
      return outcome.wallId === script.goalDoor && state.doors[outcome.wallId] === "closed"
        ? "The door is shut. Walls and closed doors refuse a move, as the server would."
        : `${cap(doorName(script, outcome.wallId ?? ""))} is shut.`;
    case "rolled-back":
      return `The rules refuse that move. Back to where you stood. ${place}`;
    case "door-opened":
      return `You open ${doorName(script, outcome.door?.id ?? "")}. ${place}`;
    case "door-closed":
      return `You close ${doorName(script, outcome.door?.id ?? "")}.`;
    case "chest-found":
      return `You find the chest. It is locked: roll ${formula} and meet ${dc} to open it.`;
    case "roll-failed":
      return `You rolled ${lastRoll?.total}. Not enough: you need ${dc}. Roll again.`;
    case "roll-succeeded":
      return `You rolled ${lastRoll?.total}. The chest opens.`;
    case "chest-already-open":
      return "The chest is already open.";
    case "reset":
      return `You are back where you started. ${place}`;
    case "no-door-in-reach":
    case "roll-too-soon":
      return outcome.reason ?? null;
    case "ignored":
      return null;
  }
}

const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
