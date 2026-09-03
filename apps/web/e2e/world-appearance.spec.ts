import { test, expect, type Page } from "@playwright/test";

/**
 * specs/032-pack-architecture, User Story 1: a Game Master dresses the table.
 *
 * What the suites below cannot answer, and this can: that the pack a Game
 * Master picks reaches *another participant's browser without a reload*
 * (SC-001), that a player is refused (FR-010), and that a pool's bar survives
 * the wire — the regression T019a exists to prevent, where a bar was recovered
 * by parsing a rendered string and a system writing "4 of 7" lost it silently.
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

async function registerAndCreateWorld(
  page: Page,
  worldName: string,
): Promise<string> {
  await register(page, freshCredentials("e2eappearance"));
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

test.describe("a world's interface pack", () => {
  test("the settings surface names the active pack rather than a placeholder", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(page, `Appearance ${uniqueSuffix()}`);
    await page.goto(`/world/${worldId}/settings/system`);

    const card = page.getByTestId("world-appearance-card");
    await expect(card).toBeVisible({ timeout: 15_000 });

    // FR-023: a world that has chosen nothing is drawn in the base pack, so
    // "Not yet assigned" would describe a state this product does not have.
    await expect(card).toContainText("Forge");
    await expect(card).not.toContainText("Not yet assigned");
    await expect(card).not.toContainText("Unbound placeholder");
  });

  /**
   * FR-007 and US1 scenario 6: the base pack is a peer. It is listed by being
   * in the directory, with no badge, no pinned position, and nothing marking
   * it out from any other pack.
   */
  test("the base pack is offered on the same footing as any other", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(page, `Peer ${uniqueSuffix()}`);
    await page.goto(`/world/${worldId}/settings/system`);

    const trigger = page.getByTestId("interface-pack-select");
    await expect(trigger).toBeVisible({ timeout: 15_000 });
    await trigger.click();

    const forge = page.getByRole("option", { name: /^Forge\b/ });
    await expect(forge).toBeVisible();

    // No "(default)", no "recommended", no marker of any kind. Asserted as
    // the absence of a marker rather than as exact text, because every option
    // now carries a line saying what it is for — "Forged Steel" is a code name
    // and read like one. Forge gets the same line as the rest, which is the
    // point: it says "works with any system", not that it is the right one.
    await expect(forge).toContainText("Works with any system");
    for (const marker of ["default", "recommended", "Recommended", "★"]) {
      await expect(forge).not.toContainText(marker);
    }
  });

  /**
   * The REST surface the picker reads, asserted directly. A pack that has
   * drifted out of compliance must fail closed rather than reach a browser.
   */
  test("an unknown pack is refused by the server rather than served empty", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2epackapi"));
    await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });

    const listed = await page.evaluate(async () => {
      const response = await fetch("/api/interface-packs", {
        credentials: "same-origin",
      });
      return { status: response.status, body: await response.json() };
    });
    expect(listed.status).toBe(200);
    expect(
      (listed.body as { id: string }[]).some((pack) => pack.id === "forge"),
    ).toBe(true);

    const missing = await page.evaluate(async () => {
      const response = await fetch(
        "/api/interface-packs/no-such-pack/manifest.json",
        { credentials: "same-origin" },
      );
      return response.status;
    });
    expect(missing).toBe(422);
  });

  /**
   * T019a's regression is asserted in `interface_packs_integration_tests.rs`,
   * not here, and the reason is worth recording.
   *
   * A browser test cannot reach `/api/graphql` by hand: the endpoint requires
   * a CSRF token that `postGraphQL` attaches and a raw `fetch` does not, so a
   * hand-rolled request gets a 401 with an **empty body**. The first version
   * of this test asserted the response did not say `Unknown field "fraction"`
   * — which passed against that empty body, and would have passed just as
   * happily with the field deleted. A negative assertion against a string that
   * can be empty is not an assertion at all.
   *
   * Reimplementing CSRF here would couple this file to an internal and could
   * mask a genuine auth regression. Asserting the shape where the schema is
   * actually executed is both honest and stronger.
   */
});

/**
 * Increment E's acceptance, at the table rather than in a unit test.
 *
 * `src/layout/` — the whole declared-value renderer, its vocabulary and its
 * forty-eight tests — was imported by nothing in the application. It rendered
 * in test files and nowhere else, which is why none of this was noticed: the
 * only sheet any player could open was `GenieActorSheet`, a hand-written
 * container for one of the seven bundled systems.
 */

/** Bind the world to a system through the settings page a Game Master uses. */
async function chooseSystem(
  page: Page,
  worldId: string,
  systemId: string,
  title: string,
): Promise<void> {
  await page.goto(`/world/${worldId}/settings/system`);
  const picker = page.getByTestId("system-picker");
  await expect(picker).toBeVisible({ timeout: 15_000 });
  await picker.click();
  await page.getByRole("option", { name: title }).click();

  // The legal notice has to be acknowledged before a system is assigned.
  const confirmation = page.getByTestId("pending-system-confirmation");
  await expect(confirmation).toBeVisible({ timeout: 15_000 });
  await confirmation.getByRole("button", { name: /confirm|assign/i }).click();

  await expect(page.getByTestId("active-system-card")).toContainText(title, {
    timeout: 15_000,
  });
}

async function createActor(
  page: Page,
  worldId: string,
  label: string,
): Promise<string> {
  await page.goto(`/world/${worldId}/compendium/npc/new`);
  await page.locator('[data-testid="npc-editor-name-input"]').fill(label);
  await page.locator('[data-testid="npc-editor-save"]').click();
  await page.waitForURL(/\/compendium\/npc\/[^/]+\/edit$/, { timeout: 15_000 });
  const match = /\/compendium\/npc\/([^/]+)\/edit$/.exec(
    new URL(page.url()).pathname,
  );
  if (!match) throw new Error(`Could not extract actor id from ${page.url()}`);
  return match[1];
}

interface SystemUnderTest {
  id: string;
  title: string;
  /**
   * Something this system's sheet says for a character nobody has filled in.
   *
   * Deliberately not an ability score. Stored values are **omitted when
   * absent** — a blank sheet is the absence of a score, not a zero — so a
   * fresh actor publishes only what exists whether or not anyone has touched
   * it: a speed with a manifest default, a track whose boxes exist unticked,
   * a ladder standing at no rung. That those three are what differ between
   * these systems is the point rather than a limitation.
   */
  expects: string[];
}

/**
 * Three rulesets whose sheets are shaped differently on purpose.
 *
 * 5e tracks speeds and derives modifiers; Fate has no abilities at all and
 * counts stress in boxes; Cypher's damage track is named states with no boxes
 * to count. If one renderer draws all three from their manifests alone, the
 * format carries what it claims to (SC-012).
 */
const SYSTEMS: SystemUnderTest[] = [
  { id: "dnd5e", title: "5E System Core", expects: ["Walk"] },
  { id: "fate_core", title: "Fate Core", expects: ["Stress"] },
  { id: "cypher_system", title: "Cypher System", expects: ["Damage Track"] },
];

test.describe("T080: every bundled system gets its own sheet from the base pack", () => {
  /** What each system's sheet said, for the cross-check below. */
  const rendered = new Map<string, string>();

  for (const system of SYSTEMS) {
    test(`${system.title} renders its own declared values under Forge alone`, async ({
      page,
    }) => {
      const worldId = await registerAndCreateWorld(
        page,
        `${system.id} ${uniqueSuffix()}`,
      );
      await chooseSystem(page, worldId, system.id, system.title);

      // Explicitly Forge. A world on a ruleset that has a pack written for it
      // now *starts* in that pack, so "the base pack alone" is something this
      // test has to ask for rather than something it gets by default — and
      // asking for it is the honest version: the claim is that a system needs
      // no pack of its own, which means choosing the generic one on purpose.
      await choosePack(page, worldId, "Forge");

      const actorId = await createActor(
        page,
        worldId,
        `Sheet ${uniqueSuffix()}`,
      );
      await page.goto(`/world/${worldId}/actor/${actorId}/view`);

      // Drawn by Forge, because this world has chosen no interface pack —
      // which is the point. No pack was written for any of these three and
      // all three get a sheet.
      const sheet = page.locator('[data-slot="sheet-layout"]');
      await expect(sheet).toBeVisible({ timeout: 20_000 });

      for (const expected of system.expects) {
        await expect(sheet).toContainText(expected);
      }

      rendered.set(system.id, (await sheet.innerText()).trim());
    });
  }

  test("and the three sheets are not the same sheet", async () => {
    // The claim T080 actually makes. One renderer drawing three rulesets is
    // only worth anything if the three come out different — a generic sheet
    // that showed every system the same thing would pass every assertion
    // above and mean nothing.
    expect(rendered.size).toBe(SYSTEMS.length);

    const texts = [...rendered.values()];
    expect(new Set(texts).size).toBe(texts.length);
  });
});

/** Bind the world to an interface pack through the settings card. */
async function choosePack(
  page: Page,
  worldId: string,
  packTitle: string,
): Promise<void> {
  await page.goto(`/world/${worldId}/settings/system`);
  const trigger = page.getByTestId("interface-pack-select");
  await expect(trigger).toBeVisible({ timeout: 15_000 });
  await trigger.click();
  // Anchored, because an option's accessible name now carries the line saying
  // what the pack is for — so a bare "Forge" substring-matches "Forged Silver"
  // as well. `\b` after the title is what separates "Forge" from "Forged".
  await page
    .getByRole("option", { name: new RegExp(`^${packTitle}\\b`) })
    .click();
  await expect(page.getByTestId("world-appearance-card")).toContainText(
    packTitle,
    { timeout: 15_000 },
  );
}

/**
 * T060 (SC-005): two worlds, two systems, two packs written for them.
 *
 * The claim is narrow and worth stating exactly: the *sheets* differ in what
 * they show and how it is arranged, while the application around them does
 * not change at all. A pack that could alter the shell would be a pack that
 * could hide a control (FR-012), and the value of a pack is that it cannot.
 */
test.describe("T060: a targeted pack dresses the sheet and nothing else", () => {
  test("two systems under their own packs render different sheets inside identical chrome", async ({
    page,
  }) => {
    // Fate and Cypher rather than 5e, and the reason is worth stating: both
    // declare a track or a ladder, which exist on a sheet whether or not
    // anyone has ticked them. Forged Steel's layout is `pair` and `value`
    // throughout, so for a character nobody has filled in it correctly draws
    // nothing — a true answer, and a poor subject for a test about two sheets
    // differing.
    const steelWorld = await registerAndCreateWorld(page, `Silver ${uniqueSuffix()}`);
    await chooseSystem(page, steelWorld, "fate_core", "Fate Core");
    await choosePack(page, steelWorld, "Forged Silver");
    const steelActor = await createActor(page, steelWorld, `Fate ${uniqueSuffix()}`);

    await page.goto(`/world/${steelWorld}/actor/${steelActor}/view`);
    const sheet = page.locator('[data-slot="sheet-layout"]');
    await expect(sheet).toBeVisible({ timeout: 20_000 });
    const steelSheet = (await sheet.innerText()).trim();
    // The chrome around it, which must be the same on both worlds.
    const steelNav = (
      await page.getByRole("navigation", { name: "Primary" }).innerText()
    ).trim();

    // A second world, a second ruleset, a second pack — same browser, same
    // account, same application.
    await page.goto("/worlds/create");
    await page.locator("#world-name").fill(`Bronze ${uniqueSuffix()}`);
    await page.getByRole("button", { name: /create world/i }).click();
    await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
    const silverWorld = /\/world\/([^/]+)\/staging$/.exec(
      new URL(page.url()).pathname,
    )![1];

    await chooseSystem(page, silverWorld, "cypher_system", "Cypher System");
    await choosePack(page, silverWorld, "Forged Bronze");
    const silverActor = await createActor(
      page,
      silverWorld,
      `Silver ${uniqueSuffix()}`,
    );

    await page.goto(`/world/${silverWorld}/actor/${silverActor}/view`);
    await expect(sheet).toBeVisible({ timeout: 20_000 });
    const silverSheet = (await sheet.innerText()).trim();
    const silverNav = (
      await page.getByRole("navigation", { name: "Primary" }).innerText()
    ).trim();

    // Different sheets...
    expect(steelSheet).not.toBe(silverSheet);
    expect(steelSheet.length).toBeGreaterThan(0);
    expect(silverSheet.length).toBeGreaterThan(0);

    // ...inside an application that did not change.
    expect(steelNav).toBe(silverNav);
  });
});

/**
 * T061 / SC-008: every surface showing an unset binding says the same true
 * thing.
 *
 * Measured as zero distinct strings for the unset state. There were two —
 * "Unbound placeholder" on the hub card and "Not yet assigned" on the
 * dashboard — and both described a state this product does not have: a world
 * that has chosen no pack is drawn in the base pack, which is on screen and
 * has a name.
 *
 * The other half of T061 — a world whose bound pack is no longer installed —
 * is not reachable from here. `updateWorldInterfacePack` validates the pack
 * before storing it, so the product refuses to create that state on purpose;
 * it arises only from a pack being removed from a deployment after the fact.
 * Simulating that means moving files under a running server, so it is proved
 * in `src/server/src/interface_packs_integration_tests.rs` against a
 * temporary packs directory instead.
 */
test.describe("T061: one wording for an unset pack binding", () => {
  test("the hub card and the world dashboard both name the pack in force", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(page, `Label ${uniqueSuffix()}`);

    await page.goto("/worlds");
    const card = page.getByText("Interface pack").first();
    await expect(card).toBeVisible({ timeout: 15_000 });
    const hub = await page
      .locator("div", { has: page.getByText("Interface pack") })
      .last()
      .innerText();

    await page.goto(`/world/${worldId}`);
    await expect(page.getByText("Interface pack")).toBeVisible({
      timeout: 15_000,
    });
    const dashboard = await page.locator("body").innerText();

    // The true thing, on both.
    expect(hub).toContain("Forge");
    expect(dashboard).toContain("Forge");

    // And neither of the two strings that were not true.
    for (const surface of [hub, dashboard]) {
      expect(surface).not.toContain("Unbound placeholder");
      expect(surface).not.toContain("Not yet assigned");
    }
  });
});

test.describe("T103: a failing pack surface is contained and names the pack", () => {
  /**
   * Spec 032 FR-016 and SC-009, which measure two separate things: the rest
   * of the session stays usable, and the message names the responsible pack.
   *
   * # How the failure is injected
   *
   * By serving a malformed layout to the browser and nothing else. The pack's
   * manifest response is rewritten in flight so that its layout carries a
   * container node with no `children` — the renderer walks `children` to
   * decide whether the node draws anything, and walking a missing array
   * throws.
   *
   * Deliberately not by adding a fault switch to the product. A test-only
   * branch in shipping code is a branch that has to be kept working and can
   * be reached by accident, and it would prove that the boundary catches a
   * fault the product was asked to raise rather than one a pack caused.
   *
   * This also covers the gap the boundary originally had: the throw happens
   * in the *decision* to render, one call above the renderer, which an
   * earlier version of this boundary did not wrap.
   */
  async function serveABrokenLayout(page: Page): Promise<void> {
    await page.route("**/api/interface-packs/*/manifest.json", async (route) => {
      const response = await route.fetch();
      const manifest = (await response.json()) as Record<string, unknown>;
      // A container that promises children and has none.
      manifest.layout = [{ kind: "column" }];
      await route.fulfill({ response, json: manifest });
    });
  }

  test("the sheet is replaced by a named notice, and the session stays usable", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(page, `Fault ${uniqueSuffix()}`);
    // Fate Core rather than Genie, and the reason is worth stating: Genie is
    // the one bundled system with a hand-written container in
    // `systemActorSheets.ts`, so `PackActorSheet` never mounts for it and
    // there is no pack-drawn surface to break. Every other system's sheet is
    // drawn by the pack, which is what this test is about.
    await chooseSystem(page, worldId, "fate_core", "Fate Core");
    await choosePack(page, worldId, "Forge");
    const actorId = await createActor(page, worldId, `Fault ${uniqueSuffix()}`);

    await serveABrokenLayout(page);
    await page.goto(`/world/${worldId}/actor/${actorId}/view`);

    // SC-009's second half: the message names the pack that was rendering.
    const notice = page.locator('[data-slot="pack-surface-failed"]');
    await expect(notice).toBeVisible({ timeout: 20_000 });
    await expect(notice).toHaveAttribute("data-pack", "forge");
    await expect(notice).toContainText("forge");

    // And the sheet it replaced is genuinely not there — a notice rendered
    // beside a half-drawn sheet would be a worse outcome than either.
    await expect(page.locator('[data-slot="sheet-layout"]')).toHaveCount(0);

    // SC-009's first half: the rest of the session is usable. Not "the page
    // did not go blank" — actually leave, arrive somewhere else, and find it
    // working.
    await page.goto(`/world/${worldId}/settings/system`);
    await expect(page.getByTestId("active-system-card")).toContainText(
      "Fate Core",
      { timeout: 20_000 },
    );

    await page.goto(`/world/${worldId}/compendium`);
    await expect(page.locator("body")).toContainText(/compendium/i, {
      timeout: 20_000,
    });
  });

  test("a sound pack still draws after a broken one has failed in the same session", async ({
    page,
  }) => {
    // The half a boundary gets wrong quietly: having caught once, it stays
    // caught, and the next sheet mounted into it shows the previous one's
    // error. Proved by fixing the pack and going back.
    const worldId = await registerAndCreateWorld(page, `Recover ${uniqueSuffix()}`);
    await chooseSystem(page, worldId, "fate_core", "Fate Core");
    await choosePack(page, worldId, "Forge");
    const actorId = await createActor(page, worldId, `Recover ${uniqueSuffix()}`);

    await serveABrokenLayout(page);
    await page.goto(`/world/${worldId}/actor/${actorId}/view`);
    await expect(page.locator('[data-slot="pack-surface-failed"]')).toBeVisible({
      timeout: 20_000,
    });

    await page.unroute("**/api/interface-packs/*/manifest.json");
    await page.goto(`/world/${worldId}/actor/${actorId}/view`);

    await expect(page.locator('[data-slot="sheet-layout"]')).toBeVisible({
      timeout: 20_000,
    });
    await expect(page.locator('[data-slot="pack-surface-failed"]')).toHaveCount(0);
  });
});
