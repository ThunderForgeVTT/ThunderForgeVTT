import type { Locator, Page } from "@playwright/test";

import { expectNoAxeViolations } from "./fixtures/axe";
import { graphql, registerAndCreateWorld } from "./fixtures/helpers";
import { expect, test } from "./fixtures/test";

/**
 * Genie's actor sheet as a laid-out page, not one column of tabs.
 *
 * Asserted by geometry, because "the regions exist" passed against the old
 * sheet too — every one of these facts was present, one tab at a time, in a
 * column as wide as a phone. What the owner asked for is where they sit: side
 * by side on a wide screen, one above another at 375px, in the order a sheet
 * is read, with the host's imagery panel above and not repeated inside.
 */

const REGION_ORDER = [
  "identity",
  "scores",
  "pools",
  "conditions",
  "traits",
  "notes",
];

async function createGenieCharacter(page: Page): Promise<{
  worldId: string;
  actorId: string;
}> {
  const worldId = await registerAndCreateWorld(
    page,
    `E2E Genie Sheet ${Date.now()}`,
    "e2egsheet",
  );
  const actor = await graphql<{ data: { createActor: { id: string } } }>(
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
        label: "Layout Genie",
        isNpc: false,
        gameSystemId: "genie",
      },
    },
  );
  return { worldId, actorId: actor.data.createActor.id };
}

function region(page: Page, id: string): Locator {
  return page.locator(
    `[data-testid="genie-actor-sheet"] [data-region="${id}"]`,
  );
}

async function boxOf(locator: Locator) {
  const box = await locator.boundingBox();
  if (!box) throw new Error("region is not rendered");
  return box;
}

async function gridColumnCount(page: Page): Promise<number> {
  return page
    .getByTestId("genie-actor-sheet")
    .evaluate(
      (element) =>
        getComputedStyle(element).gridTemplateColumns.split(" ").length,
    );
}

test("Genie's sheet is laid out in columns on a wide screen and one column at 375px", async ({
  page,
}) => {
  test.setTimeout(120_000);
  const { worldId, actorId } = await createGenieCharacter(page);

  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(`/world/${worldId}/actor/${actorId}/edit`);
  const sheet = page.getByTestId("genie-actor-sheet");
  await expect(sheet).toBeVisible({ timeout: 15_000 });

  // The regions, in reading order, and nothing else at the top level.
  const ids = await sheet
    .locator(":scope > section")
    .evaluateAll((sections) =>
      sections.map((section) => section.getAttribute("data-region")),
    );
  expect(ids).toEqual(REGION_ORDER);

  // Each region is a named section with its own heading.
  for (const id of REGION_ORDER) {
    await expect(
      region(page, id).getByRole("heading", { level: 2 }),
    ).toBeVisible();
  }

  // Wide: three columns, rows paired as declared.
  expect(await gridColumnCount(page)).toBe(3);
  const identity = await boxOf(region(page, "identity"));
  const scores = await boxOf(region(page, "scores"));
  const pools = await boxOf(region(page, "pools"));
  const conditions = await boxOf(region(page, "conditions"));
  const traits = await boxOf(region(page, "traits"));
  const notes = await boxOf(region(page, "notes"));

  expect(scores.y).toBeCloseTo(identity.y, 0);
  expect(scores.x).toBeGreaterThan(identity.x + identity.width - 1);
  expect(scores.width).toBeGreaterThan(identity.width * 1.5);

  expect(conditions.y).toBeCloseTo(pools.y, 0);
  expect(pools.y).toBeGreaterThan(identity.y);
  expect(conditions.x).toBeGreaterThan(pools.x);

  expect(notes.y).toBeCloseTo(traits.y, 0);
  expect(notes.width).toBeGreaterThan(traits.width * 1.5);

  // The host's imagery panel sits above the sheet, and the sheet draws no
  // picture or uploader of its own.
  const imagery = await boxOf(page.getByTestId("actor-imagery-panel"));
  expect(imagery.y).toBeLessThan(identity.y);
  await expect(sheet.locator('input[type="file"], img')).toHaveCount(0);

  // Every control is still there and labelled.
  await expect(sheet.getByLabel("Might", { exact: true })).toBeVisible();
  await expect(sheet.getByLabel("Health", { exact: true })).toBeVisible();
  await expect(sheet.getByLabel("Wish Points", { exact: true })).toBeVisible();
  await expect(sheet.getByLabel("Level", { exact: true })).toBeVisible();
  await expect(
    page.getByTestId("genie-condition-editor").getByLabel("Bound"),
  ).toBeVisible();

  await expectNoAxeViolations(page, '[data-testid="genie-actor-sheet"]');

  // Notes write through `trait_data.notes`, and survive a reload.
  await page
    .getByTestId("genie-notes-input")
    .fill("Owes the Brass Sultan a favour.");
  await page.getByTestId("genie-notes-save").click();
  await expect(page.getByTestId("genie-notes-save")).toBeDisabled({
    timeout: 10_000,
  });
  await page.reload();
  await expect(page.getByTestId("genie-notes-input")).toHaveValue(
    "Owes the Brass Sultan a favour.",
    { timeout: 15_000 },
  );

  // Narrow: one column, stacked in the same order, no sideways scroll.
  await page.setViewportSize({ width: 375, height: 812 });
  await page.reload();
  await expect(sheet).toBeVisible({ timeout: 15_000 });
  expect(await gridColumnCount(page)).toBe(1);
  let previousBottom = -Infinity;
  let firstX: number | null = null;
  for (const id of REGION_ORDER) {
    const box = await boxOf(region(page, id));
    expect(box.y, `${id} is below the region before it`).toBeGreaterThanOrEqual(
      previousBottom - 1,
    );
    firstX ??= box.x;
    expect(box.x).toBeCloseTo(firstX, 0);
    previousBottom = box.y + box.height;
  }
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - window.innerWidth,
  );
  expect(
    overflow,
    "the page must not scroll sideways at 375px",
  ).toBeLessThanOrEqual(0);
  await expectNoAxeViolations(page, '[data-testid="genie-actor-sheet"]');

  // Read-only: the same regions, and nothing to change.
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(`/world/${worldId}/actor/${actorId}/view`);
  await expect(sheet).toBeVisible({ timeout: 15_000 });
  await expect(sheet).toHaveAttribute("data-editable", "false");
  await expect(sheet.locator("input, textarea, button")).toHaveCount(0);
  await expect(region(page, "notes")).toContainText(
    "Owes the Brass Sultan a favour.",
  );
  await expectNoAxeViolations(page, '[data-testid="genie-actor-sheet"]');
});
