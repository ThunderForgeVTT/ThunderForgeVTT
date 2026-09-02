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
