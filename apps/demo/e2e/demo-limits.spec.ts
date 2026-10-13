import { expect, test, type Page } from "@playwright/test";
import { openSitting, sitting, WORLD_ID } from "./sitting";

/**
 * Spec 074: where the demo stops. What it does not do says so, and a player
 * sees only what a player is sent.
 *
 * The tests share a page and run in order, one sitting in which the browser
 * never once reaches past the demo's own files (SC-003).
 */

let page: Page;
/**
 * Every request and socket that is not one of the demo's own files, nor a
 * telemetry post to the configured origin (spec 086 FR-033).
 */
let outside: string[];

const { ask, scenes, cast } = sitting(() => page);

test.describe.configure({ mode: "serial" });

test.beforeAll(async ({ browser, baseURL }) => {
  ({ page, outside } = await openSitting(browser, baseURL));
});

test.afterAll(async () => {
  await page.context().close();
});

test.afterEach(() => {
  // SC-003: checked after every step, so the step that reached out is the
  // one that fails.
  expect(outside, "nothing leaves the demo's own static files").toEqual([]);
});

test("something the demo does not do says so, and is not a connection error", async () => {
  await page.goto(`/demo/world/${WORLD_ID}`);
  const answer = await ask(
    "mutation ($input: GenerateInviteCodeInput!) { generateInviteCode(input: $input) { id } }",
    { input: { worldId: WORLD_ID, maxUses: 1 } },
  );
  expect(answer.status).toBe(200);
  expect(answer.body.errors?.[0]?.extensions?.code).toBe("NOT_IN_DEMO");
  // Named for the visitor, not for the schema.
  expect(answer.body.errors?.[0]?.message).toBe(
    "Invite links is not part of the demo.",
  );
  await expect(page.getByText("Not part of the demo")).toBeVisible();
  await expect(
    page.getByText("Invite links needs a real ThunderForge instance."),
  ).toBeVisible();
  await expect(page.getByText("generateInviteCode")).toHaveCount(0);

  // The one XMLHttpRequest the client makes, the book importer's upload, is
  // answered as a refusal it can read, not thrown at as it is constructed.
  const upload = await page.evaluate(
    () =>
      new Promise<{ status: number; message: string }>((resolve, reject) => {
        const request = new XMLHttpRequest();
        request.open("POST", "/api/graphql");
        request.onerror = () => reject(new Error("the upload errored"));
        request.onload = () =>
          resolve({
            status: request.status,
            message: JSON.parse(request.responseText).errors[0].message,
          });
        request.send("{}");
      }),
  );
  expect(upload).toEqual({
    status: 404,
    message: "Importing a book is not part of the demo.",
  });
  await expect(
    page.getByText("Importing a book needs a real ThunderForge instance."),
  ).toBeVisible();
});

test("as a player, the heroes are theirs and the ambushers stand on the board unnamed", async () => {
  await page.goto(`/demo/world/${WORLD_ID}`);
  await page.getByRole("button", { name: "View as player" }).click();
  await expect(page.getByTestId("demo-viewer")).toContainText(
    "Viewing as a player",
  );
  // A player's dashboard: no owner's controls, and no "not part of the demo"
  // for anything it asks on the way in.
  await expect(page.getByRole("button", { name: "Delete world" })).toHaveCount(
    0,
  );
  await expect(page.getByText("Not part of the demo")).toHaveCount(0);
  const seen = await cast();
  expect(seen.map((a) => a.label).sort()).toEqual([
    "Brannoc Stoneward",
    "Elowen Vire",
  ]);
  expect(seen.every((a) => a.myPermissionLevel === "OWNER")).toBe(true);
  // And they arrive already playing the fighter (spec 023), not choosing.
  const claim = await ask<{ myActorClaim: { actor: { label: string } } }>(
    "query ($worldId: UUID!) { myActorClaim(worldId: $worldId) { actorId actor { label } } }",
    { worldId: WORLD_ID },
  );
  expect(claim.body.data?.myActorClaim.actor.label).toBe("Brannoc Stoneward");
  const ambush = (await scenes()).find((s) => s.name === "Grassy Path Ambush");
  if (!ambush) throw new Error("Grassy Path Ambush is a scene of the demo");
  // Every figure is on a player's board, as the server sends it: the
  // monsters are there to be seen, only their names are the GM's.
  const tokens = await ask<{
    tokens: { tokenId: string; name: string | null }[];
  }>("query ($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId name } }", {
    sceneId: ambush.sceneId,
  });
  const board = tokens.body.data?.tokens ?? [];
  expect(board).toHaveLength(7);
  expect(board.filter((t) => t.name === null)).toHaveLength(5);
  // And the play field is a player's: a board with no Game Master's tools.
  await page.goto(`/demo/world/${WORLD_ID}/play`);
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("gm-tool-walls")).toHaveCount(0);
  await page.getByRole("button", { name: "View as Game Master" }).click();
  await expect(page.getByTestId("demo-viewer")).toContainText(
    "Viewing as the Game Master",
  );
  expect(await cast()).toHaveLength(5);
});
