import { test, expect, type Page } from "@playwright/test";

/**
 * specs/033-abilities-vocabulary, User Story 2 — changing a world's system
 * warns with real numbers and asks twice.
 *
 * SC-007 says the rule holds "including attempts that call the operation
 * directly without the interface", so the last test here skips the browser
 * entirely. A guard that only exists in a dialog is not a guard.
 */

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

async function registerGm(page: Page): Promise<void> {
  const suffix = uniqueSuffix();
  await page.goto("/register");
  await page.locator("#register-username").fill(`e2eguard${suffix}`);
  await page.locator("#register-email").fill(`e2eguard${suffix}@example.test`);
  await page.locator("#register-password").fill("Sup3r-Secret-Passphrase!");
  await page
    .locator("#register-password-confirmation")
    .fill("Sup3r-Secret-Passphrase!");
  await page.getByRole("button", { name: "Create account" }).click();
  await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
}

async function createWorld(page: Page, name: string): Promise<string> {
  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(name);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const match = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname);
  if (!match) throw new Error(`Could not extract world id from ${page.url()}`);
  return match[1];
}

async function graphql(page: Page, query: string, variables: unknown) {
  const cookies = await page.context().cookies();
  const csrf = cookies.find((cookie) => cookie.name === "csrf_token")?.value;
  const response = await page.request.post("/api/graphql", {
    headers: csrf ? { "x-csrf-token": csrf } : {},
    data: { query, variables },
  });
  return (await response.json()) as {
    data?: Record<string, unknown>;
    errors?: { message: string }[];
  };
}

/** Give the world something to lose, so the guard has a reason to fire. */
async function addAnAbility(page: Page, worldId: string): Promise<void> {
  await page.goto(`/world/${worldId}/compendium?tab=abilities`);
  await expect(page.getByTestId("ability-type-tabs")).toBeVisible({
    timeout: 20_000,
  });
  await page.getByTestId("new-ability-name-input").fill(`Ward ${uniqueSuffix()}`);
  await page.getByTestId("add-ability-button").click();
  await expect(page.getByTestId("ability-catalog-table")).toBeVisible({
    timeout: 15_000,
  });
}

async function pickSystem(page: Page, worldId: string, title: string) {
  await page.goto(`/world/${worldId}/settings/system`);
  const picker = page.getByTestId("system-picker");
  await expect(picker).toBeVisible({ timeout: 15_000 });
  await picker.click();
  await page.getByRole("option", { name: title }).click();
}

test.describe("US2: a system change is counted, red, and asked twice", () => {
  test("a world with content shows real counts and needs two confirmations", async ({
    page,
  }) => {
    await registerGm(page);
    const worldId = await createWorld(page, `Guard ${uniqueSuffix()}`);
    await addAnAbility(page, worldId);

    await pickSystem(page, worldId, "5E System Core");

    // FR-025: severe, and specific. The counts come from the world, not a
    // generic sentence.
    const warning = page.getByTestId("system-change-warning");
    await expect(warning).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId("system-change-counts")).toContainText("1 ability");

    // FR-026: it must not overstate. Nothing is destroyed here, and saying so
    // would be false — a false warning is worse than none, because it teaches
    // a GM to distrust the true ones.
    //
    // Asserted against *claims* rather than words. A bare `not.toContain(
    // "delete")` fails on "nothing is deleted", which is the reassurance this
    // requirement is asking for — the first version of this test did exactly
    // that and called honest wording a violation.
    const text = (await warning.innerText()).toLowerCase();
    for (const claim of [
      "will be deleted",
      "will be lost",
      "will be destroyed",
      "permanently",
      "cannot be undone",
    ]) {
      expect(text, `the warning must not claim "${claim}"`).not.toContain(claim);
    }
    expect(text).toContain("hides this content");
    expect(text).toContain("switching back restores");

    // FR-027: one confirmation is not enough. The legal-notice confirmation
    // does not even appear until the data risk is accepted.
    await expect(page.getByTestId("pending-system-confirmation")).toHaveCount(0);
    await page.getByTestId("system-change-accept-risk").click();

    const confirmation = page.getByTestId("pending-system-confirmation");
    await expect(confirmation).toBeVisible({ timeout: 15_000 });
    // ...and the second names the system being switched to.
    await expect(confirmation).toContainText("5E System Core");

    await confirmation.getByRole("button", { name: /confirm/i }).click();
    await expect(page.getByTestId("active-system-card")).toContainText(
      "5E System Core",
      { timeout: 15_000 },
    );
  });

  test("cancelling at the warning leaves the world's system and content alone", async ({
    page,
  }) => {
    await registerGm(page);
    const worldId = await createWorld(page, `Guard cancel ${uniqueSuffix()}`);
    await addAnAbility(page, worldId);

    const before = await graphql(
      page,
      `query C($worldId: UUID!) {
        worldContentInventory(worldId: $worldId) { counts { kind count } }
      }`,
      { worldId },
    );

    await pickSystem(page, worldId, "5E System Core");
    await expect(page.getByTestId("system-change-warning")).toBeVisible({
      timeout: 15_000,
    });
    await page.getByTestId("system-change-cancel").click();

    // FR-032: nothing moved.
    await expect(page.getByTestId("system-change-warning")).toHaveCount(0);
    const after = await graphql(
      page,
      `query C($worldId: UUID!) {
        worldContentInventory(worldId: $worldId) { counts { kind count } }
      }`,
      { worldId },
    );
    expect(after.data?.worldContentInventory).toEqual(
      before.data?.worldContentInventory,
    );
  });

  test("an empty world switches in one step, with no warning", async ({ page }) => {
    // FR-029. Its auto-created default scene does not make it non-empty —
    // every world has one, so counting scenes would put the red panel in front
    // of a GM on a world they made a minute ago, and a warning shown when
    // nothing is at stake is one people learn to click through.
    await registerGm(page);
    const worldId = await createWorld(page, `Guard empty ${uniqueSuffix()}`);

    await pickSystem(page, worldId, "5E System Core");

    await expect(page.getByTestId("pending-system-confirmation")).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.getByTestId("system-change-warning")).toHaveCount(0);
  });

  test("the guard holds against a direct call, with no acknowledgement and with a stale one", async ({
    page,
  }) => {
    // SC-007's real test. Everything above can be skipped by not using the
    // interface; this cannot.
    await registerGm(page);
    const worldId = await createWorld(page, `Guard api ${uniqueSuffix()}`);
    await addAnAbility(page, worldId);

    const MUTATION = `mutation U($input: UpdateWorldGameSystemInput!) {
      updateWorldGameSystem(input: $input) { id gameSystemId }
    }`;

    const unacknowledged = await graphql(page, MUTATION, {
      input: { worldId, gameSystemId: "dnd5e" },
    });
    expect(
      unacknowledged.errors?.length,
      "a world with content must refuse an unacknowledged change",
    ).toBeTruthy();

    const forged = await graphql(page, MUTATION, {
      input: { worldId, gameSystemId: "dnd5e", acknowledgedDigest: "0000000000000000" },
    });
    expect(forged.errors?.length, "a made-up digest must be refused").toBeTruthy();

    // A real digest, then the world changes underneath it.
    const counted = await graphql(
      page,
      `query C($worldId: UUID!, $target: String) {
        worldContentInventory(worldId: $worldId, targetSystemId: $target) { digest }
      }`,
      { worldId, target: "dnd5e" },
    );
    const digest = (
      counted.data?.worldContentInventory as { digest: string } | undefined
    )?.digest;
    expect(digest).toBeTruthy();

    await addAnAbility(page, worldId);

    const stale = await graphql(page, MUTATION, {
      input: { worldId, gameSystemId: "dnd5e", acknowledgedDigest: digest },
    });
    expect(
      stale.errors?.length,
      "a digest taken before the world changed must not still acknowledge it",
    ).toBeTruthy();

    // And the world is still on its original system.
    const world = await graphql(
      page,
      `query W($worldId: UUID!) { world(id: $worldId) { gameSystemId } }`,
      { worldId },
    );
    const system = (
      world.data?.world as { gameSystemId: string | null } | undefined
    )?.gameSystemId;
    expect(system).not.toBe("dnd5e");
  });
});
