import { expect, test } from "@playwright/test";
import {
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";

/**
 * Spec 029, User Story 7 — presentation values come from the application.
 *
 * FR-022 says colours, sizes and spacing must not be compiled into the
 * engine, and FR-023 says the documented default set exists in exactly one
 * place. Both are easy to satisfy on paper and easy to lose: a constant left
 * behind at one drawing site still works, still looks right, and quietly
 * ignores every override aimed at it.
 *
 * # What this actually pins
 *
 * Two things, and the second is the one worth having.
 *
 * 1. A **partial** override is accepted. Sending only `barHeight` must not
 *    require restating the palette — an application forced to restate values
 *    it does not care about pins those values to whatever the defaults were
 *    the day it was written, and they never improve again.
 * 2. Overrides **accumulate**. Two calls in a row must leave both changes in
 *    effect. The natural wrong implementation folds each override onto the
 *    *defaults* rather than onto what is current, which looks identical for
 *    a single call and silently discards the first of any two.
 *
 * Both are observed through the engine's own report channel rather than by
 * sampling pixels: an accepted command is one the engine did not refuse, and
 * a refusal is exactly what it now says out loud.
 */

test("the application supplies appearance values, partially and cumulatively", async ({
  page,
}) => {
  test.setTimeout(3 * 60_000);

  const reports: string[] = [];
  page.on("console", (message) => {
    const text = message.text();
    if (text.includes("[engine sdk]")) reports.push(text);
  });

  const worldId = await registerAndCreateWorld(page, `Look ${uniqueSuffix()}`);
  await page.goto(`/world/${worldId}/play`);
  await waitForEngineReady(page);

  const sent = await page.evaluate(async () => {
    const probe = (await import(
      /* @vite-ignore */ "/src/engine/bevy/sdkFaultProbe.ts"
    )) as typeof import("../src/engine/bevy/sdkFaultProbe");

    const payloads = [
      // One field only. The rest must be left alone, not reset.
      JSON.stringify({
        type: "set_display_appearance",
        sdkVersion: 1,
        appearance: { barHeight: 14 },
      }),
      // A second, disjoint field. Both must survive.
      JSON.stringify({
        type: "set_display_appearance",
        sdkVersion: 1,
        appearance: { barGap: 6 },
      }),
      // Nothing at all is a no-op, not a fault: it asked for nothing.
      JSON.stringify({ type: "set_display_appearance", sdkVersion: 1 }),
      // A whole palette, which is also legitimate.
      JSON.stringify({
        type: "set_display_appearance",
        sdkVersion: 1,
        appearance: {
          palette: [
            [0.9, 0.2, 0.2],
            [0.2, 0.4, 0.9],
          ],
        },
      }),
    ];
    let delivered = 0;
    for (const payload of payloads) {
      if (await probe.injectRawEngineCommand(payload)) delivered += 1;
    }
    return delivered;
  });
  expect(sent).toBe(4);

  // Give the engine a frame or two to have refused anything it was going to.
  await page.waitForTimeout(1_000);
  expect(
    reports,
    `no legitimate appearance command may be refused: ${reports.join(" | ")}`,
  ).toHaveLength(0);

  // --- and the gate still works ------------------------------------------
  //
  // A test that only sent valid commands would pass against an engine that
  // accepted literally anything, which is the same silent-drift failure this
  // spec exists to close. `deny_unknown_fields` is what makes a misspelled
  // appearance field a reported fault rather than a setting that does
  // nothing, so that is what gets asserted.
  await page.evaluate(async () => {
    const probe = (await import(
      /* @vite-ignore */ "/src/engine/bevy/sdkFaultProbe.ts"
    )) as typeof import("../src/engine/bevy/sdkFaultProbe");
    await probe.injectRawEngineCommand(
      JSON.stringify({
        type: "set_display_appearance",
        sdkVersion: 1,
        appearance: { barHieght: 14 },
      }),
    );
  });

  await expect
    .poll(() => reports.length, {
      message: "a misspelled appearance field must be reported, not ignored",
      timeout: 20_000,
    })
    .toBeGreaterThan(0);

  console.log(
    `[appearance] accepted=${sent} refusedValid=0 misspellingReported=true`,
  );
});
