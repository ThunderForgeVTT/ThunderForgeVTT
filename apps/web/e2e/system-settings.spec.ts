import { test, expect, type Page } from "./fixtures/test";
import { expectNoAxeViolations } from "./fixtures/axe";

/**
 * specs/016-system-pack-legal-compliance: the persistent world System
 * Settings surface (`/world/:id/settings/system`) where a GM assigns a
 * game system and everyone can see its legal/attribution notice. Backend
 * legal-manifest enforcement already has resolver-level coverage
 * (systems::manifest_legal_enforcement_tests); this is the missing
 * browser-level check that a GM can actually pick a system through the
 * real UI, see the legal notice before confirming, have it persist, and
 * that a non-GM sees the same info with no picker.
 *
 * A Player reads it on the world's Overview, under "About this table": the
 * settings page shows a Player a GIF and a way back, not the settings.
 */

/** Tenor's GIF, answered locally so the test never waits on Tenor. */
const GM_SIDE_GIF = "https://media.tenor.com/ejjuR2cYxvoAAAAM/wait-nahhh.gif";
const ONE_PIXEL_GIF = Buffer.from(
  "R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7",
  "base64",
);

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
  await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
    timeout: 15_000,
  });
}

async function createWorld(page: Page, worldName: string): Promise<string> {
  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const match = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname);
  if (!match)
    throw new Error(`Could not extract world id from URL: ${page.url()}`);
  return match[1];
}

test.describe("Spec 016: GM assigns a game system and its legal notice is persistently visible", () => {
  test("GM sees Genie already assigned, every bundled system offered, and re-confirming Genie's legal notice persists across a revisit", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2esystem"));
    const worldId = await createWorld(
      page,
      `E2E System Settings ${uniqueSuffix()}`,
    );

    // Spec 021: world creation now sends a game-system picker default
    // (Genie), so a freshly-created world already has one assigned rather
    // than showing "no system assigned yet".
    await page.goto(`/world/${worldId}/settings/system`);
    await expect(page.getByTestId("active-system-card")).toContainText("Genie");

    // The owner's complaint, as an assertion: "if you are Genie, it should
    // show Genie on the select dropdown". The closed trigger used to be
    // blank — its `value` was the *pending* choice, undefined until you
    // touched it — so a world with a system told its GM to select one.
    await expect(page.getByTestId("system-picker")).toContainText("Genie");
    await expect(page.getByTestId("system-picker")).not.toContainText(
      "Select a system",
    );

    await page.getByTestId("system-picker").click();

    // Spec 021 marked every pack but Genie "(TBD)" and disabled it, because
    // Genie was the only system with a real, data-connected actor sheet.
    // Spec 032 removed that condition rather than the caution: a sheet is
    // what a system declares, laid out by the world's interface pack, so a
    // bundled pack works in play by having a manifest — and all seven do.
    // `world-appearance.spec.ts` opens three of them and reads their sheets.
    const dnd5eOption = page.getByRole("option", { name: "5E System Core" });
    await expect(dnd5eOption).toBeVisible();
    await expect(dnd5eOption).not.toContainText("(TBD)");
    await expect(dnd5eOption).not.toHaveAttribute("aria-disabled", "true");

    // A *different* system, because the trigger now carries the world's own
    // system as its value: a select cannot change to what it already is, so
    // re-picking Genie is no longer a change and raises no confirmation.
    // Picking 5E is the better exercise of FR-004's "point of selection"
    // review step anyway — it is the thing a GM would actually do — and the
    // world is empty, so it takes the one-step path (FR-029).
    await dnd5eOption.click();
    await expect(page.getByTestId("pending-system-confirmation")).toBeVisible({
      timeout: 10_000,
    });
    const pendingLegalText = await page
      .getByTestId("pending-system-confirmation")
      .innerText();
    expect(pendingLegalText.length).toBeGreaterThan(0);

    await page.getByRole("button", { name: "Confirm" }).click();
    await expect(page.getByText("System assigned.")).toBeVisible({
      timeout: 10_000,
    });
    await expect(page.getByTestId("active-system-card")).toContainText(
      "5E System Core",
    );

    // Persists across a fresh navigation, not just optimistic local state —
    // and the closed picker says so too, which is the whole of the fix.
    await page.goto(`/world/${worldId}/settings/system`);
    await expect(page.getByTestId("active-system-card")).toContainText(
      "5E System Core",
      { timeout: 10_000 },
    );
    await expect(page.getByTestId("system-picker")).toContainText(
      "5E System Core",
      { timeout: 10_000 },
    );
    await expect(page.getByTestId("active-system-card")).not.toContainText(
      /no system assigned yet/i,
    );
  });

  test("a non-GM member reads the active system and its legal notice on the Overview, and no picker anywhere", async ({
    browser,
  }) => {
    const gmContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const gmPage = await gmContext.newPage();
    await register(gmPage, freshCredentials("e2esystemgm"));
    // Spec 021: world creation already assigns Genie by default, so there's
    // no separate "GM assigns a system" step needed before checking that a
    // non-GM member sees the same info with no picker.
    const worldId = await createWorld(
      gmPage,
      `E2E System Settings Viewer ${uniqueSuffix()}`,
    );

    await gmPage.goto(`/world/${worldId}/players`);
    await gmPage.getByRole("button", { name: "Generate Join Link" }).click();
    const inviteInput = gmPage.locator("input[readonly]").first();
    await expect(inviteInput).toBeVisible({ timeout: 10_000 });
    const inviteUrl = await inviteInput.inputValue();
    const inviteCode = new URL(inviteUrl).pathname.split("/").pop();
    if (!inviteCode) throw new Error("Could not extract invite code");

    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    await register(playerPage, freshCredentials("e2esystemplayer"));
    await playerPage.goto(`/join/${inviteCode}`);
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();
    await playerPage.waitForURL(
      (url) => url.pathname.startsWith(`/world/${worldId}`),
      {
        timeout: 15_000,
      },
    );

    await playerPage.goto(`/world/${worldId}/staging`);
    const about = playerPage.getByTestId("about-this-table");
    await expect(about.getByTestId("active-system-card")).toContainText(
      "Genie",
      {
        timeout: 10_000,
      },
    );
    await expect(about.getByTestId("system-legal-notice")).toBeVisible();
    // No GM-only picker for a non-GM member, here or on the settings page.
    await expect(playerPage.getByTestId("system-picker-card")).toHaveCount(0);
    await playerPage.goto(`/world/${worldId}/settings/system`);
    await expect(
      playerPage.getByTestId("settings-not-for-players"),
    ).toBeVisible({ timeout: 10_000 });
    await expect(playerPage.getByTestId("system-picker-card")).toHaveCount(0);

    await gmContext.close();
    await playerContext.close();
  });

  test("a Player who types in the settings address is shown the GIF, no settings, and a way back", async ({
    browser,
  }) => {
    const gmContext = await browser.newContext();
    const gmPage = await gmContext.newPage();
    await register(gmPage, freshCredentials("e2esysgifgm"));
    const worldId = await createWorld(
      gmPage,
      `E2E System Settings GIF ${uniqueSuffix()}`,
    );

    await gmPage.goto(`/world/${worldId}/players`);
    await gmPage.getByRole("button", { name: "Generate Join Link" }).click();
    const inviteInput = gmPage.locator("input[readonly]").first();
    await expect(inviteInput).toBeVisible({ timeout: 10_000 });
    const inviteCode = new URL(await inviteInput.inputValue()).pathname
      .split("/")
      .pop();
    if (!inviteCode) throw new Error("Could not extract invite code");

    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    await register(playerPage, freshCredentials("e2esysgifplayer"));
    await playerPage.goto(`/join/${inviteCode}`);
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();
    await playerPage.waitForURL(
      (url) => url.pathname.startsWith(`/world/${worldId}`),
      { timeout: 15_000 },
    );

    await test.step("the GIF and the line, and none of the settings", async () => {
      await playerContext.route(GM_SIDE_GIF, (route) =>
        route.fulfill({ contentType: "image/gif", body: ONE_PIXEL_GIF }),
      );
      await playerPage.goto(`/world/${worldId}/settings/system`);
      const notice = playerPage.getByTestId("settings-not-for-players");
      await expect(notice).toBeVisible({ timeout: 15_000 });
      await expect(
        notice.getByRole("heading", {
          name: "Nice try — this is the Game Master's side of the table.",
        }),
      ).toBeVisible();
      const gif = notice.getByTestId("settings-not-for-players-gif");
      await expect(gif).toHaveAttribute("src", GM_SIDE_GIF);
      await expect(gif).toHaveAttribute("alt", /\S/);
      await expect(
        playerPage.getByTestId("world-system-settings-page"),
      ).toHaveCount(0);
      for (const card of [
        "active-system-card",
        "system-picker-card",
        "world-appearance-card",
        "default-scene-grid-card",
        "settings-invite-players-card",
        "world-system-settings-card",
      ]) {
        await expect(playerPage.getByTestId(card)).toHaveCount(0);
      }
      await expectNoAxeViolations(
        playerPage,
        '[data-testid="settings-not-for-players"]',
      );
    });

    await test.step("the line stands alone when the GIF cannot load", async () => {
      await playerContext.unroute(GM_SIDE_GIF);
      await playerContext.route(GM_SIDE_GIF, (route) => route.abort());
      await playerPage.reload();
      const notice = playerPage.getByTestId("settings-not-for-players");
      await expect(notice).toContainText("Game Master's side of the table", {
        timeout: 15_000,
      });
      await expect(
        notice.getByTestId("settings-not-for-players-gif"),
      ).toHaveCount(0);
    });

    await test.step("the button goes back to the world", async () => {
      await playerPage.getByTestId("settings-back-to-world").click();
      await expect(playerPage).toHaveURL(
        new RegExp(`/world/${worldId}/staging$`),
      );
      await expect(playerPage.getByTestId("about-this-table")).toBeVisible({
        timeout: 15_000,
      });
    });

    await test.step("the Game Master still gets the settings", async () => {
      await gmPage.goto(`/world/${worldId}/settings/system`);
      await expect(gmPage.getByTestId("system-picker-card")).toBeVisible({
        timeout: 15_000,
      });
      await expect(gmPage.getByTestId("settings-not-for-players")).toHaveCount(
        0,
      );
    });

    await gmContext.close();
    await playerContext.close();
  });
});

test.describe("Spec 022 User Story 4: named settings groups + Default Scene Grid Type control", () => {
  /**
   * Spec 022 FR-017 asked for one card headed "System Settings", because the
   * page was one card and that card was really the world's settings home.
   * The page is now grouped — "Game system", "At the table", "Content and
   * contributors" — on the owner's instruction that it read as sections
   * rather than one run of cards, so the settings home is the page and the
   * card is only about the system. The labelled controls FR-017 named are
   * unchanged and still asserted here.
   */
  test("the settings are grouped under named headings, the picker is labeled 'Change System', and Default Scene Grid Type offers Gridless/Squares/Hexagons", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2esystemlabel"));
    const worldId = await createWorld(
      page,
      `E2E System Settings Labels ${uniqueSuffix()}`,
    );
    await page.goto(`/world/${worldId}/settings/system`);

    await expect(
      page.getByRole("heading", { name: "Game system", exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "At the table", exact: true }),
    ).toBeVisible();
    await expect(
      page
        .getByTestId("system-picker-card")
        .getByRole("heading", { name: "Change the system" }),
    ).toBeVisible();
    await expect(page.getByLabel("Change System")).toBeVisible();
    await expect(page.getByLabel("Default Scene Grid Type")).toBeVisible();

    // "None" read as *nothing chosen* — the empty state of a picker rather
    // than a choice inside it — and hid a real mode with its own rules. The
    // engine has called it Gridless all along.
    const gridPicker = page.getByTestId("default-scene-grid-type-picker");
    await expect(gridPicker).toContainText("Squares");
    await gridPicker.click();
    await expect(page.getByRole("option", { name: "Gridless" })).toBeVisible();
    await expect(page.getByRole("option", { name: "Squares" })).toBeVisible();
    await expect(page.getByRole("option", { name: "Hexagons" })).toBeVisible();
    await expect(page.getByRole("option", { name: "None" })).toHaveCount(0);

    // The stored value is what the closed trigger reads, here as much as on
    // the system picker beside it.
    await page.getByRole("option", { name: "Gridless" }).click();
    await expect(gridPicker).toContainText("Gridless");
    await page.goto(`/world/${worldId}/settings/system`);
    await expect(
      page.getByTestId("default-scene-grid-type-picker"),
    ).toContainText("Gridless", { timeout: 10_000 });
  });
});

test.describe("System settings is readable on a desktop and on a phone", () => {
  test("no axe violations and no sideways page scroll at 1440px or 375px", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2esystema11y"));
    const worldId = await createWorld(
      page,
      `E2E System Settings Layout ${uniqueSuffix()}`,
    );

    for (const width of [1440, 375]) {
      await page.setViewportSize({ width, height: 900 });
      await page.goto(`/world/${worldId}/settings/system`);
      await expect(page.getByTestId("system-picker-card")).toBeVisible({
        timeout: 20_000,
      });

      await expectNoAxeViolations(
        page,
        "[data-testid=world-system-settings-page]",
      );

      // The page itself must not scroll sideways. A card may scroll inside
      // its own container; the document may not. At 375px the world rail
      // used to sit beside the content and leave it about 95px, which is
      // what this assertion was failing on before the rail became a strip.
      const overflows = await page.evaluate(
        () =>
          document.documentElement.scrollWidth >
          document.documentElement.clientWidth + 1,
      );
      expect(overflows, `page scrolls sideways at ${width}px`).toBe(false);
    }

    await page.setViewportSize({ width: 1280, height: 900 });
  });
});
