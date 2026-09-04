import { test, expect, type Page } from "@playwright/test";

/**
 * specs/033-abilities-vocabulary — the experience, rather than the
 * requirements.
 *
 * `abilities-vocabulary.spec.ts` and `system-change-guard.spec.ts` check the
 * spec clause by clause, which is what they are for. This file walks the path
 * a Game Master actually takes and asserts the things a person would notice:
 * that the words are theirs, that a count matches what is on screen, that a
 * player sees the same shelf as the GM, that nothing is a dead end, and that
 * the warning before a system change reads like the truth.
 *
 * The value is in the joins. Every assertion below passed its own unit test
 * somewhere; what this catches is two correct pieces meeting badly — a tab
 * that counts one thing and lists another, a picker offering a type the tab
 * set does not show, an empty state with no way out of it.
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

async function chooseSystem(page: Page, worldId: string, title: string) {
  await page.goto(`/world/${worldId}/settings/system`);
  const picker = page.getByTestId("system-picker");
  await expect(picker).toBeVisible({ timeout: 15_000 });
  await picker.click();
  await page.getByRole("option", { name: title }).click();
  const warning = page.getByTestId("system-change-warning");
  if (await warning.isVisible().catch(() => false)) {
    await page.getByTestId("system-change-accept-risk").click();
  }
  await page
    .getByTestId("pending-system-confirmation")
    .getByRole("button", { name: /confirm/i })
    .click();
  await expect(page.getByTestId("active-system-card")).toContainText(title, {
    timeout: 15_000,
  });
}

async function openAbilities(page: Page, worldId: string) {
  const tabs = page.getByTestId("ability-type-tabs");
  await page.goto(`/world/${worldId}/compendium?tab=abilities`);
  try {
    await expect(tabs).toBeVisible({ timeout: 12_000 });
  } catch {
    await page.goto(`/world/${worldId}/compendium?tab=abilities`);
    await expect(tabs).toBeVisible({ timeout: 20_000 });
  }
}

async function addAbility(page: Page, name: string) {
  await page.getByTestId("new-ability-name-input").fill(name);
  await page.getByTestId("add-ability-button").click();
  await expect(page.getByTestId("ability-catalog-table")).toContainText(name, {
    timeout: 15_000,
  });
}

test.describe("A Game Master finds their rulebook's sections", () => {
  // Each of these registers one or two fresh accounts and creates a world,
  // every one a full page load against the dev server. That legitimately
  // exceeds Playwright's 30s default — slow, not hung — and hitting it
  // produces "Test ended" failures that read like product bugs.
  test.describe.configure({ timeout: 120_000 });

  test("the whole path reads in the system's words, and every number matches the screen", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eux"));
    const worldId = await createWorld(page, `UX ${uniqueSuffix()}`);
    await chooseSystem(page, worldId, "Genie");
    await openAbilities(page, worldId);

    // The complaint this feature started from: a flat list with a Type column.
    // What a GM should meet instead is their own vocabulary.
    await expect(page.getByRole("tab", { name: "Scrolls" })).toBeVisible();
    await expect(page.getByRole("tab", { name: "Knacks" })).toBeVisible();
    await expect(page.getByRole("tab", { name: "Spells" })).toHaveCount(0);

    // An empty section says so and offers a way out of itself. An empty state
    // with no next step is where a GM gets stuck.
    await expect(page.getByTestId("ability-tab-empty")).toBeVisible();
    await expect(page.getByTestId("new-ability-name-input")).toBeVisible();

    // Two Scrolls and one Knack, created from inside their own tabs.
    await page.getByTestId("ability-type-tab-spell").click();
    await addAbility(page, `Ember ${uniqueSuffix()}`);
    await addAbility(page, `Gale ${uniqueSuffix()}`);
    await page.getByTestId("ability-type-tab-talent").click();
    await addAbility(page, `Nimble ${uniqueSuffix()}`);

    // The join worth testing: a count is a promise about what the tab lists.
    for (const [id, expected] of [
      ["spell", 2],
      ["talent", 1],
    ] as const) {
      await page.getByTestId(`ability-type-tab-${id}`).click();
      await expect(page.getByTestId(`ability-type-count-${id}`)).toHaveText(
        String(expected),
      );
      await expect(
        page.getByTestId("ability-catalog-table").locator("tbody tr"),
      ).toHaveCount(expected);
    }

    // Nothing anywhere on the ability surface calls them "abilities" where the
    // system supplied a word — SC-002's measurable version of FR-006.
    await page.getByTestId("ability-type-tab-spell").click();
    const tabs = await page.getByTestId("ability-type-tabs").innerText();
    expect(tabs.toLowerCase()).not.toContain("spell");
    expect(tabs).toContain("Scrolls");
  });

  test("a player sees the same shelf as the Game Master, with the same words", async ({
    browser,
  }) => {
    // FR-010. The tab set is not a GM tool: a player browsing the compendium
    // should meet the same sections under the same names, or the two are
    // looking at different products. Only the abilities *within* them differ,
    // which the GM-only filtering already handles.
    const gmContext = await browser.newContext();
    const gm = await gmContext.newPage();
    await register(gm, freshCredentials("e2euxgm"));
    const worldId = await createWorld(gm, `UX shared ${uniqueSuffix()}`);
    await chooseSystem(gm, worldId, "Genie");
    await openAbilities(gm, worldId);
    await gm.getByTestId("ability-type-tab-spell").click();
    const shared = `Shared ${uniqueSuffix()}`;
    await addAbility(gm, shared);

    // A real invite, so the player is a real member.
    await gm.goto(`/world/${worldId}`);
    await expect(
      gm.getByRole("heading", { name: /campaign settings/i }),
    ).toBeVisible({ timeout: 15_000 });
    await gm.getByRole("button", { name: /generate join link/i }).click();
    await expect(gm.getByTestId("invite-link-row").first()).toBeVisible({
      timeout: 15_000,
    });
    const url = await gm
      .getByTestId("invite-link-row")
      .first()
      .getByLabel("Invite link")
      .inputValue();
    const code = url.split("/join/")[1];
    expect(code, `could not parse a code out of ${url}`).toBeTruthy();

    const playerContext = await browser.newContext();
    const player = await playerContext.newPage();
    await register(player, freshCredentials("e2euxpl"));
    await player.goto(`/join/${code}`);
    // The join is a deliberate act, not a redirect — the invite page asks
    // before adding anyone to a world.
    await player.getByRole("button", { name: /join campaign/i }).click();
    await player.waitForURL(new RegExp(`/world/${worldId}`), {
      timeout: 20_000,
    });

    await openAbilities(player, worldId);

    // The same shelf, in the same words.
    await expect(player.getByRole("tab", { name: "Scrolls" })).toBeVisible();
    await expect(player.getByRole("tab", { name: "Knacks" })).toBeVisible();
    await expect(player.getByRole("tab", { name: "Spells" })).toHaveCount(0);

    // And the ability itself, since it is not GM-only.
    await player.getByTestId("ability-type-tab-spell").click();
    await expect(player.getByTestId("ability-catalog-table")).toContainText(
      shared,
    );

    // A player is offered no creation control, on any tab.
    await expect(player.getByTestId("add-ability-button")).toHaveCount(0);

    await gmContext.close();
    await playerContext.close();
  });

  test("a Game Master changing system is told the truth, and can back out of it", async ({
    page,
  }) => {
    // The moment this feature is most likely to lose someone's trust: a
    // destructive-looking change to a world they have put work into.
    await register(page, freshCredentials("e2euxsw"));
    const worldId = await createWorld(page, `UX switch ${uniqueSuffix()}`);
    await chooseSystem(page, worldId, "Genie");
    await openAbilities(page, worldId);
    await addAbility(page, `Keepsake ${uniqueSuffix()}`);

    await page.goto(`/world/${worldId}/settings/system`);
    await page.getByTestId("system-picker").click();
    await page.getByRole("option", { name: "5E System Core" }).click();

    const warning = page.getByTestId("system-change-warning");
    await expect(warning).toBeVisible({ timeout: 15_000 });

    // It names the system by the word a person picked, not by an id.
    await expect(warning).toContainText("5E System Core");
    await expect(warning).not.toContainText("dnd5e");

    // It is specific: a count a GM can check against their own compendium.
    await expect(page.getByTestId("system-change-counts")).toContainText("1 ability");

    // And it can be backed out of, which is the difference between a warning
    // and a trap.
    await page.getByTestId("system-change-cancel").click();
    await expect(warning).toHaveCount(0);
    await expect(page.getByTestId("active-system-card")).toContainText("Genie");

    // The ability is exactly where it was.
    await openAbilities(page, worldId);
    await page.getByTestId("ability-type-tab-spell").click();
    await expect(page.getByTestId("ability-type-count-spell")).toHaveText("1");
  });
});
