import { expect, test, type Page } from "@playwright/test";
import { openSitting, sitting, WORLD_ID } from "./sitting";

/**
 * Spec 074: the demo's pages around the board, as it ships: lore, the
 * compendium, the dashboard, the sidebar and the example maps, each opened
 * without the demo refusing anything.
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

const { ask, refusedSoFar, scenes } = sitting(() => page);

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

test("a lore entry is written, edited and listed, and its link reaches the fighter's sheet", async () => {
  await page.goto(`/demo/world/${WORLD_ID}/compendium?tab=lore`);
  const table = page.getByTestId("lore-catalog-table");
  // The ambush's own entries are there to read before anything is written.
  await expect(table.getByText("The Stoneward Oath")).toBeVisible({
    timeout: 30_000,
  });

  await page.getByTestId("new-lore-entry-title-input").fill("Camp Notes");
  await page.getByTestId("add-lore-entry-button").click();
  await expect(table.getByText("Camp Notes")).toBeVisible();

  await page.goto(`/demo/world/${WORLD_ID}/lore/camp-notes/edit`);
  const editor = page
    .getByTestId("lore-markdown-editor-textarea")
    .locator(".cm-content");
  await editor.click();
  await page.keyboard.type("Watch kept by [[Brannoc Stoneward]] until dawn.");
  await page.locator("#lore-entry-title").fill("Camp Notes, Night One");
  await page.getByRole("button", { name: "Save" }).click();
  await expect(page.getByText("Saved.")).toBeVisible();
  await expect(page).toHaveURL(/\/lore\/camp-notes-night-one\/edit$/);

  await page.goto(`/demo/world/${WORLD_ID}/lore/camp-notes-night-one/view`);
  const link = page.locator("a.lore-link", { hasText: "Brannoc Stoneward" });
  await expect(link).toBeVisible();
  await expect(link).toHaveAttribute(
    "href",
    new RegExp(`^/demo/world/${WORLD_ID}/actor/[^/]+/view$`),
  );
  await link.click();
  await expect(page.getByTestId("dnd5e-actor-sheet")).toBeVisible();

  await page.goto(`/demo/world/${WORLD_ID}/compendium?tab=lore`);
  await expect(table.getByText("Camp Notes, Night One")).toBeVisible();
  expect(await refusedSoFar()).toEqual([]);
  await expect(page.getByText("Not part of the demo")).toHaveCount(0);
});

test("the compendium's Books tab says the world holds no books, rather than refusing", async () => {
  await page.goto(`/demo/world/${WORLD_ID}/compendium?tab=books`);
  await expect(page.getByRole("tab", { name: /books/i })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await page.waitForLoadState("networkidle");
  expect(await refusedSoFar()).toEqual([]);
  await expect(page.getByText("Not part of the demo")).toHaveCount(0);
});

test("controls that only a real instance has are not offered, and session notes are kept", async () => {
  // FR-013: offered and then refused is worse than not offered.
  await page.goto(`/demo/world/${WORLD_ID}`);
  await expect(
    page.getByRole("link", { name: /enter world/i }).first(),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Delete world" })).toHaveCount(
    0,
  );
  await expect(page.getByText("Create another world")).toHaveCount(0);
  await expect(page.getByText("Permanently delete this world")).toHaveCount(0);
  await expect(page.getByText("Generate Join Link")).toHaveCount(0);

  await page.getByRole("button", { name: "Menu" }).click();
  await expect(
    page.getByRole("menuitem", { name: "World archive" }),
  ).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Sign out" })).toHaveCount(0);
  await page.keyboard.press("Escape");

  await page.goto("/demo/settings/account");
  await expect(page.getByTestId("account-demo")).toBeVisible();
  await expect(
    page.getByRole("button", { name: /delete account/i }),
  ).toHaveCount(0);
  await expect(page.getByRole("button", { name: /download/i })).toHaveCount(0);

  await page.goto("/demo/status");
  await expect(page.getByTestId("status-demo")).toContainText(
    "Running in your browser",
  );

  await page.goto(`/demo/world/${WORLD_ID}/staging`);
  await expect(page.getByTestId("session-setup-invite-link")).toHaveCount(0);
  const notes = page.getByTestId("session-notes-editor").locator(".cm-content");
  await notes.click();
  await page.keyboard.type("The goblins owe the party a cart.");
  await page.getByTestId("session-notes-save-button").click();
  await expect(page.getByText("Saved.")).toBeVisible();
  const saved = await ask<{ world: { sessionNotes: string } }>(
    `query ($id: UUID!) { world(id: $id) { sessionNotes } }`,
    { id: WORLD_ID },
  );
  expect(saved.body.data?.world.sessionNotes).toContain("owe the party a cart");

  expect(await refusedSoFar()).toEqual([]);
  await expect(page.getByText("Not part of the demo")).toHaveCount(0);
});

test("every page of the Game Master's sidebar opens without the demo refusing anything", async () => {
  // T023. A fresh page load, so what was refused is this walk's alone.
  await page.goto(`/demo/world/${WORLD_ID}/staging`);
  const nav = page.getByTestId("world-sidebar-nav");
  await expect(nav).toBeVisible();
  const links = await nav.getByRole("link").evaluateAll((anchors) =>
    anchors.map((a) => ({
      name: (a.textContent ?? "").trim(),
      href: a.getAttribute("href") ?? "",
    })),
  );
  expect(links.length).toBeGreaterThanOrEqual(8);

  for (const link of links) {
    await nav.getByRole("link", { name: link.name, exact: true }).click();
    await expect(page, link.name).toHaveURL(
      new RegExp(link.href.replace(/[?]/g, "\\?") + "$"),
    );
    await page.waitForLoadState("networkidle");
    expect(await refusedSoFar(), `refused on ${link.name}`).toEqual([]);
    await expect(
      page.getByText("Not part of the demo"),
      `toast on ${link.name}`,
    ).toHaveCount(0);
  }
});

test("every example map is a scene that opens, credited to its author", async () => {
  await page.goto(`/demo/world/${WORLD_ID}/scenes`);
  const rows = page.getByTestId("scenes-table").locator("tbody tr");
  await expect(rows).toHaveCount(7);
  for (const scene of await scenes()) {
    await page.goto(`/demo/world/${WORLD_ID}/scenes/${scene.sceneId}`);
    await expect(page.getByTestId("scene-not-found")).toHaveCount(0);
    await expect(
      page.getByRole("heading", { name: scene.name }).first(),
    ).toBeVisible();
    await expect(page.getByTestId("demo-notice")).toContainText("MBRound18");
  }
  const notice = await page.request.get("/demo/maps/NOTICE.txt");
  expect(notice.ok()).toBe(true);
  expect(await notice.text()).toContain("MBRound18");
});
