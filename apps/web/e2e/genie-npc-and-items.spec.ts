import { test, expect, type Page } from "@playwright/test";

/**
 * specs/018-genie-house-system, User Story 3 (Scenario 3, NPC size
 * category -> token footprint) and User Story 5 (Scenario 5, a
 * Wish-Granted Item with a mechanical effect in a character's inventory).
 *
 * Real gaps found while building this file (documented rather than
 * forced, per task scope):
 * - `NpcCompendiumTab`'s "Add" form has no field to set an NPC's
 *   `trait_data.size_category` — only `label`/`description`. There is
 *   no UI anywhere to set an actor's `trait_data` at all (confirmed via
 *   grep: no `updateActorSystemData` caller exists in any page/component
 *   except the RxDB-backed `dnd5e/CharacterSheet.tsx`, which is itself
 *   never routed anywhere in apps/web). So `trait_data.size_category` is
 *   set here via the real `updateActorSystemData` GraphQL mutation
 *   directly through the live session.
 * - FIXED since this file was first written: `TokenPanel`'s NPC
 *   size-category -> scale resolution (spec 018 T047) used to read
 *   `trait_data` via `useActorSystemData`, which queried a client-side
 *   RxDB `world_actor_system_data` collection that had no pull/push
 *   replication ever registered for it, so it never reflected a
 *   server-side `updateActorSystemData` write. The RxDB hard-cut
 *   (apps/web/src/hooks/useActorSystemData.ts) replaced that with a
 *   direct GraphQL fetch against the new `actorSystemData(actorId)`
 *   query (apps/web/src/api/actorSystemData.ts,
 *   src/server/src/graphql/queries/actor.rs's `actor_system_data_impl`),
 *   so `resolveSizeScale` (apps/web/src/utils/sizeCategory.ts,
 *   unit-tested in apps/web/src/utils/__tests__/sizeCategory.test.ts) now
 *   receives real trait_data in the running app. This test previously
 *   used `test.fail()` to flag the confirmed regression; it now asserts
 *   the real (fixed) behavior.
 * - `ActorInventoryPanel`'s inventory row only renders `itemName` and
 *   `quantity` (`InventoryEntryRecord` in apps/web/src/types/inventory.ts
 *   has no `description`/`effects` fields at all) — so Acceptance
 *   Scenario 1 of User Story 5 ("the item's name, description, and
 *   mechanical effect are all visible on the character's inventory
 *   view") is only PARTIALLY met by the real running app: the item's
 *   name and its presence in inventory are real and verified below: its
 *   description and effect are verified via the item's own detail page
 *   (where they ARE genuinely wired, spec 013), not via the inventory
 *   view itself, which is a real, scoped gap in spec 013's UI that this
 *   spec did not attempt to redesign.
 */

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

interface Credentials {
  username: string;
  email: string;
  password: string;
}

function freshCredentials(prefix: string): Credentials {
  const suffix = uniqueSuffix();
  const username = `${prefix}${suffix}`;
  return {
    username,
    email: `${username}@example.test`,
    password: "Sup3r-Secret-Passphrase!",
  };
}

async function register(page: Page, creds: Credentials): Promise<void> {
  await page.goto("/register");
  await page.locator("#register-username").fill(creds.username);
  await page.locator("#register-email").fill(creds.email);
  await page.locator("#register-password").fill(creds.password);
  await page.locator("#register-password-confirmation").fill(creds.password);
  await page.getByRole("button", { name: "Create account" }).click();
}

async function registerAndCreateWorld(page: Page, worldName: string): Promise<string> {
  await register(page, freshCredentials("e2egnpc"));
  await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const match = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname);
  if (!match) {
    throw new Error(`Could not extract world id from URL: ${page.url()}`);
  }
  return match[1];
}

async function assignGenieSystem(page: Page, worldId: string): Promise<void> {
  await page.goto(`/world/${worldId}/settings/system`);
  await page.getByTestId("system-picker").click();
  await page.getByRole("option", { name: "genie" }).click();
  await page.getByRole("button", { name: "Confirm" }).click();
  await expect(page.getByText("System assigned.")).toBeVisible({ timeout: 10_000 });
}

async function graphql<T>(page: Page, query: string, variables: Record<string, unknown>): Promise<T> {
  return page.evaluate(
    async ({ query, variables }) => {
      const csrfToken = document.cookie
        .split(";")
        .map((part) => part.trim())
        .find((part) => part.startsWith("csrf_token="))
        ?.slice("csrf_token=".length);
      const res = await fetch("/api/graphql", {
        method: "POST",
        credentials: "same-origin",
        headers: {
          "Content-Type": "application/json",
          ...(csrfToken ? { "x-csrf-token": csrfToken } : {}),
        },
        body: JSON.stringify({ query, variables }),
      });
      const text = await res.text();
      try {
        return JSON.parse(text);
      } catch {
        throw new Error(`Non-JSON response (status ${res.status}): ${text.slice(0, 500)}`);
      }
    },
    { query, variables },
  );
}

async function createGenieActor(
  page: Page,
  worldId: string,
  label: string,
  isNpc: boolean,
): Promise<string> {
  const result = await graphql<{ data: { createActor: { id: string } } }>(
    page,
    `mutation($input: CreateActorInput!) { createActor(input: $input) { id } }`,
    { input: { worldId, label, isNpc, gameSystemId: "genie" } },
  );
  return result.data.createActor.id;
}

async function setTraitData(
  page: Page,
  actorId: string,
  traitData: Record<string, unknown>,
): Promise<void> {
  const result = await graphql<{ data?: unknown; errors?: { message: string }[] }>(
    page,
    `mutation($input: GraphQLUpdateActorSystemDataInput!) { updateActorSystemData(input: $input) { id } }`,
    {
      input: {
        actorId,
        gameSystemId: "genie",
        dataType: "trait_data",
        data: traitData,
      },
    },
  );
  if (result.errors?.length) {
    throw new Error(`updateActorSystemData failed: ${result.errors[0].message}`);
  }
}

async function clickPlay(page: Page): Promise<void> {
  await page.getByTestId("play-button").click();
  await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });
}

async function waitForEngineReady(page: Page): Promise<void> {
  const canvas = page.locator("canvas");
  await expect(canvas).toBeVisible({ timeout: 15_000 });
  await page.waitForTimeout(3_000);
}

async function ensureSidebarOpen(page: Page): Promise<void> {
  const toggle = page.getByTestId("token-panel-toggle-button");
  if (!(await toggle.isVisible().catch(() => false))) {
    await page.getByTestId("sidebar-toggle-button").click({ force: true });
    await expect(toggle).toBeVisible({ timeout: 10_000 });
  }
}

test.describe("Spec 018 Scenario 3: NPC size category sets a token's default footprint", () => {
  // Previously flagged via test.fail() as a confirmed regression (see file
  // header): TokenPanel's NPC scale hint never reflected a real NPC's
  // trait_data.size_category because nothing in the running app populated
  // the RxDB collection it read from. The RxDB hard-cut replaced that read
  // path with a direct GraphQL fetch, so this now asserts the real,
  // working behavior.
  test(
    "a colossal NPC's token defaults to a larger scale than a diminutive NPC's",
    async ({ page }) => {
    test.setTimeout(90_000);
    const worldId = await registerAndCreateWorld(page, `E2E Genie NPCs ${uniqueSuffix()}`);
    await assignGenieSystem(page, worldId);

    const diminutiveId = await createGenieActor(page, worldId, "Minor Sprite", true);
    await setTraitData(page, diminutiveId, { size_category: "diminutive" });

    const colossalId = await createGenieActor(page, worldId, "Towering Elemental Servant", true);
    await setTraitData(page, colossalId, { size_category: "colossal" });

    await page.goto(`/world/${worldId}/staging`);
    await clickPlay(page);
    await waitForEngineReady(page);
    await ensureSidebarOpen(page);

    // Diminutive NPC's token. `trait_data` was set via a raw GraphQL
    // call above (no UI path exists to set it — see file header), and
    // TokenPanel reads it via `useActorSystemData`'s direct GraphQL fetch
    // on mount. Retry selecting the NPC until the scale hint reflects the
    // real value, rather than asserting on the very first render, and
    // reload first to force a fresh fetch.
    await page.reload();
    await waitForEngineReady(page);
    await page.getByTestId("token-panel-toggle-button").click({ force: true });
    await page.getByTestId("token-create-trigger").click({ force: true });
    let diminutiveHintText = "";
    for (let attempt = 0; attempt < 10; attempt++) {
      await page.getByTestId("token-create-npc-select").selectOption({ label: "(blank token)" });
      await page.getByTestId("token-create-npc-select").selectOption({ label: "Minor Sprite" });
      diminutiveHintText = (await page.getByTestId("token-create-npc-scale-hint").innerText().catch(() => "")) ?? "";
      if (/0\.5/.test(diminutiveHintText)) break;
      await page.waitForTimeout(1_000);
    }
    expect(
      diminutiveHintText,
      "TokenPanel's NPC scale hint never picked up trait_data.size_category (GraphQL fetch of a server-side updateActorSystemData write) within 10s.",
    ).toMatch(/0\.5/);
    const [createDiminutiveResp] = await Promise.all([
      page.waitForResponse(
        (r) => r.url().includes("/api/graphql") && (r.request().postData() ?? "").includes("createToken"),
      ),
      page.getByTestId("token-create-submit").click({ force: true }),
    ]);
    const diminutiveTokenBody = (await createDiminutiveResp.json()) as {
      data?: { createToken?: { tokenId?: string; scale?: number } };
    };
    expect(diminutiveTokenBody.data?.createToken?.scale).toBeCloseTo(0.5, 5);
    await page.keyboard.press("Escape");

    // Colossal NPC's token.
    await page.getByTestId("token-create-trigger").click({ force: true });
    let colossalHintText = "";
    for (let attempt = 0; attempt < 10; attempt++) {
      await page.getByTestId("token-create-npc-select").selectOption({ label: "(blank token)" });
      await page.getByTestId("token-create-npc-select").selectOption({ label: "Towering Elemental Servant" });
      colossalHintText = (await page.getByTestId("token-create-npc-scale-hint").innerText().catch(() => "")) ?? "";
      if (/\b4\b/.test(colossalHintText)) break;
      await page.waitForTimeout(1_000);
    }
    expect(colossalHintText).toMatch(/\b4\b/);
    const [createColossalResp] = await Promise.all([
      page.waitForResponse(
        (r) => r.url().includes("/api/graphql") && (r.request().postData() ?? "").includes("createToken"),
      ),
      page.getByTestId("token-create-submit").click({ force: true }),
    ]);
    const colossalTokenBody = (await createColossalResp.json()) as {
      data?: { createToken?: { tokenId?: string; scale?: number } };
    };
    expect(colossalTokenBody.data?.createToken?.scale).toBeCloseTo(4.0, 5);

    // Acceptance scenarios 1 & 2: the colossal token's footprint is
    // proportional to (here, 8x) the diminutive token's.
    expect(colossalTokenBody.data!.createToken!.scale!).toBeGreaterThan(
      diminutiveTokenBody.data!.createToken!.scale!,
    );

    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
    await page.screenshot({ path: "e2e/screenshots/genie-npc-size-category-tokens.png" });
  });
});

test.describe("Spec 018 Scenario 5: a Wish-Granted Item with a mechanical effect in inventory", () => {
  test("a Wish-Granted Item with a formula-bearing effect is added to a character's inventory via the real items UI", async ({
    page,
  }) => {
    test.setTimeout(90_000);
    const worldId = await registerAndCreateWorld(page, `E2E Genie Items ${uniqueSuffix()}`);
    await assignGenieSystem(page, worldId);

    const actorId = await createGenieActor(page, worldId, "Test Genie PC", false);

    // Create the Wish-Granted Item via the real compendium UI.
    await page.goto(`/world/${worldId}/compendium`);
    await page.getByRole("tab", { name: "Items" }).click();
    await page.getByTestId("new-item-name-input").fill("Lamp of Minor Binding");
    await page
      .getByTestId("new-item-description-input")
      .fill("A tarnished brass lamp that suppresses a bound Genie's power while held.");
    const [createItemResp] = await Promise.all([
      page.waitForResponse(
        (r) => r.url().includes("/api/graphql") && (r.request().postData() ?? "").includes("createItem"),
      ),
      page.getByTestId("add-item-button").click(),
    ]);
    const createItemBody = (await createItemResp.json()) as { data?: { createItem?: { id?: string } } };
    const itemId = createItemBody.data?.createItem?.id;
    expect(itemId).toBeTruthy();

    // Add a formula-bearing effect via the real item detail/edit UI.
    await page.goto(`/world/${worldId}/item/${itemId}/edit`);
    await expect(page.getByTestId("item-effect-editor")).toBeVisible({ timeout: 10_000 });
    await page.getByTestId("new-item-effect-formula").fill("1d4");
    await page.getByTestId("new-item-effect-target").fill("Binding Suppression");
    await page.getByTestId("add-item-effect-button").click();
    await expect(page.getByTestId(/^item-effect-row-/)).toBeVisible({ timeout: 10_000 });

    // Add the item to the Genie character's inventory via the real
    // ActorInventoryPanel UI.
    await page.goto(`/world/${worldId}/actor/${actorId}/view`);
    await expect(page.getByTestId("actor-inventory-panel")).toBeVisible({ timeout: 10_000 });
    await page.getByTestId("inventory-add-item-select").selectOption({ label: "Lamp of Minor Binding" });
    await page.getByTestId("inventory-add-quantity-input").fill("1");
    await page.getByTestId("add-item-button").isVisible().catch(() => false); // no-op guard
    await page.getByTestId("inventory-add-button").click();

    // Scope to the entry row specifically (not `inventory-add-item-select`'s
    // own `<option>`, which also matches the same text within the panel).
    const entryRow = page.locator('[data-testid^="inventory-entry-"]').first();
    await expect(entryRow).toBeVisible({ timeout: 10_000 });
    await expect(entryRow).toContainText("Lamp of Minor Binding");
    await expect(entryRow).toContainText("Quantity: 1");

    // The item's description and effect ARE real and visible, but on the
    // item's own detail page (spec 013's actual wiring) rather than in
    // the inventory row itself (see file header note on this gap).
    await page.goto(`/world/${worldId}/item/${itemId}/view`);
    await expect(page.getByText("A tarnished brass lamp that suppresses a bound Genie's power while held.")).toBeVisible();
    const effectRow = page.getByTestId(/^item-effect-row-/).first();
    await expect(effectRow).toBeVisible({ timeout: 10_000 });
    // The effect's formula/target render as editable inputs (ItemEffectEditor
    // is always in "edit" shape, regardless of the page's own view/edit
    // mode) — assert on their values, not on text-node content.
    await expect(effectRow.locator("input").nth(0)).toHaveValue("1d4");
    await expect(effectRow.locator("input").nth(1)).toHaveValue("Binding Suppression");
  });
});
