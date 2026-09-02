import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  openDockTab,
  openGmTool,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 030, through the interface a Game Master actually has.
 *
 * Every other spec 030 test drives the feature over GraphQL, which proves the
 * rules but says nothing about whether a GM can *reach* them. This is the
 * other half: the Interactions tool is in the rail, it is driven by the effect
 * registry, and what it saves is what the server stores.
 *
 * Written because the feature shipped fully tested and mounted nowhere — the
 * components existed and no page rendered them, which is exactly the gap an
 * end-to-end suite is supposed to catch and this one could not, because every
 * test reached past the UI.
 */

async function gql<T>(
  page: Page,
  query: string,
  variables: Record<string, unknown>,
): Promise<T> {
  const res = await graphql<{ data?: T; errors?: { message: string }[] }>(
    page,
    query,
    variables,
  );
  if (res.errors?.length || !res.data) {
    throw new Error(`GraphQL failed: ${JSON.stringify(res.errors ?? res)}`);
  }
  return res.data;
}

test("a Game Master reaches interactions from the tool rail", async ({
  page,
}) => {
  test.setTimeout(4 * 60_000);

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Rail ${suffix}`);
  const [sceneId] = await sceneIds(page, worldId);

  // Created for its side effect: the interaction tool's configuration field is
  // a picker of this world's lore, so there has to be an entry to pick. The
  // returned id is not needed — the test selects by name.
  await gql<{ createLoreEntry: { id: string } }>(
    page,
    `mutation ($input: CreateLoreEntryInput!) {
      createLoreEntry(input: $input) { id }
    }`,
    {
      input: {
        worldId,
        title: `Ledger ${suffix}`,
        content: "Debts nobody wishes to settle.",
      },
    },
  );

  const prop = await gql<{ createToken: { tokenId: string } }>(
    page,
    `mutation ($input: GraphQLCreateTokenInput!) {
      createToken(input: $input) { tokenId }
    }`,
    { input: { sceneId, x: 0, y: 0, tokenType: "object" } },
  );

  await page.goto(`/world/${worldId}/play`);
  await waitForEngineReady(page);

  // The tool is in the rail at all.
  await openGmTool(page, "interactions");
  const tool = page.getByTestId("interaction-tool");
  await expect(tool).toBeVisible();

  // With nothing selected it says so, rather than showing an empty form that
  // would silently save against nothing.
  await expect(tool).toContainText(/select a token or a wall/i);

  // Select the prop through the store, the way clicking it does.
  await page.evaluate(async (tokenId: string) => {
    const bevy = (await import(
      /* @vite-ignore */ "/src/engine/bevy/index.ts"
    )) as typeof import("../src/engine/bevy/index");
    bevy
      .getBoundWorldStore()
      ?.dispatch({ type: "select_token", tokenId }, "ui");
  }, prop.createToken.tokenId);

  // The author panel appears, and its effect list came from the registry.
  const effect = page.locator("#interaction-effect");
  await expect(effect).toBeVisible({ timeout: 10_000 });
  await effect.click();
  await expect(
    page.getByRole("option", { name: /open a lore page/i }),
  ).toBeVisible({ timeout: 10_000 });

  // Nothing offers a sound effect, because no audio subsystem exists.
  await expect(page.getByRole("option", { name: /sound|audio/i })).toHaveCount(
    0,
  );

  await page.getByRole("option", { name: /open a lore page/i }).click();

  // The configuration field is a picker of this world's lore, not a text box.
  const entryField = page.locator("#interaction-config-entry");
  await expect(entryField).toBeVisible();
  await entryField.click();
  await page.getByRole("option", { name: `Ledger ${suffix}` }).click();

  await page.getByRole("button", { name: /^save$/i }).click();

  // What the GM saved is what the server stored.
  await expect
    .poll(
      async () => {
        const data = await gql<{
          interactives: {
            subjectRef: string | null;
            effectId: string | null;
          }[];
        }>(
          page,
          `query ($s: UUID!) {
            interactives(sceneId: $s) { subjectRef effectId }
          }`,
          { s: sceneId },
        );
        return data.interactives.find(
          (i) => i.subjectRef === prop.createToken.tokenId,
        )?.effectId;
      },
      { message: "the rail's save should reach the server", timeout: 20_000 },
    )
    .toBe("lore.open");

  // And the GM's approval queue is reachable from the dock.
  await openDockTab(page, "requests");
  await expect(page.getByText(/nothing is waiting on you/i)).toBeVisible({
    timeout: 10_000,
  });

  console.log(
    `[rail] toolReachable=true registryDriven=true saved=lore.open queueReachable=true`,
  );
});

/**
 * Spec 031 FR-011 — the half of the lore marker that was missing.
 *
 * Authoring one has worked since spec 030, and there was no way to put one on
 * a map: `placeProp` had no caller anywhere in the application. This drives
 * the gesture the way a Game Master does — choose what it does, then click the
 * map — and checks what the server ended up with, because "a token appeared"
 * and "a token appeared with a lore page attached to it" are different claims
 * and only the second one is the feature.
 *
 * The engine side is deliberately not asserted by looking at pixels. What is
 * checked instead is that the token reached the world store through the
 * ordinary sync, which is what the badge is drawn from — a canvas holding
 * something chrome does not know about, or the reverse, is the failure this
 * suite exists to catch.
 */
test("a Game Master carries a lore marker onto the map", async ({ page }) => {
  test.setTimeout(4 * 60_000);

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Carry ${suffix}`);
  const [sceneId] = await sceneIds(page, worldId);

  const entryTitle = `Ledger ${suffix}`;
  await gql<{ createLoreEntry: { id: string } }>(
    page,
    `mutation ($input: CreateLoreEntryInput!) {
      createLoreEntry(input: $input) { id }
    }`,
    {
      input: {
        worldId,
        title: entryTitle,
        content: "Debts nobody wishes to settle.",
      },
    },
  );

  await page.goto(`/world/${worldId}/play`);
  await waitForEngineReady(page);
  await openGmTool(page, "interactions");

  // With nothing selected, the panel offers to place something new — which is
  // the only moment a Game Master has nothing to author onto.
  const placer = page.getByTestId("prop-placer");
  await expect(placer).toBeVisible({ timeout: 10_000 });

  // The helper button, not the dropdown: it is what a GM placing their first
  // marker reaches for, and it is registry-driven, so its presence is also a
  // check that `lore.open` is contributed to this build for a prop.
  await placer.getByTestId("interaction-helper-lore.open").click();

  const entryField = page.locator("#interaction-config-entry");
  await expect(entryField).toBeVisible();
  await entryField.click();
  await page.getByRole("option", { name: entryTitle }).click();

  const tokensBefore = await page.evaluate(
    () => window.__worldProbe?.state().counts.tokens ?? -1,
  );
  expect(
    tokensBefore,
    "the world probe should be available in a dev build",
  ).toBeGreaterThanOrEqual(0);

  await page.getByTestId("interaction-save").click();
  await expect(page.getByTestId("prop-placer-carrying")).toBeVisible({
    timeout: 10_000,
  });

  // Nothing exists yet: the carry is a gesture, and the engine has created
  // nothing (FR-040's sibling claim — arming a tool places nothing).
  expect(
    await page.evaluate(() => window.__worldProbe?.state().counts.tokens ?? -1),
    "carrying a marker must not create anything",
  ).toBe(tokensBefore);

  // Drop it. Down and up are separated because Bevy reads `just_pressed` per
  // polled frame, and a zero-delay synthetic click can collapse into one.
  const box = await page.locator("canvas").boundingBox();
  if (!box) {
    throw new Error("the canvas should have a box");
  }
  await page.mouse.move(
    box.x + box.width / 2 + 60,
    box.y + box.height / 2 + 40,
  );
  await page.waitForTimeout(150);
  await page.mouse.down();
  await page.waitForTimeout(80);
  await page.mouse.up();

  // What the server ended up with: a prop, with the lore page attached.
  await expect
    .poll(
      async () => {
        const data = await gql<{
          interactives: {
            subjectKind: string;
            subjectRef: string | null;
            effectId: string | null;
          }[];
        }>(
          page,
          `query ($s: UUID!) {
            interactives(sceneId: $s) { subjectKind subjectRef effectId }
          }`,
          { s: sceneId },
        );
        const marker = data.interactives.find(
          (i) => i.effectId === "lore.open",
        );
        return marker
          ? `${marker.subjectKind}:${marker.subjectRef !== null}`
          : null;
      },
      {
        message: "the drop should reach the server as a prop with an effect",
        timeout: 30_000,
      },
    )
    .toBe("prop:true");

  // A prop is a token with no actor — the whole implementation, and the thing
  // that makes it drawable at all.
  const tokens = await gql<{
    tokens: { tokenType: string | null; actorId: string | null }[];
  }>(page, `query ($s: UUID!) { tokens(sceneId: $s) { tokenType actorId } }`, {
    s: sceneId,
  });
  expect(
    tokens.tokens.some((t) => t.tokenType === "object" && t.actorId === null),
  ).toBe(true);

  // And it reached the canvas the ordinary way, rather than only the server.
  await expect
    .poll(
      () =>
        page.evaluate(() => window.__worldProbe?.state().counts.tokens ?? -1),
      {
        message: "the placed prop should arrive on the canvas through sync",
        timeout: 30_000,
      },
    )
    .toBeGreaterThan(tokensBefore);

  await expect(page.getByTestId("prop-placer-placed")).toBeVisible();

  console.log(`[place] carried=true placed=lore.open onProp=true`);
});
