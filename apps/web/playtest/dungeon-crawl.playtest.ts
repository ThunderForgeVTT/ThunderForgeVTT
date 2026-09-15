import { writeFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import {
  activateAsPlayer,
  addLight,
  addWall,
  closeTable,
  crosses,
  doorStateOn,
  drag,
  tryDrag,
  expectAgreed,
  expectEveryoneLoaded,
  hiddenTokens,
  joinLate,
  markedTokens,
  makeDoor,
  movementStateOn,
  openTable,
  overview,
  placeCast,
  placeCharacter,
  seededRandom,
  setAmbient,
  sitDown,
  snapshot,
  walk,
  type Point,
} from "./table";
import { setTraits } from "./combat";

/**
 * A Game Master runs a short dungeon crawl for two players, once in Genie and
 * once in D&D 5e: a dark crypt, a wall with a door in it, a goblin on the far
 * side, a torch, a lantern, and two heroes who move.
 *
 * What it is for is watching. Every step attaches what each seated client
 * showed, and the report carries a recording of each. The checks keep the run
 * honest about what it saw.
 *
 * This scenario has no FINDINGs left. Its first run (2026-09-11) recorded
 * seven, as soft checks: a movement key moved nothing, a drag went through a
 * wall, the opened door stayed shut on all three boards, and so daylight and a
 * brazier showed nothing through it. Spec 045 fixed each, and each is now a
 * hard check. Two checks are still `expect.soft` — the goblin staying dark
 * beyond the lantern, and the Game Master's board hiding nothing on each
 * wandering round — only so one miss does not end the session before the rest
 * is seen; a soft miss still fails the run. A new gap goes in as a soft check
 * whose message starts FINDING, as both scenarios' first gaps did.
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
/** Behind the wall from both heroes, and carrying a lamp of its own. */
const SENTRY: Point = { x: 500, y: 200 };
/** On the heroes' own side, beyond every placed light: hidden by dark alone. */
const WRAITH: Point = { x: -430, y: 330 };
/**
 * Near Brom, and beyond every light the Game Master places: more than 450 from
 * the torch at (-100, 0), and far from the brazier, the sentry's lamp and
 * Aria's lantern. Only a light Brom carries can reach it.
 */
const BAT: Point = { x: 150, y: -450 };
/**
 * The light on Brom's sheet (D&D 5e `traitData`, spec 045 T065). Sized to the
 * crypt rather than the rulebook: this scene keeps the server's default grid
 * of 5 units a square, so a foot is one world unit and a real torch's forty
 * feet would not reach past Brom's own token.
 */
const BROM_TORCH = { light_bright: 200, light_dim: 400 };
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
      await test.step("Aria sits down and walks with the keyboard", async () => {
        ariaToken = await placeCharacter(table, {
          label: "Aria",
          at: ARIA_START,
          seat: aria,
        });
        await sitDown(table, table.gm);
        await sitDown(table, aria.page);
        const before = await expectAgreed(table, ariaToken, "Aria is placed");

        await walk(aria, "east");
        const after = await expectAgreed(
          table,
          ariaToken,
          "Aria's step reaches the Game Master and the server",
        );
        expect(
          after.x,
          `D walks Aria's own token one cell east (engine: ${await movementStateOn(aria.page)})`,
        ).toBeGreaterThan(before.x);
        await snapshot(table, "0 · the keyboard");
      });

      let doorWall = "";
      let goblin = "";
      let wraith = "";
      let bromToken = "";
      let bromActor = "";
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

        const bromCast = await placeCast(table, {
          label: "Brom",
          at: BROM_START,
          seat: brom,
        });
        bromToken = bromCast.tokenId;
        bromActor = bromCast.actorId;
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
        // FR-033: and the goblin is *marked* for them, because the players
        // cannot see it. Not the same claim as the line above — that one says
        // the Game Master keeps the token, this one says they are told the
        // table has lost it.
        await expect
          .poll(() => markedTokens(table.gm), {
            timeout: 15_000,
            message: "the Game Master is shown what the party cannot see",
          })
          .toContain(goblin);
        expect(
          await markedTokens(aria.page),
          "a player is never told what anyone else can see",
        ).toEqual([]);
        await snapshot(table, "2 · behind the wall");
      });

      await test.step("a token in darkness alone is hidden, with no wall involved", async () => {
        // FR-031, which the crawl has never covered: everything above is
        // about the wall. This puts a token on the players' *own* side of it,
        // out of every light, so the only thing that can hide it is the dark.
        // Placed beyond the torch's 450 reach — WRAITH is ~467 away from it
        // — and nowhere near Aria's carried lantern. Inside the radius the
        // step would pass without darkness doing anything.
        wraith = await placeCharacter(table, {
          label: "Wraith",
          at: WRAITH,
          tokenType: "npc",
        });
        for (const seat of [aria, brom]) {
          await expect
            .poll(() => hiddenTokens(seat.page), {
              timeout: 15_000,
              message: `darkness alone hides the wraith from ${seat.name}`,
            })
            .toContain(wraith);
        }
        await snapshot(table, "2a · out of the light");
      });

      await test.step("a player with no token of their own sees the lit board", async () => {
        // FR-035 and decision 4: such a player sees the board *as it is lit*,
        // with no line of sight applied. Both halves of that matter, so this
        // needs a token that is lit *and* behind the wall.
        //
        // A sentry with a lamp of its own, rather than the goblin: the goblin
        // stands in the dark on purpose, and later steps turn on its light to
        // say so. Lighting it here would quietly delete that story.
        //
        // Unlit, this step would prove nothing — the token would be hidden
        // from Carl by darkness and from the players by the wall, and two
        // different rules reaching the same answer is not evidence about
        // either. Lit, the wall is the only thing left that can hide it.
        const sentry = await placeCharacter(table, {
          label: "Sentry",
          at: SENTRY,
          tokenType: "npc",
        });
        await addLight(table, SENTRY, 200);
        const carl = await joinLate(table, "Carl");
        await sitDown(table, carl.page);
        await expect
          .poll(
            () =>
              carl.page.evaluate(
                () => window.__worldProbe?.state()?.counts.tokens ?? 0,
              ),
            { timeout: 20_000, message: "Carl's board loads the scene" },
          )
          .toBeGreaterThan(0);

        await expect
          .poll(() => hiddenTokens(carl.page), {
            timeout: 15_000,
            message:
              "a player with no token has no point of view, so a lit token " +
              "behind a wall is on their board (FR-035)",
          })
          .not.toContain(sentry);
        await expect
          .poll(() => hiddenTokens(aria.page), {
            timeout: 15_000,
            message:
              "and the wall still hides the same lit sentry from a player " +
              "who does have a token",
          })
          .toContain(sentry);
        await snapshot(table, "2b · a player with no token");
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
        const moved = await tryDrag(aria, ariaToken, { x: 250, y: 2 * DOOR });
        const after = await expectAgreed(
          table,
          ariaToken,
          "the attempt reaches the whole table",
        );
        expect(
          eastWall.some(([c, d]) => crosses(from, after, c, d)),
          "a wall that blocks movement stops Aria at it — the engine returns " +
            "her drag to where it began, and the server refuses the move if " +
            "any client sends it anyway (spec 045 US2)",
        ).toBe(false);
        // Said separately from the crossing check above, because they can
        // fail apart: a drag that moved her *somewhere* without crossing
        // would satisfy the first and not this, and would mean the wall
        // deflected the move rather than refusing it.
        expect(
          moved,
          "the drag leaves Aria exactly where it began (FR-015)",
        ).toBe(false);
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

      await test.step("Aria's lantern travels with her", async () => {
        // FR-042. The scene is dark and the brazier is off again, so the only
        // light near the wraith is the one Aria carries — which means the
        // wraith is visible exactly when Aria has walked close enough, and
        // hidden again when she leaves. A light that stayed where it was
        // placed would never reach it at all.
        await setAmbient(table, "dark");
        await expect
          .poll(() => hiddenTokens(aria.page), {
            timeout: 15_000,
            message: "the wraith is in the dark before Aria walks over",
          })
          .toContain(wraith);

        const before = await expectAgreed(table, ariaToken, "Aria is at rest");
        await drag(aria, ariaToken, {
          x: WRAITH.x + 120 - before.x,
          y: WRAITH.y - before.y,
        });
        await expectAgreed(table, ariaToken, "Aria reaches the wraith");
        await expect
          .poll(() => hiddenTokens(aria.page), {
            timeout: 15_000,
            message:
              "Aria's lantern lights from wherever she is, so walking to the " +
              "wraith reveals it (FR-042)",
          })
          .not.toContain(wraith);
        await snapshot(table, "7a · the lantern travels");
      });

      await test.step("Brom's torch, from his sheet, lights the board for Aria", async () => {
        // Spec 045 FR-061 and FR-064, and the owner's decision of 2026-09-14:
        // a light a game system says a character carries is a light attached
        // to that character's token. So it lights the board for every seat —
        // this watches Aria's, not Brom's — and nobody placed it: the sheet
        // did. Genie declares no carried light, so its crawl has nothing to
        // light here and says so.
        if (system !== "dnd5e") {
          testInfo.annotations.push({
            type: "carried light",
            description: `${system} declares no carried light`,
          });
          return;
        }
        const bat = await placeCharacter(table, {
          label: "Bat",
          at: BAT,
          tokenType: "npc",
        });
        await expect
          .poll(() => hiddenTokens(aria.page), {
            timeout: 15_000,
            message: "the bat is in the dark before Brom lights his torch",
          })
          .toContain(bat);

        await setTraits(table, bromActor, {
          class: "fighter",
          level: 3,
          ...BROM_TORCH,
        });
        await expect
          .poll(() => carriedLightOn(aria.page, bromToken), {
            timeout: 20_000,
            message:
              "the torch on Brom's sheet reaches Aria's engine as a light on " +
              "Brom's token (T065)",
          })
          .toMatchObject({ bright: 200, dim: 400 });
        await expect
          .poll(() => hiddenTokens(aria.page), {
            timeout: 15_000,
            message:
              "Brom's torch lights the bat on Aria's board — a carried light " +
              "is the table's light, not only its bearer's (T065)",
          })
          .not.toContain(bat);
        await snapshot(table, "7b · Brom's torch");
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
          // `tryDrag`: a random walk aimed at the crypt wall is *meant* to be
          // refused now, and a refused drag leaves the token exactly where it
          // was — which `drag` treats as a missed grab and retries. Whether
          // each move landed does not need asserting here; the crossing check
          // below is the point, and it is stricter.
          await Promise.all(
            heroes.map((hero, i) => tryDrag(hero.seat, hero.token, moves[i])),
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
        // Hard, and the more demanding of the two wall checks: the aimed
        // attempt above is one drag at a known wall, while this is however
        // many random moves the seed produced, from wherever the heroes had
        // wandered to. A wall that only holds when it is being aimed at is
        // not a wall.
        expect(
          crossings,
          "no hero walked through a wall that blocks movement",
        ).toEqual([]);
      });
    } finally {
      await closeTable(table);
    }
  });
}

/** The light `tokenId` carries on this board's engine, or `null` (T065). */
async function carriedLightOn(
  page: Page,
  tokenId: string,
): Promise<{ x: number; y: number; bright: number; dim: number } | null> {
  return page.evaluate(
    (id) =>
      (
        window as unknown as {
          __engineProbe?: {
            carriedLights?: () => {
              tokenId: string;
              x: number;
              y: number;
              bright: number;
              dim: number;
            }[];
          };
        }
      ).__engineProbe
        ?.carriedLights?.()
        .find((light) => light.tokenId === id) ?? null,
    tokenId,
  );
}
