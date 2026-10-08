import { expect, test, type Page } from "./fixtures/test";
import { type GqlResult } from "./fixtures/admin";
import {
  clickPlay,
  freshCredentials,
  graphql,
  register,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import {
  diceEntities,
  diceLanded,
  diceTimings,
  nextLanded,
  openChat,
  rollInRoller,
  rollOnServer,
  seatRollTable,
  serverRoll,
  serverRolls,
  setCamera,
  waitForPlayView,
  type LandedThrow,
} from "./fixtures/rolls";

/**
 * Spec 083: a roll is thrown on the board as real dice, landing on the faces
 * the server decided, with a line of arithmetic under them.
 *
 * Everything here is read from the engine's own log (`diceLanded`), written
 * when the dice come to rest, and compared with the roll as the server
 * recorded it (`worldRoll`). Nothing is read off the canvas's pixels.
 */

/** A GM alone at a fresh world's play view. */
async function seatAlone(page: Page): Promise<string> {
  const worldId = await registerAndCreateWorld(
    page,
    `E2E Dice ${uniqueSuffix()}`,
    "e2edicegm",
  );
  await clickPlay(page);
  await waitForPlayView(page);
  return worldId;
}

/** Rolls `formula` on the server and answers the throw this board landed. */
async function throwOne(
  page: Page,
  worldId: string,
  formula: string,
): Promise<LandedThrow> {
  const before = (await diceLanded(page)).length;
  await rollOnServer(page, worldId, formula);
  const [landed] = await nextLanded(page, before);
  expect(landed.skipped, `${formula} was played, not skipped`).toBe(false);
  return landed;
}

function sidesOf(landed: LandedThrow) {
  return landed.dice.map((die) => die.sides);
}

test.describe("Dice on the screen", () => {
  test("US1: a d20 lands on the server's face on every board, wherever the camera is", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);
    const table = await seatRollTable(page, browser);
    const [roller, watcher] = table.players;
    try {
      const timings = await diceTimings(roller);
      await rollInRoller(roller, "1d20 + 5");
      const [mine] = await nextLanded(roller, 0);
      const roll = await serverRoll(roller, table.worldId, mine.rollId);
      const [die] = roll.resolution.dice;

      expect(mine.dice).toHaveLength(1);
      expect(mine.dice[0].sides).toBe(20);
      expect(mine.dice[0].face).toBe(die.finalValue);
      expect(mine.readout).toMatch(
        new RegExp(
          ` ${die.finalValue} \\+ 5 = ${roll.resolution.resultValue}$`,
        ),
      );

      // Every board drew the same throw: the same faces, the same places.
      for (const board of [watcher, table.gm]) {
        const [theirs] = await nextLanded(board, 0);
        expect(theirs.rollId).toBe(mine.rollId);
        expect(theirs.dice).toEqual(mine.dice);
        expect(theirs.readout).toBe(mine.readout);
      }

      // The throw follows the screen, not the map: far from the origin and
      // zoomed, the dice rest where they rest on an unmoved board.
      await setCamera(watcher, { x: 6000, y: -4500, zoom: 2 });
      await watcher.waitForTimeout(1_500);
      for (const board of [roller, watcher, table.gm]) {
        await expect
          .poll(() => diceEntities(board), {
            timeout: timings.holdMs + timings.fadeMs + 1_000,
            message: "the first throw fades and leaves nothing behind",
          })
          .toBe(0);
      }
      await rollInRoller(roller, "1d20 + 5");
      const [unmoved] = await nextLanded(roller, 1);
      const [moved] = await nextLanded(watcher, 1);
      expect(moved.rollId).toBe(unmoved.rollId);
      expect(moved.readout).toBe(unmoved.readout);
      expect(moved.dice.map((d) => d.restingPlace)).toEqual(
        unmoved.dice.map((d) => d.restingPlace),
      );

      // The landed throw holds, fades, and leaves no entity behind.
      await expect
        .poll(() => diceEntities(watcher), {
          timeout: timings.holdMs + timings.fadeMs + 1_000,
        })
        .toBe(0);
    } finally {
      await table.close();
    }
  });

  test("US2: several dice of several kinds", async ({ page }) => {
    test.setTimeout(180_000);
    const worldId = await seatAlone(page);

    const mixed = await throwOne(page, worldId, "2d6 + 1d8 + 3");
    const mixedRoll = await serverRoll(page, worldId, mixed.rollId);
    expect(sidesOf(mixed)).toEqual([6, 6, 8]);
    const faces = mixed.dice.map((d) => d.face);
    expect(faces).toEqual(mixedRoll.resolution.dice.map((d) => d.finalValue));
    expect(mixed.readout).toMatch(
      new RegExp(
        ` ${faces.join(" \\+ ")} \\+ 3 = ${mixedRoll.resolution.resultValue}$`,
      ),
    );

    const percentile = await throwOne(page, worldId, "1d100");
    const percentileRoll = await serverRoll(page, worldId, percentile.rollId);
    expect(sidesOf(percentile)).toEqual([100]);
    expect(percentile.dice[0].face).toBe(
      percentileRoll.resolution.dice[0].finalValue,
    );

    const fate = await throwOne(page, worldId, "4dF");
    expect(sidesOf(fate)).toEqual(["F", "F", "F", "F"]);

    const odd = await throwOne(page, worldId, "1d7");
    expect(sidesOf(odd)).toEqual([7]);
  });

  test("US3: advantage, rerolls, explosions, clamps and successes", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    const worldId = await seatAlone(page);

    const advantage = await throwOne(page, worldId, "2d20kh1 + 4");
    const advantageRoll = await serverRoll(page, worldId, advantage.rollId);
    const kept = advantage.dice.filter((d) => d.kept);
    expect(kept).toHaveLength(1);
    expect(advantage.dice.filter((d) => !d.kept)).toHaveLength(1);
    expect(advantage.readout).toMatch(
      new RegExp(
        ` ${kept[0].face} \\+ 4 = ${advantageRoll.resolution.resultValue}$`,
      ),
    );

    const reroll = await throwOne(page, worldId, "1d6r<7");
    const rerollRoll = await serverRoll(page, worldId, reroll.rollId);
    expect(reroll.dice).toHaveLength(1);
    expect(reroll.dice[0].rerolled).toHaveLength(1);
    expect(reroll.dice[0].face).toBe(rerollRoll.resolution.dice[0].finalValue);

    const explosion = await throwOne(page, worldId, "1d6xo>0");
    expect(explosion.dice).toHaveLength(2);
    expect(explosion.dice[0].explosionOf).toBeNull();
    expect(explosion.dice[1].explosionOf).toBe(0);

    const clamp = await throwOne(page, worldId, "1d20min21");
    const clampRoll = await serverRoll(page, worldId, clamp.rollId);
    expect(clamp.dice[0].clamped).toBe(clampRoll.resolution.dice[0].rolls[0]);
    expect(clamp.dice[0].face).toBe(21);

    const successes = await throwOne(page, worldId, "6d10cs>=1");
    expect(successes.readout).toMatch(/ 6 successes$/);
    expect(successes.dice.map((d) => d.succeeded)).toEqual(Array(6).fill(true));
  });

  test("US4: a check's bonus is the number the server put in", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    await register(page, freshCredentials("e2edicecheck"));
    const suffix = uniqueSuffix();
    const world = await graphql<GqlResult<{ createWorld: { id: string } }>>(
      page,
      `
        mutation ($input: GraphQLCreateWorldInput!) {
          createWorld(input: $input) {
            id
          }
        }
      `,
      { input: { name: `Dice Check ${suffix}`, gameSystemId: "dnd5e" } },
    );
    const worldId = world.data?.createWorld?.id;
    if (!worldId) throw new Error(JSON.stringify(world.errors ?? world));
    const actor = await graphql<GqlResult<{ createActor: { id: string } }>>(
      page,
      `
        mutation ($input: CreateActorInput!) {
          createActor(input: $input) {
            id
          }
        }
      `,
      {
        input: {
          worldId,
          label: `Ayla ${suffix}`,
          isNpc: false,
          gameSystemId: "dnd5e",
        },
      },
    );
    const actorId = actor.data?.createActor?.id;
    if (!actorId) throw new Error(JSON.stringify(actor.errors ?? actor));
    const sheet = await graphql<
      GqlResult<{ updateActorSystemData: { id: string } }>
    >(
      page,
      `
        mutation ($input: GraphQLUpdateActorSystemDataInput!) {
          updateActorSystemData(input: $input) {
            id
          }
        }
      `,
      {
        input: {
          actorId,
          gameSystemId: "dnd5e",
          dataType: "ability_data",
          data: {
            strength: 10,
            dexterity: 16,
            constitution: 14,
            intelligence: 8,
            wisdom: 12,
            charisma: 7,
          },
        },
      },
    );
    if (!sheet.data?.updateActorSystemData?.id) {
      throw new Error(JSON.stringify(sheet.errors ?? sheet));
    }

    await page.goto(`/world/${worldId}/staging`);
    await clickPlay(page);
    await waitForPlayView(page);

    // The check is rolled from the sheet in another tab; it reaches this
    // board as any roll does, from the server.
    const sheetTab = await page.context().newPage();
    await sheetTab.goto(`/world/${worldId}/actor/${actorId}/view`);
    const dexterity = sheetTab.getByTestId("system-check-dexterity");
    await expect(dexterity).toBeVisible({ timeout: 30_000 });
    await dexterity.click();
    await expect(sheetTab.getByTestId("system-check-result")).toBeVisible({
      timeout: 15_000,
    });

    const [landed] = await nextLanded(page, 0);
    const roll = await serverRoll(page, worldId, landed.rollId);
    expect(roll.resolution.formula).toContain("MODIFIER");
    expect(roll.bindings).toHaveLength(1);
    const bonus = roll.bindings[0].value;
    expect(landed.readout).toContain("Dexterity");
    expect(landed.readout).not.toContain("MODIFIER");
    expect(landed.readout).toMatch(
      new RegExp(
        ` ${landed.dice[0].face} \\+ ${bonus} = ${roll.resolution.resultValue}$`,
      ),
    );
    await sheetTab.close();
  });

  test("US5: a busy table, a calm screen, and many dice", async ({ page }) => {
    test.setTimeout(240_000);
    const worldId = await seatAlone(page);
    const timings = await diceTimings(page);

    // Five rolls inside a second: one plays, the rest wait their turn, and
    // the board never falls more than four behind.
    const startedAt = Date.now();
    for (let i = 0; i < 5; i += 1) await rollOnServer(page, worldId, "1d6");
    const sentWithin = Date.now() - startedAt;
    test.info().annotations.push({
      type: "US5",
      description: `five rolls sent within ${sentWithin} ms`,
    });
    const burst = await nextLanded(page, 0, 5, 60_000);
    const created = (await serverRolls(page, worldId)).slice(-5);
    // Every roll is accounted for once. The server's list is not an arrival
    // order (rolls made in the same instant tie on createdAt), so the
    // ids are compared as sets.
    expect(burst.map((t) => t.rollId).sort()).toEqual(
      created.map((r) => r.id).sort(),
    );
    expect(burst.filter((t) => t.skipped).length).toBeLessThanOrEqual(1);
    expect(burst[0].skipped).toBe(false);
    await openChat(page);
    await expect(page.getByTestId("roll-entry")).toHaveCount(5, {
      timeout: 15_000,
    });
    await expect
      .poll(() => diceEntities(page), {
        timeout: timings.holdMs + timings.fadeMs + 1_000,
      })
      .toBe(0);

    // Reduced motion: the dice appear landed, after a short fade.
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.waitForTimeout(500);
    const watch = page.evaluate(
      () =>
        new Promise<number>((resolve) => {
          const probe = (
            window as unknown as {
              __engineProbe: {
                dicePlayed: () => unknown[];
                diceLanded: () => unknown[];
              };
            }
          ).__engineProbe;
          const played = probe.dicePlayed().length;
          const landed = probe.diceLanded().length;
          let handedAt = 0;
          const tick = () => {
            const now = performance.now();
            if (!handedAt && probe.dicePlayed().length > played) {
              handedAt = now;
            }
            if (handedAt && probe.diceLanded().length > landed) {
              resolve(now - handedAt);
              return;
            }
            requestAnimationFrame(tick);
          };
          tick();
        }),
    );
    const before = (await diceLanded(page)).length;
    await rollOnServer(page, worldId, "1d20");
    const tookMs = await watch;
    const [calm] = await nextLanded(page, before);
    test.info().annotations.push({
      type: "US5",
      description: `reduced motion landed ${Math.round(tookMs)} ms after the engine took the roll`,
    });
    expect(calm.reducedMotion).toBe(true);
    // 150 ms of fade, plus a few frames of a software-rendered board.
    expect(tookMs).toBeLessThan(timings.reducedMs + 350);
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await page.waitForTimeout(500);

    // Forty dice: twenty drawn, the rest counted, and the server's total.
    const many = await throwOne(page, worldId, "40d6");
    const manyRoll = await serverRoll(page, worldId, many.rollId);
    expect(many.dice).toHaveLength(20);
    expect(many.chip).toBe("+20 more");
    expect(many.readout).toMatch(
      new RegExp(`= ${manyRoll.resolution.resultValue}$`),
    );
  });

  test("SC-004 and SC-007: fifty rolls leave nothing behind, and twenty dice keep the frame rate", async ({
    page,
  }) => {
    test.setTimeout(300_000);
    const worldId = await seatAlone(page);
    const timings = await diceTimings(page);

    for (let i = 0; i < 50; i += 1) await rollOnServer(page, worldId, "1d6");
    await expect
      .poll(() => diceEntities(page), {
        // The playing throw and four waiting ones, each landing in turn,
        // then the last one's hold and fade.
        timeout:
          6 * timings.tumbleMs + timings.holdMs + timings.fadeMs + 15_000,
        message: "fifty throws end with no dice entities alive",
      })
      .toBe(0);

    const readFrameMs = () =>
      page.evaluate(async () => {
        const mod = (await import(
          /* @vite-ignore */ "/src/engine/bevy/stats.ts"
        )) as typeof import("../src/engine/bevy/stats");
        return (await mod.readEngineStats())?.frameTimeMs ?? null;
      });
    const median = (values: number[]) => {
      const sorted = [...values].sort((a, b) => a - b);
      return sorted[Math.floor(sorted.length / 2)] ?? Infinity;
    };
    const sample = async (ms: number) => {
      const readings: number[] = [];
      const until = Date.now() + ms;
      while (Date.now() < until) {
        const frame = await readFrameMs();
        if (frame !== null && frame > 0) readings.push(frame);
        await page.waitForTimeout(200);
      }
      return readings;
    };

    const idle = median(await sample(2_000));
    await rollOnServer(page, worldId, "20d6");
    const during = median(await sample(timings.tumbleMs + timings.holdMs));
    // The probe keeps the last fifty throws, so after fifty-one the count no
    // longer grows: the newest entry is the one to read.
    await expect
      .poll(async () => (await diceLanded(page)).at(-1)?.dice.length ?? 0, {
        timeout: 30_000,
      })
      .toBe(20);
    test.info().annotations.push({
      type: "SC-007",
      description: `median frame ${during.toFixed(2)} ms during a 20d6 throw, ${idle.toFixed(2)} ms idle`,
    });
    expect(during).toBeLessThanOrEqual(18.2);
  });
});
