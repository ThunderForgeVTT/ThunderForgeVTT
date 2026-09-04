import { test, expect, type Page } from "@playwright/test";

/**
 * specs/033-abilities-vocabulary, User Story 1 — the compendium breaks out
 * ability types in the system's own words.
 *
 * SC-004 names four systems and asks that each render *its own* tab set from
 * *its own* declarations. That is why this file loops rather than testing one:
 * a tab set built against a single system passes for the wrong reason, and the
 * case most likely to break is the system that declares nothing at all.
 */

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

async function registerGm(page: Page): Promise<void> {
  const suffix = uniqueSuffix();
  await page.goto("/register");
  await page.locator("#register-username").fill(`e2evocab${suffix}`);
  await page.locator("#register-email").fill(`e2evocab${suffix}@example.test`);
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

/** Bind the world to a system through its settings, acknowledging the legal notice. */
async function chooseSystem(
  page: Page,
  worldId: string,
  title: string,
): Promise<void> {
  await page.goto(`/world/${worldId}/settings/system`);
  const picker = page.getByTestId("system-picker");
  await expect(picker).toBeVisible({ timeout: 15_000 });
  await picker.click();
  await page.getByRole("option", { name: title }).click();
  const confirmation = page.getByTestId("pending-system-confirmation");
  await expect(confirmation).toBeVisible({ timeout: 15_000 });
  await confirmation.getByRole("button", { name: /confirm|assign/i }).click();
  await expect(page.getByTestId("active-system-card")).toContainText(title, {
    timeout: 15_000,
  });
}

/** The compendium's abilities area, retrying the lazy chunk once. */
async function openAbilities(page: Page, worldId: string): Promise<void> {
  const tabs = page.getByTestId("ability-type-tabs");
  await page.goto(`/world/${worldId}/compendium?tab=abilities`);
  try {
    await expect(tabs).toBeVisible({ timeout: 12_000 });
  } catch {
    await page.goto(`/world/${worldId}/compendium?tab=abilities`);
    await expect(tabs).toBeVisible({ timeout: 20_000 });
  }
}

/** What the server says this world calls its abilities — the same answer the
 * page renders from, so the assertions below compare the UI against the
 * system's declarations rather than against words hardcoded here. */
async function csrfHeader(page: Page): Promise<Record<string, string>> {
  const cookies = await page.context().cookies();
  const csrf = cookies.find((cookie) => cookie.name === "csrf_token")?.value;
  return csrf ? { "x-csrf-token": csrf } : {};
}

async function vocabularyOf(page: Page, worldId: string) {
  // The app sends the double-submit CSRF token on every GraphQL call
  // (`withCsrf` in `api/auth.ts`). A request made through the context shares
  // the cookies but not that header, so it has to be lifted across by hand.
  const cookies = await page.context().cookies();
  const csrf = cookies.find((cookie) => cookie.name === "csrf_token")?.value;

  const response = await page.request.post("/api/graphql", {
    headers: csrf ? { "x-csrf-token": csrf } : {},
    data: {
      query: `query V($worldId: UUID!) {
        abilityVocabulary(worldId: $worldId) {
          umbrella { label pluralLabel }
          types { id label pluralLabel order builtin }
        }
      }`,
      variables: { worldId },
    },
  });
  const body = (await response.json()) as {
    data: {
      abilityVocabulary: {
        umbrella: { label: string; pluralLabel: string };
        types: { id: string; pluralLabel: string }[];
      };
    };
  };
  return body.data.abilityVocabulary;
}

const SYSTEMS = [
  { id: "genie", title: "Genie" },
  { id: "dnd5e", title: "5E System Core" },
  { id: "pathfinder2e", title: "Pathfinder Second Edition" },
  { id: "blades_in_the_dark", title: "Blades in the Dark / Forged in the Dark" },
];

test.describe("US1: every system's own tab set, in its own words", () => {
  for (const system of SYSTEMS) {
    test(`${system.title} presents its own ability types`, async ({ page }) => {
      await registerGm(page);
      const worldId = await createWorld(page, `Vocab ${system.id} ${uniqueSuffix()}`);
      await chooseSystem(page, worldId, system.title);

      const vocabulary = await vocabularyOf(page, worldId);
      expect(
        vocabulary.types.length,
        "every system must present at least one type — a blank tab set is SC-013's failure",
      ).toBeGreaterThan(0);

      await openAbilities(page, worldId);

      // FR-002 and FR-004: one tab per type the system presents, carrying that
      // system's plural word, in that system's order.
      const tabs = page.getByTestId("ability-type-tabs");
      for (const kind of vocabulary.types) {
        const tab = page.getByTestId(`ability-type-tab-${kind.id}`);
        await expect(tab, `${system.id} should show a ${kind.id} tab`).toBeVisible();
        await expect(tab).toContainText(kind.pluralLabel);
      }
      const rendered = (await tabs.innerText()).trim();
      expect(rendered.length, "no tab may render blank (SC-013)").toBeGreaterThan(0);

      // FR-003: the umbrella term names the area itself, not just the tabs.
      await expect(
        page.getByRole("tab", { name: vocabulary.umbrella.pluralLabel }),
      ).toBeVisible();
    });
  }

  test("a system that declares no vocabulary still gets a complete built-in set", async ({
    page,
  }) => {
    // The case SC-013 measures, and the one most likely to be broken by a
    // change only ever exercised against a system that declares plenty.
    await registerGm(page);
    const worldId = await createWorld(page, `Vocab bare ${uniqueSuffix()}`);
    await chooseSystem(page, worldId, "Blades in the Dark / Forged in the Dark");

    const vocabulary = await vocabularyOf(page, worldId);
    expect(vocabulary.umbrella.pluralLabel).toBe("Abilities");
    // Named explicitly rather than looped over: an empty list would make the
    // loop below pass by doing nothing, which is how this test first went
    // green against a vocabulary that had no types at all.
    expect(vocabulary.types.map((kind) => kind.id).sort()).toEqual([
      "feat",
      "power",
      "spell",
      "talent",
    ]);

    await openAbilities(page, worldId);
    for (const kind of vocabulary.types) {
      await expect(page.getByTestId(`ability-type-tab-${kind.id}`)).toContainText(
        kind.pluralLabel,
      );
    }
  });

  test("a tab lists only its own type, counts what it lists, and creating in it needs no second answer", async ({
    page,
  }) => {
    await registerGm(page);
    const worldId = await createWorld(page, `Vocab tabs ${uniqueSuffix()}`);
    await chooseSystem(page, worldId, "5E System Core");

    const vocabulary = await vocabularyOf(page, worldId);
    const first = vocabulary.types[0];
    const second = vocabulary.types[1];
    expect(second, "this assertion needs a system with two types").toBeTruthy();

    await openAbilities(page, worldId);

    // FR-009: an empty tab says so rather than borrowing another tab's rows.
    await expect(page.getByTestId("ability-tab-empty")).toBeVisible();

    // FR-008: created from inside a tab, of that tab's type, without choosing.
    await page.getByTestId(`ability-type-tab-${first.id}`).click();
    const name = `Ember ${uniqueSuffix()}`;
    await page.getByTestId("new-ability-name-input").fill(name);
    await page.getByTestId("add-ability-button").click();

    const table = page.getByTestId("ability-catalog-table");
    await expect(table).toBeVisible({ timeout: 15_000 });
    await expect(table).toContainText(name);

    // FR-007: the count equals the rows the tab lists.
    await expect(page.getByTestId(`ability-type-count-${first.id}`)).toHaveText("1");

    // FR-009 again, from the other side: a sibling tab does not show it.
    await page.getByTestId(`ability-type-tab-${second.id}`).click();
    await expect(page.getByTestId("ability-tab-empty")).toBeVisible();
    await expect(page.getByTestId("ability-catalog-table")).toHaveCount(0);
  });
});

test.describe("US3: a system names its own ability types", () => {
  test("5e's Enchantment is offered in a 5e world and refused in another system's", async ({
    page,
  }) => {
    // SC-003's worked example. `enchantment` exists only in
    // `packs/systems/dnd5e/system.json` — no shared file mentions it, which
    // `scripts/check-ability-vocabulary.mjs` enforces separately.
    await registerGm(page);
    const worldId = await createWorld(page, `Enchant ${uniqueSuffix()}`);
    await chooseSystem(page, worldId, "5E System Core");

    const vocabulary = await vocabularyOf(page, worldId);
    expect(
      vocabulary.types.map((kind) => kind.id),
      "a pack's own type joins the world's vocabulary",
    ).toContain("enchantment");

    await openAbilities(page, worldId);
    await expect(
      page.getByTestId("ability-type-tab-enchantment"),
    ).toContainText("Enchantments");

    // FR-013: the same type is not authorable in a world running a system
    // that never declared it. Asked through the API, because that is where the
    // refusal has to hold.
    const other = await createWorld(page, `Enchant other ${uniqueSuffix()}`);
    await chooseSystem(page, other, "Genie");

    const refused = await page.request.post("/api/graphql", {
      headers: await csrfHeader(page),
      data: {
        query: `mutation C($input: CreateAbilityInput!) {
          createAbility(input: $input) { id }
        }`,
        variables: {
          input: {
            worldId: other,
            name: `Sneaky ${uniqueSuffix()}`,
            classification: "enchantment",
          },
        },
      },
    });
    const body = (await refused.json()) as { errors?: { message: string }[] };
    expect(
      body.errors?.length,
      "a type this world's system never declared must not be authorable here",
    ).toBeTruthy();
    expect(body.errors?.[0]?.message).toContain("does not recognise");
  });

  test("an ability of a pack's own type is created, listed and kept", async ({
    page,
  }) => {
    await registerGm(page);
    const worldId = await createWorld(page, `Enchant make ${uniqueSuffix()}`);
    await chooseSystem(page, worldId, "5E System Core");
    await openAbilities(page, worldId);

    await page.getByTestId("ability-type-tab-enchantment").click();
    const name = `Flametongue ${uniqueSuffix()}`;
    await page.getByTestId("new-ability-name-input").fill(name);
    await page.getByTestId("add-ability-button").click();

    // The type came from the tab, and the row is stored under a value the
    // dropped CHECK constraint would have refused (ADR-064).
    const table = page.getByTestId("ability-catalog-table");
    await expect(table).toBeVisible({ timeout: 15_000 });
    await expect(table).toContainText(name);
    await expect(
      page.getByTestId("ability-type-count-enchantment"),
    ).toHaveText("1");

    // And it is not in the Spells tab.
    await page.getByTestId("ability-type-tab-spell").click();
    await expect(page.getByTestId("ability-tab-empty")).toBeVisible();
  });
});
