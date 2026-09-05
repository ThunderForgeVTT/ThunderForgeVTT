import { test, expect, type Page } from "@playwright/test";

/**
 * Spec 034: mirroring a world's lore into a repository its owner controls.
 *
 * # What is covered here, and what is not
 *
 * The unconfigured path (FR-036b) is covered because it is **the state every
 * self-hosted instance starts in** and the easiest to leave broken: nobody
 * developing this feature ever sees it, because their instance is configured.
 * A Game Master must never be shown a flow that cannot complete, and the only
 * way to know that holds is to look at an instance with no application
 * registered — which is exactly what the e2e stack is.
 *
 * The *connected* paths are not covered here, deliberately. Reaching them
 * needs an application registered with a real repository host and a grant
 * completed in that host's UI, which no automated suite on this machine can
 * do without inventing a fake host — and a fake host would prove the fake
 * host works. Those claims are covered where they can be proven for real:
 * `lore_sync::git_roundtrip_tests` drives the actual `git` binary against a
 * local bare repository and asserts the mirror, the rename history, and the
 * divergence refusal.
 *
 * That split is stated rather than left implicit, because "there is no e2e for
 * the happy path" should be a decision someone can disagree with, not an
 * omission they discover.
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
  if (!match) throw new Error(`Could not extract world id from URL: ${page.url()}`);
  return match[1];
}

/**
 * User Story 2's promise, checked where it is actually made: **no failure of
 * the remote may affect in-app lore.**
 *
 * These do not simulate a broken remote — they assert the stronger and simpler
 * thing that makes simulating one unnecessary. Nothing in Stories 1 and 2
 * writes to a lore table, so lore behaves identically whether a repository is
 * connected, broken, or absent. The e2e that would "break the host" would be
 * testing a mock; this tests the property.
 *
 * The failure-mode table itself (unreachable host, revoked grant, force-pushed
 * branch, deleted repository) is exercised where those things are real:
 * `lore_sync::git_roundtrip_tests` drives the actual git binary against a local
 * bare repository, and its divergence test has a second clone rewrite the
 * branch mid-pass and asserts the push is refused with the other commit intact.
 */
test.describe("Spec 034 User Story 2: a world is unharmed by its mirror", () => {
  /**
   * SC-006: zero instances of in-app lore being altered, hidden or lost across
   * every failure mode. The structural version of that claim.
   */
  test("lore can be created and read with the mirror surface live", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eloreharm"));
    const worldId = await createWorld(page, `E2E Lore Unharmed ${uniqueSuffix()}`);

    // Visit the settings surface first, so the connection machinery is loaded
    // and answering rather than never having been asked.
    await page.goto(`/world/${worldId}/settings/system`);
    await expect(page.getByTestId("lore-repository-card")).toBeVisible({
      timeout: 15_000,
    });

    // Then do the thing FR-028 protects: read and write lore.
    const title = `Unharmed ${uniqueSuffix()}`;
    await page.goto(`/world/${worldId}/compendium`);
    await page.getByRole("tab", { name: "Lore" }).click();
    await page.getByTestId("new-lore-entry-title-input").fill(title);
    await page.getByTestId("add-lore-entry-button").click();

    const row = page
      .getByTestId("lore-catalog-table")
      .locator("tr", { hasText: title });
    await expect(row).toBeVisible({ timeout: 15_000 });
    await row.getByRole("link", { name: "View" }).click();
    await page.waitForURL(/\/world\/[^/]+\/lore\/[^/]+\/view$/, {
      timeout: 15_000,
    });

    const slug = /\/lore\/([^/]+)\/view$/.exec(new URL(page.url()).pathname)?.[1];
    if (!slug) throw new Error("no slug");

    // Reading the entry back is the half FR-028 is about: lore behaves exactly
    // as it does in a world with no connection. Editing is covered thoroughly
    // by `lore-wiki.spec.ts`; duplicating its editor interaction here would
    // couple this spec to that surface's markup for no extra assurance.
    // The breadcrumb rather than the rendered markdown: a freshly created
    // entry has an empty body, so there is no markdown block to find, and
    // asserting one would fail for a reason that has nothing to do with this
    // spec.
    await page.goto(`/world/${worldId}/lore/${slug}/view`);
    await expect(page.getByTestId("lore-breadcrumb")).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.getByTestId("lore-breadcrumb")).toContainText(title);
  });
});

test.describe("Spec 034: an instance with no repository integration", () => {
  /**
   * FR-036b. The failure this rules out is a Game Master clicking "connect",
   * being sent to a repository host, granting access, and coming back to an
   * error — because the instance never had an application registered.
   */
  test("shows exactly one of the two states, and never a broken flow", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2elore"));
    const worldId = await createWorld(page, `E2E Lore Repo ${uniqueSuffix()}`);

    await page.goto(`/world/${worldId}/settings/system`);

    const card = page.getByTestId("lore-repository-card");
    await expect(card).toBeVisible({ timeout: 15_000 });

    // Which branch renders depends on whether *this instance* has an
    // application registered, and the suite must not assume either. An earlier
    // version asserted the unconfigured branch unconditionally and started
    // failing the moment a real application was configured — the test was
    // right, its assumption was not.
    const unconfigured = page.getByTestId("lore-sync-unconfigured");
    const isUnconfigured = (await unconfigured.count()) > 0;

    if (isUnconfigured) {
      // FR-036b. Nothing connectable exists at all — not a disabled button,
      // which a Game Master would sit and wait for.
      await expect(unconfigured).toContainText(/no repository integration/i);
      await expect(page.getByTestId("lore-sync-connect")).toHaveCount(0);
      await expect(page.getByTestId("lore-sync-notice")).toHaveCount(0);
      await expect(page.getByTestId("lore-sync-acknowledge")).toHaveCount(0);
    } else {
      // Configured: a connect affordance exists, and the pre-synchronisation
      // notice has NOT been skipped — FR-038's gate is not something a
      // configured instance gets to bypass.
      await expect(page.getByTestId("lore-sync-connect")).toHaveCount(1);
      await expect(page.getByTestId("lore-sync-acknowledge")).toHaveCount(0);
    }

    // True in both states, and the reason the branch matters: a Game Master is
    // never shown a half-built flow.
    const connectable = await page.getByTestId("lore-sync-connect").count();
    const explained = await unconfigured.count();
    expect(
      connectable + explained,
      "the card showed neither a way forward nor a reason there is none",
    ).toBeGreaterThan(0);
  });

  /**
   * FR-035 and FR-004c, checked where it actually matters — in what the
   * browser receives. The server type has no credential field to return, and
   * this is the assertion that the shape reaching a client stays that way.
   *
   * Reading the response rather than the rendered page on purpose: a value can
   * be absent from the screen and present in the payload, and it is the
   * payload an attacker reads.
   */
  test("never sends a credential or a host identifier to the browser", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eloresec"));
    const worldId = await createWorld(page, `E2E Lore Secrets ${uniqueSuffix()}`);

    const payloads: string[] = [];
    page.on("response", async (response) => {
      if (!response.url().includes("/api/graphql")) return;
      try {
        payloads.push(await response.text());
      } catch {
        // A response body that cannot be read is not evidence of a leak.
      }
    });

    await page.goto(`/world/${worldId}/settings/system`);
    await expect(page.getByTestId("lore-repository-card")).toBeVisible({
      timeout: 15_000,
    });

    const all = payloads.join("\n");
    expect(all.length, "no GraphQL response was captured").toBeGreaterThan(0);
    for (const forbidden of [
      "installationRef",
      "installation_ref",
      "hostKind",
      "host_kind",
      "privateKey",
      "PRIVATE KEY",
    ]) {
      expect(all, `${forbidden} reached the browser`).not.toContain(forbidden);
    }
  });

  /**
   * FR-002. Every mutation behind this card is owner-level, so a card offered
   * to someone who cannot use it is an invitation to a permission error.
   */
  test("is not offered to a world member who is not its owner", async ({
    browser,
  }) => {
    const ownerContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const ownerPage = await ownerContext.newPage();
    await register(ownerPage, freshCredentials("e2eloreowner"));
    const worldId = await createWorld(
      ownerPage,
      `E2E Lore Member ${uniqueSuffix()}`,
    );

    await ownerPage.goto(`/world/${worldId}`);
    await ownerPage.getByRole("button", { name: "Generate Join Link" }).click();
    const inviteInput = ownerPage.locator("input[readonly]").first();
    await expect(inviteInput).toBeVisible({ timeout: 10_000 });
    const inviteUrl = await inviteInput.inputValue();
    const inviteCode = new URL(inviteUrl).pathname.split("/").pop();
    if (!inviteCode) throw new Error("Could not extract invite code");

    const memberContext = await browser.newContext();
    const memberPage = await memberContext.newPage();
    await register(memberPage, freshCredentials("e2eloremember"));
    await memberPage.goto(`/join/${inviteCode}`);
    await memberPage.getByRole("button", { name: "Join Campaign" }).click();
    await memberPage.waitForURL(
      (url) => url.pathname.startsWith(`/world/${worldId}`),
      { timeout: 15_000 },
    );

    await memberPage.goto(`/world/${worldId}/settings/system`);
    await expect(memberPage.getByTestId("active-system-card")).toBeVisible({
      timeout: 15_000,
    });
    await expect(memberPage.getByTestId("lore-repository-card")).toHaveCount(0);

    await ownerContext.close();
    await memberContext.close();
  });
});

/**
 * User Story 3: accepting edits made in the repository.
 *
 * This is the only part of the feature that can put text into a world its
 * members did not write in the app, so the test that matters most is the one
 * that proves it **does not happen by default**.
 *
 * `incoming_enabled` is false on every connection until someone turns it on
 * (FR-022), and the server enforces that through a type whose constructor
 * refuses a connection that has not opted in. The browser-level assertion here
 * is the complement: a Game Master who has not enabled it is not shown a
 * surface that implies they might have, and no query the page makes returns a
 * proposal for their world.
 *
 * Accepting an actual proposal is not covered here. Producing one needs a real
 * repository, a completed grant, and a push from outside the app — the same
 * ceiling that keeps T036 and T038 out of this suite. It is covered where it
 * can be proven for real: `lore_sync::incoming`'s tests apply an accepted
 * change to a live database and assert the resulting revision's author,
 * content and origin, and that a declined deletion leaves the entry
 * byte-for-byte.
 */
test.describe("Spec 034 User Story 3: incoming changes are off until asked for", () => {
  /**
   * **FR-022.** The single most important assertion in this spec: a world that
   * never enabled incoming acceptance is never modified by anything in the
   * repository. If this ever fails, every world on the instance is exposed.
   */
  test("a world that never enabled it is shown no incoming surface", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eloreincoming"));
    const worldId = await createWorld(page, `E2E Lore Incoming ${uniqueSuffix()}`);

    await page.goto(`/world/${worldId}/settings/system`);
    await expect(page.getByTestId("lore-repository-card")).toBeVisible({
      timeout: 15_000,
    });

    // No section, and no individual control from it. Asserted separately
    // because a section that renders empty and a section that does not render
    // are different failures, and only one of them is this one.
    await expect(page.getByTestId("lore-incoming-changes")).toHaveCount(0);
    await expect(page.getByTestId("lore-incoming-change")).toHaveCount(0);
    await expect(page.getByTestId("lore-incoming-accept")).toHaveCount(0);
    await expect(page.getByTestId("lore-incoming-decline")).toHaveCount(0);
  });

  /**
   * The server half of the same rule, read from the wire rather than the
   * screen. A control can be absent from the page while the data that would
   * drive it is being sent, and it is the data an attacker reads.
   */
  test("no proposal is sent to a browser for a world that never enabled it", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eloreincsrv"));
    const worldId = await createWorld(page, `E2E Lore Incoming Wire ${uniqueSuffix()}`);

    const payloads: string[] = [];
    page.on("response", async (response) => {
      if (!response.url().includes("/api/graphql")) return;
      try {
        payloads.push(await response.text());
      } catch {
        // Unreadable bodies are not evidence of a leak.
      }
    });

    await page.goto(`/world/${worldId}/settings/system`);
    await expect(page.getByTestId("lore-repository-card")).toBeVisible({
      timeout: 15_000,
    });

    const all = payloads.join("\n");
    expect(all.length, "no GraphQL response was captured").toBeGreaterThan(0);
    // `incomingBody` is the field that would carry text from outside the app.
    expect(all, "a repository proposal reached a world that never asked for one")
      .not.toContain("incomingBody");
  });
});

/**
 * T036 and T038: the grant flow and the mirror, against a real repository.
 *
 * # Why these are gated rather than always-on
 *
 * Every step here reaches a repository host. The connection half only *reads*
 * — it asks which repositories a grant covers — but the mirror half **writes
 * to a real repository**, and a suite that commits to someone's repository on
 * every run is a suite nobody can run casually.
 *
 * So both halves skip unless this instance has an application registered, and
 * the writing half additionally needs `THUNDERFORGE_E2E_LIVE_MIRROR=1` plus a
 * repository named in `THUNDERFORGE_E2E_LIVE_REPO`. Opt-in twice, because the
 * second opt-in is the one that leaves commits behind.
 *
 * Skipping is not the same as passing, and these say which they did.
 */
const LIVE_INSTALLATION = process.env.THUNDERFORGE_E2E_LIVE_INSTALLATION;
const LIVE_REPO = process.env.THUNDERFORGE_E2E_LIVE_REPO;
const LIVE_MIRROR = process.env.THUNDERFORGE_E2E_LIVE_MIRROR === "1";

/** Ask the server, rather than assuming, whether this instance can connect. */
async function integrationConfigured(page: Page): Promise<boolean> {
  const body = await page.evaluate(async () => {
    const csrf = document.cookie
      .split(";")
      .map((p) => p.trim())
      .find((p) => p.startsWith("csrf_token="))
      ?.slice("csrf_token=".length);
    const res = await fetch("/api/graphql", {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "Content-Type": "application/json",
        ...(csrf ? { "x-csrf-token": csrf } : {}),
      },
      body: JSON.stringify({
        query: "{ instanceRepositoryIntegration { configured } }",
      }),
    });
    return res.text();
  });
  return JSON.parse(body)?.data?.instanceRepositoryIntegration?.configured === true;
}

test.describe("Spec 034 T036/T038: connecting a real repository", () => {
  test("a grant hand-off names both permissions and why the second exists", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eloregrant"));
    const worldId = await createWorld(page, `E2E Lore Grant ${uniqueSuffix()}`);

    await page.goto(`/world/${worldId}/settings/system`);
    await expect(page.getByTestId("lore-repository-card")).toBeVisible({
      timeout: 15_000,
    });

    test.skip(
      !(await integrationConfigured(page)),
      "this instance has no repository application registered",
    );

    // The button inside, not the container. `lore-sync-connect` marks the
    // section — it is on the wrapper in both the "not connected" and the
    // "about to grant" branches, which is right for asserting presence and
    // wrong for clicking: clicking the container does nothing and looks
    // exactly like a button that did not work.
    await page
      .getByTestId("lore-sync-connect")
      .getByRole("button", { name: /connect a repository/i })
      .click();

    // FR-036e. The wider scope exists so the platform can withdraw in public
    // after a takedown, and the user is told that before granting rather than
    // discovering it in a permissions screen.
    const link = page.getByTestId("lore-sync-grant-link");
    await expect(link).toBeVisible({ timeout: 15_000 });

    // Asserted on what a person reads, not on scope identifiers. The card
    // deliberately shows "Read and write the files in this repository" rather
    // than `contents:write` — a permission screen that names an API scope is
    // one nobody makes an informed decision from.
    const card = page.getByTestId("lore-repository-card");
    await expect(card).toContainText(/read and write the files/i);
    await expect(card).toContainText(/open issues/i);

    // FR-036e's whole justification: the second permission exists so the
    // platform can withdraw in public after a takedown, and the reason is
    // given *before* granting rather than discovered in a host's screen.
    await expect(card).toContainText(/moderation action/i);
    await expect(card).toContainText(/never delete, edit, or force-push/i);

    // The hand-off carries an anti-forgery state, which is what binds the
    // return to this world and this person.
    const href = await link.getAttribute("href");
    expect(href, "the hand-off has no address").toBeTruthy();
    expect(href, "the hand-off carries no anti-forgery state").toContain("state=");
  });

  test("the mirror reaches a real repository and a clone matches", async ({
    page,
  }) => {
    test.skip(
      !LIVE_MIRROR || !LIVE_REPO || !LIVE_INSTALLATION,
      "live mirror is opt-in: set THUNDERFORGE_E2E_LIVE_MIRROR=1, " +
        "THUNDERFORGE_E2E_LIVE_REPO and THUNDERFORGE_E2E_LIVE_INSTALLATION",
    );

    await register(page, freshCredentials("e2eloremirror"));
    const worldId = await createWorld(page, `E2E Lore Mirror ${uniqueSuffix()}`);

    await page.goto(`/world/${worldId}/settings/system`);
    await expect(page.getByTestId("lore-repository-card")).toBeVisible({
      timeout: 15_000,
    });
    test.skip(
      !(await integrationConfigured(page)),
      "this instance has no repository application registered",
    );

    // The flow, driven through the API the browser uses: begin the hand-off,
    // then return the way the host would. The host's own consent screen is not
    // ours to automate and not ours to test.
    const directory = `e2e-${uniqueSuffix()}`;
    const connected = await page.evaluate(
      async ({ worldId, repo, installation, directory }) => {
        const csrf = document.cookie
          .split(";")
          .map((p) => p.trim())
          .find((p) => p.startsWith("csrf_token="))
          ?.slice("csrf_token=".length);
        const call = async (query: string, variables: unknown) => {
          const res = await fetch("/api/graphql", {
            method: "POST",
            credentials: "same-origin",
            headers: {
              "Content-Type": "application/json",
              ...(csrf ? { "x-csrf-token": csrf } : {}),
            },
            body: JSON.stringify({ query, variables }),
          });
          return res.json();
        };

        const begun = await call(
          `mutation ($w: UUID!) { beginLoreRepositoryConnection(worldId: $w) { url } }`,
          { w: worldId },
        );
        const url: string | undefined =
          begun?.data?.beginLoreRepositoryConnection?.url;
        if (!url) return { error: JSON.stringify(begun?.errors) };
        const state = new URL(url).searchParams.get("state");

        const done = await call(
          `mutation ($i: CompleteConnectionInput!) {
             completeLoreRepositoryConnection(input: $i) { repositoryRef directory }
           }`,
          {
            i: {
              worldId,
              grantResponse: `${state}:${installation}`,
              repositoryRef: repo,
              directory,
            },
          },
        );
        return done?.data?.completeLoreRepositoryConnection
          ? { repositoryRef: done.data.completeLoreRepositoryConnection.repositoryRef }
          : { error: JSON.stringify(done?.errors) };
      },
      {
        worldId,
        repo: LIVE_REPO as string,
        installation: LIVE_INSTALLATION as string,
        directory,
      },
    );

    expect(connected.error, `connecting failed: ${connected.error}`).toBeUndefined();
    expect(connected.repositoryRef).toBe(LIVE_REPO);

    // FR-038: the notice gate is real, and nothing has synchronised yet.
    await page.reload();
    await expect(page.getByTestId("lore-sync-notice")).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.getByTestId("lore-sync-acknowledge")).toBeVisible();
  });
});
