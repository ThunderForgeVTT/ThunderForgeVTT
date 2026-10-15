/**
 * Spec 091: where the hero stands, in words.
 *
 * The room can be played without seeing the board. These sentences say what
 * a sighted player reads off it: the room or hall the hero is in, the
 * nearest door and its state, and the chest when it is close.
 */

import { toWorld } from "./map.ts";
import type { RoomState } from "./referee.ts";
import { inRect, type RoomScript } from "./script.ts";
import { distanceToSegment, doorInReach, type Point } from "./walls.ts";

/** The chest is mentioned within this many squares. */
const CHEST_RANGE = 5;

const WORDS = [
  "no",
  "one",
  "two",
  "three",
  "four",
  "five",
  "six",
  "seven",
  "eight",
  "nine",
  "ten",
  "eleven",
  "twelve",
];

const squares = (n: number): string =>
  `${WORDS[n] ?? String(n)} square${n === 1 ? "" : "s"}`;

const COMPASS = [
  "east",
  "north-east",
  "north",
  "north-west",
  "west",
  "south-west",
  "south",
  "south-east",
];

/** The compass word for the way from `from` to `to`, in eight sectors. */
function heading(script: RoomScript, from: Point, to: Point): string {
  const east = to.x - from.x;
  const north = (to.y - from.y) * script.north;
  const turns = Math.atan2(north, east) / (Math.PI / 4);
  return COMPASS[(Math.round(turns) + 8) % 8]!;
}

const distance = (a: Point, b: Point): number =>
  Math.hypot(b.x - a.x, b.y - a.y);

function placeSentence(script: RoomScript, hero: Point): string {
  const area = script.areas.find((a) => inRect(hero, a.bounds));
  return area ? `You stand in ${area.phrase}.` : "You stand in the dark.";
}

function doorSentence(script: RoomScript, state: RoomState): string | null {
  const grid = script.map.gridSize;
  const center = toWorld(state.hero, grid);
  const doors = script.map.walls
    .filter((w) => w.id in state.doors)
    .map((w) => ({
      w,
      gap: distanceToSegment(
        center,
        { x: w.x1, y: w.y1 },
        { x: w.x2, y: w.y2 },
      ),
    }))
    .sort((a, b) => a.gap - b.gap);
  const nearest = doors[0]?.w;
  if (!nearest) return null;

  const name = script.doorNames[nearest.id] ?? "a door";
  const Name = name.charAt(0).toUpperCase() + name.slice(1);
  const middle = {
    x: (nearest.x1 + nearest.x2) / 2 / grid,
    y: (nearest.y1 + nearest.y2) / 2 / grid,
  };
  const way = heading(script, state.hero, middle);
  const shut = state.doors[nearest.id] === "open" ? "open" : "shut";
  if (doorInReach(nearest, { center, side: grid }, grid)) {
    return `${Name} is within reach to the ${way}, ${shut}.`;
  }
  return `${Name} is ${squares(Math.round(distance(state.hero, middle)))} ${way}, ${shut}.`;
}

function chestSentence(script: RoomScript, state: RoomState): string | null {
  if (!script.chest) return null;
  const chest = script.chest.square;
  const far = distance(state.hero, chest);
  if (far > CHEST_RANGE) return null;
  const opened = state.goals.includes("chest-opened");
  const article = opened ? "An open chest" : "A chest";
  const steps = Math.round(far);
  if (steps === 0) return `${article} is here.`;
  return `${article} is ${squares(steps)} ${heading(script, state.hero, chest)}.`;
}

/**
 * The hero's place in words. For example: "You stand in a dark room. The
 * door to the hall is four squares north, shut."
 */
export function describePlace(script: RoomScript, state: RoomState): string {
  return [
    placeSentence(script, state.hero),
    doorSentence(script, state),
    chestSentence(script, state),
  ]
    .filter((s): s is string => s !== null)
    .join(" ");
}
