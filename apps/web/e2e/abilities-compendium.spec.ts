import { test, expect, type Page } from "@playwright/test";

/**
 * specs/025-world-abilities-compendium, User Story 1 (T020): the Compendium's
 * Abilities tab, replacing spec 011's placeholder — the last one (SC-001).
 *
 * Runnable in this sandbox, unlike most e2e here: this feature has **no Bevy
 * canvas surface**, so it escapes the documented "headless Chromium can't
 * render the canvas" limitation that blocks every canvas-interaction spec in
 * this repo. Same property that let spec 005's `live-sync.spec.ts` pass.
 *
 * ⚠️ KNOWN PRE-EXISTING FLAKE (~1 run in 3, not caused by this feature):
 * the `/world/:id/compendium` route's lazy chunk intermittently never
 * resolves under the Vite dev server, leaving the page stuck on the
 * `renderLazyPage` Suspense fallback ("Loading world compendium") and no
 * compendium markup at all. Verified pre-existing by stashing every
 * `apps/web/src` change from spec 025 and reproducing the identical hang on
 * the untouched baseline. It is a dev-server module-loading problem, not a
 * product defect and not specific to the Abilities tab — it affects any e2e
 * test that routes to the Compendium. `openAbilitiesTab` below retries the
 * navigation to absorb it; the underlying flake deserves its own fix.
 */

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

async function registerGm(page: Page): Promise<void> {
  const username = `e2eabil${uniqueSuffix()}`;
  await page.goto("/register");
  await page.locator("#register-username").fill(username);
  await page.locator("#register-email").fill(`${username}@example.test`);
  await page.locator("#register-password").fill("Sup3r-Secret-Passphrase!");
  await page.locator("#register-password-confirmation").fill("Sup3r-Secret-Passphrase!");
  await page.getByRole("button", { name: "Create account" }).click();
  await page.waitForURL((url) => !url.pathname.startsWith("/register"), { timeout: 15_000 });
}

/**
 * Creates a world and returns its id.
 *
 * The retry is not defensive padding — it fixes a real, reproducible race:
 * `registerGm` resolves as soon as the URL leaves `/register`, which can be
 * before the auth context has hydrated. Navigating straight to
 * `/worlds/create` then hits `RequireAuthenticated` while `isAuthenticated`
 * is still false and gets bounced to `/login`, so `#world-name` never
 * appears and the fill hangs for the full test timeout. Re-navigating once
 * the context has settled lands correctly. (Observed ~1 run in 5 before this.)
 */
async function createWorld(page: Page, name: string): Promise<string> {
  const nameField = page.locator("#world-name");

  await page.goto("/worlds/create");
  try {
    await expect(nameField).toBeVisible({ timeout: 5_000 });
  } catch {
    // Auth hydration lost the first navigation — try once more.
    await page.goto("/worlds/create");
    await expect(nameField).toBeVisible({ timeout: 15_000 });
  }

  await nameField.fill(name);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const match = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname);
  if (!match) {
    throw new Error(`Could not extract world id from ${page.url()}`);
  }
  return match[1];
}

/**
 * Navigates to the Abilities tab, retrying once if the route's lazy chunk
 * hangs — see the pre-existing-flake note at the top of this file.
 */
async function openAbilitiesTab(page: Page, worldId: string): Promise<void> {
  const search = page.getByTestId("ability-catalog-search-input");
  await page.goto(`/world/${worldId}/compendium?tab=abilities`);
  try {
    await expect(search).toBeVisible({ timeout: 12_000 });
  } catch {
    await page.goto(`/world/${worldId}/compendium?tab=abilities`);
    await expect(search).toBeVisible({ timeout: 20_000 });
  }
}

test.describe("Abilities compendium (US1)", () => {
  test("a GM creates an ability, finds it by search, and previews it", async ({ page }) => {
    await registerGm(page);
    const worldId = await createWorld(page, `E2E Abilities ${uniqueSuffix()}`);

    await openAbilitiesTab(page, worldId);

    // SC-001: the placeholder is gone from the whole app, not just visually.
    await expect(page.getByTestId("compendium-coming-soon")).toHaveCount(0);

    // Empty state, not a blank table or fabricated rows.
    await expect(page.getByText("No Abilities yet.")).toBeVisible();

    // Create — appears without a reload.
    const name = `Fireball ${uniqueSuffix()}`;
    await page.getByTestId("new-ability-name-input").fill(name);
    await page.getByTestId("new-ability-description-input").fill("A roaring ball of flame.");
    await page.getByTestId("add-ability-button").click();

    const table = page.getByTestId("ability-catalog-table");
    await expect(table).toBeVisible({ timeout: 15_000 });
    await expect(table).toContainText(name);

    // Search narrows the table (SC-003).
    await page.getByTestId("ability-catalog-search-input").fill(name);
    await expect(table).toContainText(name);
    await page.getByTestId("ability-catalog-search-input").fill("definitely-no-such-ability");
    await expect(page.getByText(/No abilities match/)).toBeVisible({ timeout: 15_000 });
    await page.getByTestId("ability-catalog-search-input").fill("");

    // Row select opens the preview panel beside the table.
    await expect(page.getByTestId("ability-preview-panel-empty")).toBeVisible();
    await page.getByRole("cell", { name, exact: false }).first().click();
    const preview = page.getByTestId("ability-preview-panel");
    await expect(preview).toBeVisible({ timeout: 15_000 });
    await expect(preview).toContainText(name);
    await expect(preview).toContainText("A roaring ball of flame.");
  });

  test("duplicate names are allowed and only advisory (FR-006/FR-007)", async ({ page }) => {
    await registerGm(page);
    const worldId = await createWorld(page, `E2E Ability Dupes ${uniqueSuffix()}`);
    await openAbilitiesTab(page, worldId);

    const name = `Cleave ${uniqueSuffix()}`;
    await page.getByTestId("new-ability-name-input").fill(name);
    await page.getByTestId("add-ability-button").click();
    await expect(page.getByTestId("ability-catalog-table")).toContainText(name, {
      timeout: 15_000,
    });

    // Typing the same name surfaces a non-blocking "did you mean?" hint...
    await page.getByTestId("new-ability-name-input").fill(name);
    await expect(page.getByTestId("ability-name-suggestion")).toBeVisible({ timeout: 15_000 });

    // ...and creating anyway succeeds — the hint never gates (FR-006).
    await page.getByTestId("add-ability-button").click();
    await expect(page.getByTestId("ability-catalog-table").getByText(name, { exact: false })).toHaveCount(
      2,
      { timeout: 15_000 },
    );
  });

  test("a GM can hide an ability, and the badge reflects it (FR-024a/FR-024d)", async ({
    page,
  }) => {
    await registerGm(page);
    const worldId = await createWorld(page, `E2E Ability Hidden ${uniqueSuffix()}`);
    await openAbilitiesTab(page, worldId);

    const name = `Soul Harvest ${uniqueSuffix()}`;
    await page.getByTestId("new-ability-name-input").fill(name);
    await page.getByTestId("add-ability-button").click();
    await expect(page.getByTestId("ability-catalog-table")).toContainText(name, {
      timeout: 15_000,
    });

    // Not hidden to begin with (FR-024a default).
    await expect(page.locator('[data-testid^="ability-gm-only-badge-"]')).toHaveCount(0);

    // Open its detail page and hide it.
    await page.locator('[data-testid^="ability-catalog-view-"]').first().click();
    await page.waitForURL(/\/ability\/[^/]+\/view$/, { timeout: 15_000 });
    await page.getByTestId("ability-gm-only-toggle").click();
    await expect(page.getByTestId("ability-gm-only-badge")).toBeVisible({ timeout: 15_000 });

    // The DM still sees it in the catalog, now marked (FR-024d) — a player
    // would not see it at all, which is covered server-side by
    // gm_only_ability_is_absent_from_every_non_dm_surface.
    await openAbilitiesTab(page, worldId);
    await expect(page.locator('[data-testid^="ability-gm-only-badge-"]')).toHaveCount(1, {
      timeout: 15_000,
    });
  });

  /** quickstart.md Scenario 2 (US2): structured effects. */
  test("a GM adds, edits, and removes effects; invalid formulas are rejected", async ({
    page,
  }) => {
    await registerGm(page);
    const worldId = await createWorld(page, `E2E Ability Effects ${uniqueSuffix()}`);
    await openAbilitiesTab(page, worldId);

    const name = `Lightning ${uniqueSuffix()}`;
    await page.getByTestId("new-ability-name-input").fill(name);
    await page.getByTestId("add-ability-button").click();
    await expect(page.getByTestId("ability-catalog-table")).toContainText(name, {
      timeout: 15_000,
    });

    // Effects are edited on the detail page, in edit mode.
    await page.locator('[data-testid^="ability-catalog-edit-"]').first().click();
    await page.waitForURL(/\/ability\/[^/]+\/edit$/, { timeout: 15_000 });
    const editor = page.getByTestId("ability-effect-editor");
    await expect(editor).toBeVisible({ timeout: 15_000 });
    await expect(editor).toContainText("No effects yet.");

    // FR-018: a formula with no letters or digits is rejected, nothing saved.
    await page.getByTestId("new-ability-effect-formula").fill("+++");
    await page.getByTestId("new-ability-effect-target").fill("Hit Points");
    await page.getByTestId("add-ability-effect-button").click();
    await expect(editor).toContainText(/at least one letter or digit/, { timeout: 15_000 });
    await expect(page.locator('[data-testid^="ability-effect-row-"]')).toHaveCount(0);

    // A valid effect saves.
    await page.getByTestId("new-ability-effect-formula").fill("3d6");
    await page.getByTestId("add-ability-effect-button").click();
    await expect(page.locator('[data-testid^="ability-effect-row-"]')).toHaveCount(1, {
      timeout: 15_000,
    });

    // A second, independent effect (FR-017).
    await page.getByTestId("new-ability-effect-formula").fill("1d20 + STAT");
    await page.getByTestId("new-ability-effect-target").fill("Attack Roll");
    await page.getByTestId("add-ability-effect-button").click();
    await expect(page.locator('[data-testid^="ability-effect-row-"]')).toHaveCount(2, {
      timeout: 15_000,
    });

    // Removing one leaves the other untouched.
    await page.locator('[data-testid^="ability-effect-remove-"]').first().click();
    await expect(page.locator('[data-testid^="ability-effect-row-"]')).toHaveCount(1, {
      timeout: 15_000,
    });
    // The surviving row's formula lives in an input value, not the card's
    // text — assert on the value, not innerText.
    await expect(
      page
        .locator('[data-testid^="ability-effect-row-"]')
        .first()
        .getByLabel("Formula"),
    ).toHaveValue("1d20 + STAT");

    // And the surviving effect shows in the Compendium preview panel.
    await openAbilitiesTab(page, worldId);
    await page.getByRole("cell", { name, exact: false }).first().click();
    const preview = page.getByTestId("ability-preview-panel");
    await expect(preview).toBeVisible({ timeout: 15_000 });
    await expect(preview).toContainText("1d20 + STAT");
    await expect(preview).toContainText("Attack Roll");
  });
});
