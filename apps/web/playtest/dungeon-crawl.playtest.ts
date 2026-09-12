import { writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import {
  activateAsPlayer,
  addLight,
  addWall,
  closeTable,
  crosses,
  doorStateOn,
  drag,
  expectAgreed,
  expectEveryoneLoaded,
  hiddenTokens,
  makeDoor,
  openTable,
  overview,
  placeCharacter,
  seededRandom,
  setAmbient,
  sitDown,
  snapshot,
  walk,
  type Point,
} from "./table";

/**
 * A Game Master runs a short dungeon crawl for two players, once in Genie and
 * once in D&D 5e: a dark crypt, a wall with a door in it, a goblin on the far
 * side, a torch, a lantern, and two heroes who move.
 *
 * What it is for is watching. Every step attaches what each seated client
 * showed, and the report carries a recording of each. The checks keep the run
 * honest about what it saw — hard where the product promises something, soft
 * (`expect.soft`, messages starting FINDING) where the run is looking for a
 * gap, so one finding does not end the session before the rest is seen.
 *
 * Laid out in world units around the origin, because players move by dragging
 * and `dragToken` aims at the default 1:1 camera: everything a player drags
 * stays inside their view. Replay a wandering run with `PLAYTEST_SEED`;
 * lengthen it with `PLAYTEST_ROUNDS`.
 */

const SEED = Number(process.env.PLAYTEST_SEED ?? 20260911);
const ROUNDS = Number(process.env.PLAYTEST_ROUNDS ?? 12);

/** One grid cell at the default zoom (see `dragToken`'s notes). */
const CELL = 128;
/** The crypt's east wall; its middle, |y| < DOOR, becomes a door. */
const WALL_X = 300;
const DOOR = 80;
const ARIA_START: Point = { x: 0, y: 0 };
const BROM_START: Point = { x: 0, y: -250 };
const GOBLIN: Point = { x: 700, y: 0 };
/** Where the heroes may wander: on screen, and on both sides of the wall. */
const BOUNDS = { minX: -450, maxX: 500, minY: -300, maxY: 300 };

const SOLID_WALLS: [Point, Point][] = [
  [
    { x: WALL_X, y: -500 },
    { x: WALL_X, y: -DOOR },
  ],
  [
    { x: WALL_X, y: DOOR },
    { x: WALL_X, y: 500 },
  ],
];

const DIRECTIONS: Point[] = [
  { x: CELL, y: 0 },
  { x: -CELL, y: 0 },
  { x: 0, y: CELL },
  { x: 0, y: -CELL },
];

for (const system of ["genie", "dnd5e"] as const) {
  test(`a Game Master runs a dungeon crawl for two players (${system})`, async ({
    page,
    browser,
  }, testInfo) => {
    test.setTimeout(900_000);
    testInfo.annotations.push({ type: "seed", description: String(SEED) });

    const table = await openTable({
      browser,
      gm: page,
      testInfo,
      system,
      players: ["Aria", "Brom"],
    });
    const [aria, brom] = table.players;

    try {
      let ariaToken = "";
      await test.step("Aria sits down and tries the keyboard", async () => {
        ariaToken = await placeCharacter(table, {
          label: "Aria",
          at: ARIA_START,
          seat: aria,
        });
        await sitDown(table, table.gm);
        await sitDown(table, aria.page);
        const before = await expectAgreed(table, ariaToken, "Aria is placed");

        await walk(aria, "east");
        await aria.page.waitForTimeout(1_500);
        const after = await expectAgreed(
          table,
          ariaToken,
          "whatever the key did, the table agrees on it",
        );
        expect
          .soft(
            after.x,
            "FINDING: D (east) should walk Aria's own token one cell. The " +
              "engine's keyboard movement drives the one entity tagged " +
              "`PlayerControlled`, and only the placeholder `setup_scene` " +
              "spawns at startup carries that tag (engine app.rs) — no " +
              "token a player owns ever does.",
          )
          .toBeGreaterThan(before.x);
        await snapshot(table, "0 · the keyboard");
      });

      let doorWall = "";
      let goblin = "";
      let bromToken = "";
      await test.step("the Game Master builds a dark crypt", async () => {
        for (const [from, to] of SOLID_WALLS) await addWall(table, from, to);
        doorWall = await addWall(
          table,
          { x: WALL_X, y: -DOOR },
          { x: WALL_X, y: DOOR },
        );
        goblin = await placeCharacter(table, {
          label: "Goblin",
          at: GOBLIN,
          tokenType: "npc",
        });
        await addLight(table, { x: -100, y: 0 }, 450);
        await addLight(table, ARIA_START, 200, { attachedTokenId: ariaToken });
        await setAmbient(table, "dark");

        bromToken = await placeCharacter(table, {
          label: "Brom",
          at: BROM_START,
          seat: brom,
        });
        await sitDown(table, brom.page);
        await expectEveryoneLoaded(table, { tokens: 3, walls: 3, lights: 2 });
        // Only the Game Master zooms out; players drag at 1:1.
        await overview(table.gm, 6);
        await snapshot(table, "1 · the crypt");
      });

      await test.step("the wall hides the goblin from the players, not the Game Master", async () => {
        for (const seat of [aria, brom]) {
          await expect
            .poll(() => hiddenTokens(seat.page), {
              timeout: 15_000,
              message: `the wall hides the goblin from ${seat.name}`,
            })
            .toContain(goblin);
        }
        expect(
          await hiddenTokens(aria.page),
          "Aria always sees herself",
        ).not.toContain(ariaToken);
        expect(
          await hiddenTokens(table.gm),
          "the Game Master sees through no token, so the wall hides nothing",
        ).not.toContain(goblin);
        await snapshot(table, "2 · behind the wall");
      });

      await test.step("Aria walks up to the door, and the table sees her go", async () => {
        const from = await expectAgreed(
          table,
          ariaToken,
          "Aria is where she was",
        );
        await drag(aria, ariaToken, { x: 200 - from.x, y: -from.y });
        const there = await expectAgreed(
          table,
          ariaToken,
          "Aria's move reaches the Game Master, Brom and the server",
        );
        expect(there.x, "Aria stops short of the wall").toBeLessThan(WALL_X);
        expect(there.x, "and has moved towards it").toBeGreaterThan(100);
        await snapshot(table, "3 · at the door");
      });

      await test.step("Aria tries to walk through the wall", async () => {
        // The doorway is not a door yet — it is a wall like the rest — so a
        // crossing anywhere along the line counts. (The first version judged
        // only the solid piece north of it, and the drag went through the
        // doorway's own wall at y=64 unjudged.)
        const eastWall: [Point, Point][] = [
          ...SOLID_WALLS,
          [
            { x: WALL_X, y: -DOOR },
            { x: WALL_X, y: DOOR },
          ],
        ];
        const from = await expectAgreed(
          table,
          ariaToken,
          "Aria is at the door",
        );
        await drag(aria, ariaToken, { x: 250, y: 2 * DOOR });
        const after = await expectAgreed(
          table,
          ariaToken,
          "the attempt reaches the whole table",
        );
        expect
          .soft(
            eastWall.some(([c, d]) => crosses(from, after, c, d)),
            "FINDING: a wall that blocks movement should stop Aria at it. " +
              "Nothing checks walls when a token moves: the server's " +
              "`moveOwnToken`/`updateToken` take the position as given, and " +
              "the route the engine sends (`pathCells`) is dropped by the " +
              "web sync.",
          )
          .toBe(false);
        await snapshot(table, "4 · through the wall?");
        // Back to the door, whatever happened, so the rest is about the door.
        const now = await expectAgreed(table, ariaToken, "Aria turns back");
        await drag(aria, ariaToken, { x: 200 - now.x, y: -now.y });
        const back = await expectAgreed(
          table,
          ariaToken,
          "Aria is at the door",
        );
        expect(back.x).toBeLessThan(WALL_X);
      });

      await test.step("Aria opens the door onto darkness", async () => {
        const door = await makeDoor(table, doorWall);
        await expect
          .poll(() => hiddenTokens(aria.page), { timeout: 15_000 })
          .toContain(goblin);
        // As a click does: through Aria's own client, which asks the server
        // and applies the opening to her board at once.
        expect(
          await activateAsPlayer(aria.page, door),
          "Aria opens the door",
        ).toBe("performed");
        // Her own board, then everyone else's. A door change is announced as
        // the wall change it is, so every board re-reads the wall and sees the
        // door — which is what spec 045 phase 1 fixed, and this holds it.
        for (const [who, client] of [
          ["Aria", aria.page],
          ["the Game Master", table.gm],
          ["Brom", brom.page],
        ] as const) {
          await expect
            .poll(() => doorStateOn(client, doorWall), {
              timeout: 10_000,
              message: `the door Aria opened is open on ${who}'s board`,
            })
            .toBe("open");
        }
        // Nothing lights the far side, so an open door alone should not show
        // the goblin: it stands in the dark, well beyond Aria's lantern.
        await aria.page.waitForTimeout(1_500);
        expect
          .soft(
            await hiddenTokens(aria.page),
            "the goblin stands in the dark beyond Aria's lantern",
          )
          .toContain(goblin);
        await snapshot(table, "5 · the door opens onto darkness");
      });

      await test.step("light decides what Aria sees through the door", async () => {
        await setAmbient(table, "bright");
        await expect
          .poll(() => hiddenTokens(aria.page), {
            timeout: 15_000,
            message: "in daylight, the open door shows Aria the goblin",
          })
          .not.toContain(goblin);
        await snapshot(table, "6 · daylight");

        await setAmbient(table, "dark");
        await expect
          .poll(() => hiddenTokens(aria.page), {
            timeout: 15_000,
            message: "back in the dark, the goblin is hidden again",
          })
          .toContain(goblin);

        await addLight(table, GOBLIN, 200);
        await expect
          .poll(() => hiddenTokens(aria.page), {
            timeout: 15_000,
            message:
              "a brazier by the goblin lets Aria see it through the open door",
          })
          .not.toContain(goblin);
        await snapshot(table, "7 · a brazier");
      });

      await test.step(`the heroes wander for ${ROUNDS} rounds (seed ${SEED})`, async () => {
        const random = seededRandom(SEED);
        const heroes = [
          { seat: aria, token: ariaToken },
          { seat: brom, token: bromToken },
        ];
        const where = new Map<string, Point>();
        for (const hero of heroes) {
          where.set(
            hero.token,
            await expectAgreed(
              table,
              hero.token,
              `${hero.seat.name} is placed`,
            ),
          );
        }
        const inBounds = (p: Point) =>
          p.x >= BOUNDS.minX &&
          p.x <= BOUNDS.maxX &&
          p.y >= BOUNDS.minY &&
          p.y <= BOUNDS.maxY;

        const log: unknown[] = [];
        const crossings: string[] = [];
        for (let round = 1; round <= ROUNDS; round += 1) {
          // A random direction, turned around if it would leave the screen.
          const moves = heroes.map((hero) => {
            const from = where.get(hero.token)!;
            const pick = DIRECTIONS[Math.floor(random() * DIRECTIONS.length)];
            const to = { x: from.x + pick.x, y: from.y + pick.y };
            return inBounds(to) ? pick : { x: -pick.x, y: -pick.y };
          });
          await Promise.all(
            heroes.map((hero, i) => drag(hero.seat, hero.token, moves[i])),
          );

          const entry: Record<string, unknown> = { round };
          for (const [i, hero] of heroes.entries()) {
            const before = where.get(hero.token)!;
            const after = await expectAgreed(
              table,
              hero.token,
              `round ${round}: ${hero.seat.name}'s move reaches the whole table`,
            );
            where.set(hero.token, after);
            entry[hero.seat.name] = { by: moves[i], at: after };
            if (SOLID_WALLS.some(([c, d]) => crosses(before, after, c, d))) {
              crossings.push(
                `round ${round}: ${hero.seat.name} went through a wall`,
              );
            }
            expect(
              await hiddenTokens(hero.seat.page),
              `round ${round}: ${hero.seat.name} always sees themself`,
            ).not.toContain(hero.token);
          }
          expect
            .soft(
              await hiddenTokens(table.gm),
              `round ${round}: the Game Master's canvas hides nothing`,
            )
            .toEqual([]);
          log.push(entry);
          if (round % 4 === 0 || round === ROUNDS) {
            await snapshot(table, `8 · round ${round}`);
          }
        }

        const record = JSON.stringify(
          { seed: SEED, rounds: log, crossings },
          null,
          2,
        );
        // Attached for the report, and written beside the run's other output
        // as a plain file: an attachment is only readable by unpacking the
        // report, and "where did they walk" is a question worth `cat`.
        await testInfo.attach("wandering log", {
          body: record,
          contentType: "application/json",
        });
        writeFileSync(testInfo.outputPath("wandering-log.json"), record);
        console.log(
          `[playtest] ${system}: ${ROUNDS} rounds, seed ${SEED}, ` +
            `${crossings.length} wall crossing(s)`,
        );
        expect
          .soft(
            crossings,
            "FINDING: heroes went through walls that block movement",
          )
          .toEqual([]);
      });
    } finally {
      await closeTable(table);
    }
  });
}
