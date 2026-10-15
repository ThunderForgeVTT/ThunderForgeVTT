import { expect, test, type Page } from "@playwright/test";
import { createReferee, type Direction, type Point } from "@thunderforge/encounter";
import { BOOT_TIMEOUT, BUILT } from "./support/engine.ts";
import { expectClean, openLanding } from "./support/page.ts";
import { bootRoom, engineHero, pad, recordSaid, room, saidLog, script, square } from "./support/room.ts";

/**
 * Spec 091 T041, FR-012 and SC-004: the rule and the engine agree on every
 * step. Node runs the same referee the page runs, from the same map file,
 * and predicts each step before the key reaches the canvas. The page's own
 * square and the engine's token must both land where Node said; the page's
 * `console.error` on a disagreement fails `expectClean` too.
 *
 * The dark room spans x -4.5 to 4.5 and y 0 to 4 in squares, open to the
 * north; the door to the hall is the one square of its south wall at x 2.
 * The walk meets the shut door, both jambs, the east and west walls, the
 * table's edge past the open north, the step in the south wall, then goes
 * through the door and meets the passage and the hall's ceiling.
 */

const S = "south";
const N = "north";
const E = "east";
const W = "west";
const times = (n: number, d: Direction) => Array<Direction>(n).fill(d);

const BEFORE_DOOR: Direction[] = [
  ...times(3, S), S, // to the door, then the shut door
  E, S, // the east jamb
  ...times(2, W), S, // the west jamb
  E, // back in front of the door
  ...times(2, E), E, // the east wall
  ...times(4, N), N, // out the open north, to the table's edge
  S, ...times(8, W), W, // the west wall
  ...times(2, S), S, // the step in the south wall
  ...times(4, E), S, S, // round the step, down to the south wall
  ...times(2, E), // in front of the door again
];
const AFTER_DOOR: Direction[] = [
  S, E, // through the open door, the passage wall
  S, W, N, // into the hall, its ceiling
  E, N, N, // back up into the room
];

const label: Record<Direction, string> = { north: "North", south: "South", east: "East", west: "West" };
const KEY: Record<Direction, string> = { north: "ArrowUp", south: "ArrowDown", east: "ArrowRight", west: "ArrowLeft" };
const at = (p: Point) => `${p.x},${p.y}`;

test.describe("the rule and the engine agree", () => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.beforeEach(async ({ page }, info) => {
    test.skip(info.project.name !== "desktop", "the engine boots at the desktop width only");
    test.setTimeout(BOOT_TIMEOUT + 600_000);
    await page.emulateMedia({ reducedMotion: "reduce" });
  });

  test("on every step of a walk into each wall and door (SC-004)", async ({ page }) => {
    const opened = await openLanding(page);
    const referee = createReferee(script);
    let state = referee.start();

    await bootRoom(page);
    await recordSaid(page);
    // Keys from inside the room's controls; a click waits on stable frames.
    await pad(page, "North").focus();

    let steps = 0;
    let refused = 0;
    const walk = async (dirs: Direction[]) => {
      for (const dir of dirs) {
        const judged = referee.step(state, dir);
        state = judged.state;
        steps += 1;
        if (!judged.outcome.accepted) refused += 1;
        const before = (await saidLog(page)).length;
        await page.keyboard.press(KEY[dir]);
        await expect.poll(async () => (await saidLog(page)).length, { message: `step ${steps} ${dir}` }).toBe(before + 1);
        const want = at(state.hero);
        expect(await square(page), `page, step ${steps} ${label[dir]}`).toBe(want);
        await expect
          .poll(async () => {
            const e = await engineHero(page);
            return e && at(e);
          }, { message: `engine, step ${steps} ${label[dir]}` })
          .toBe(want);
      }
    };

    await walk(BEFORE_DOOR);
    expect(at(state.hero)).toBe("2,0.5");

    const door = referee.toggleDoor(state);
    expect(door.outcome.accepted).toBe(true);
    state = door.state;
    await room(page).getByRole("button", { name: "Open the door to the hall" }).click();
    await pad(page, "North").focus();

    await walk(AFTER_DOOR);
    expect(at(state.hero)).toBe("2,0.5");

    test.info().annotations.push({ type: "SC-004", description: `${steps} steps, ${refused} refused, 0 disagreements` });
    expectClean(opened);
  });
});
