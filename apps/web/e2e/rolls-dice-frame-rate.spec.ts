import { expect, test } from "./fixtures/test";
import {
  diceEntities,
  diceLanded,
  diceTimings,
  rollOnServer,
  seatAlone,
} from "./fixtures/rolls";

/**
 * Spec 083's SC-004 and SC-007: dice leave nothing behind, and a throw of
 * twenty keeps the frame rate.
 *
 * Split out of `rolls-dice-on-screen.spec.ts` (spec 087) because the frame
 * time is a number about the machine and the build, not about behaviour. It
 * is in `PERF_LANE_SPECS`, so it runs on a release engine in the measured
 * lane, alone on the GPU, as `engine-limits` does. On a dev engine it read
 * 19.7 and 22.2 ms against the 18.2 ms it asserts.
 */
test.describe("Dice on the screen", () => {
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
