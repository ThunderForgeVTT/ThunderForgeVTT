import { test, expect, type Page } from "./fixtures/test";
import { expectNoAxeViolations } from "./fixtures/axe";
import {
  freshCredentials,
  graphql,
  register,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * `/worlds`: tiles up to six, a table from seven, and a control that flips
 * either way at any count and is remembered.
 *
 * The owner's words: "if somebody has more than six worlds, we should convert
 * to a table, but allow them to flip between table or tiled view". Six and
 * seven are therefore the two counts worth a test — the boundary is the rule
 * — and the flip has to survive a reload, because a preference that forgets
 * is a preference nobody trusts.
 */

const CREATE_WORLD = `
  mutation CreateWorldForArchive($input: GraphQLCreateWorldInput!) {
    createWorld(input: $input) {
      id
      name
    }
  }
`;

const MY_WORLDS = `
  query MyWorldsForArchive {
    myWorldsWithRole {
      role
      world {
        id
      }
    }
  }
`;

/** Worlds made through GraphQL: this spec is about the list, not creation. */
async function createWorlds(page: Page, count: number): Promise<void> {
  for (let index = 0; index < count; index += 1) {
    const name = `Archive World ${index + 1} ${uniqueSuffix()}`;
    const result = await graphql<{
      data?: { createWorld?: { id: string } };
      errors?: unknown;
    }>(page, CREATE_WORLD, { input: { name } });
    if (!result.data?.createWorld?.id) {
      throw new Error(`createWorld failed: ${JSON.stringify(result)}`);
    }
  }
}

/**
 * Bring this account up to exactly `target` worlds.
 *
 * Whether registration leaves an account with a world of its own is an
 * onboarding detail that has changed before and is not what this spec is
 * about; counting first and topping up means the boundary under test is the
 * boundary regardless.
 */
async function ensureWorldCount(page: Page, target: number): Promise<void> {
  const result = await graphql<{
    data?: { myWorldsWithRole?: unknown[] };
  }>(page, MY_WORLDS, {});
  const existing = result.data?.myWorldsWithRole?.length ?? 0;
  if (existing > target) {
    throw new Error(`account already has ${existing} worlds, wanted ${target}`);
  }
  await createWorlds(page, target - existing);
}

test.describe("The world archive draws tiles or a table", () => {
  test("six worlds draw tiles, a seventh draws the table, and the flip survives a reload", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2earchive"));

    // Six: the last count that is still a wall of tiles.
    await ensureWorldCount(page, 6);
    await page.goto("/worlds");
    await expect(page.getByText("6 worlds")).toBeVisible({ timeout: 20_000 });
    await expect(page.getByTestId("worlds-table")).toHaveCount(0);
    await expect(page.getByTestId("worlds-view-toggle")).toBeVisible();

    // The control is there at six, and flipping to the table works below the
    // threshold as much as above it.
    await page.getByTestId("worlds-view-table").click();
    await expect(page.getByTestId("worlds-table")).toBeVisible();
    await expect(page.getByTestId("worlds-table-row")).toHaveCount(6);

    // Remembered per person, not per visit.
    await page.reload();
    await expect(page.getByTestId("worlds-table")).toBeVisible({
      timeout: 20_000,
    });
    await expect(page.getByTestId("worlds-view-table")).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    // ...and back, which must also stick — a stored choice beats the count
    // in both directions, so seven worlds will still draw tiles below.
    await page.getByTestId("worlds-view-tiles").click();
    await expect(page.getByTestId("worlds-table")).toHaveCount(0);
    await page.reload();
    await expect(page.getByTestId("worlds-view-tiles")).toHaveAttribute(
      "aria-pressed",
      "true",
      { timeout: 20_000 },
    );
    await expect(page.getByTestId("worlds-table")).toHaveCount(0);

    // A seventh world. The count's verdict is the table, but this person has
    // chosen tiles, and the choice wins.
    await ensureWorldCount(page, 7);
    await page.goto("/worlds");
    await expect(page.getByText("7 worlds")).toBeVisible({ timeout: 20_000 });
    await expect(page.getByTestId("worlds-table")).toHaveCount(0);

    // Clearing the stored choice is the state of somebody who has never
    // said, and seven is where that person gets a table.
    await page.evaluate(() => window.localStorage.removeItem("tf:worlds-view"));
    await page.reload();
    await expect(page.getByTestId("worlds-table")).toBeVisible({
      timeout: 20_000,
    });
    await expect(page.getByTestId("worlds-table-row")).toHaveCount(7);
  });

  test("a row carries what a person picks a world by, and every tile action", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2earchiverow"));
    await ensureWorldCount(page, 6);
    await page.goto("/worlds");
    await page.getByTestId("worlds-view-table").click();

    const table = page.getByTestId("worlds-table");
    await expect(table).toBeVisible({ timeout: 20_000 });

    // Real table semantics, and the owner's columns.
    for (const column of [
      "World",
      "Game system",
      "Your role",
      "Players",
      "Play",
    ]) {
      await expect(
        table.getByRole("columnheader", { name: column, exact: true }),
      ).toBeVisible();
    }

    // Last played is deliberately absent — nothing exposes
    // `world_live_play.last_beat_at` to a list, and spec 054 FR-012a is
    // where it gets read properly. A column filled from `updatedAt` would
    // be a lie a GM plans a session around.
    await expect(
      table.getByRole("columnheader", { name: /last played/i }),
    ).toHaveCount(0);

    const firstRow = page.getByTestId("worlds-table-row").first();
    // The system a world runs, by title rather than by id.
    await expect(firstRow).toContainText("Genie");
    // The caller owns every world here.
    await expect(firstRow).toContainText("Owner");
    // Both of the tile's actions are reachable from the row.
    await expect(firstRow.getByTestId("worlds-table-name")).toBeVisible();
    await expect(
      firstRow.getByRole("link", { name: "Enter world" }),
    ).toBeVisible();

    // The players figure is a count, not a placeholder, once it arrives.
    await expect(firstRow.locator("td").nth(2)).toHaveText(/^[0-9]+$/, {
      timeout: 20_000,
    });

    // The name opens the dashboard.
    await firstRow.getByTestId("worlds-table-name").click();
    await page.waitForURL(/\/world\/[^/]+$/, { timeout: 20_000 });
  });

  test("tiles and table are both readable at 1440px and 375px", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2earchivea11y"));
    await ensureWorldCount(page, 6);

    for (const width of [1440, 375]) {
      await page.setViewportSize({ width, height: 900 });
      await page.goto("/worlds");
      await expect(page.getByTestId("worlds-view-toggle")).toBeVisible({
        timeout: 20_000,
      });

      await expectNoAxeViolations(page, "main");
      await expect(await pageScrollsSideways(page)).toBe(false);

      await page.getByTestId("worlds-view-table").click();
      await expect(page.getByTestId("worlds-table")).toBeVisible();
      await expectNoAxeViolations(page, "main");
      // The table's own container scrolls at 375px; the document does not.
      await expect(await pageScrollsSideways(page)).toBe(false);

      await page.getByTestId("worlds-view-tiles").click();
    }

    await page.setViewportSize({ width: 1280, height: 900 });
  });
});

async function pageScrollsSideways(page: Page): Promise<boolean> {
  return page.evaluate(
    () =>
      document.documentElement.scrollWidth >
      document.documentElement.clientWidth + 1,
  );
}
