import { execFileSync } from "node:child_process";
import { expect, test, type Browser, type Page } from "@playwright/test";
import {
  currentSharingTermsVersion,
  graphql,
  loginAsAdmin,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Spec 039 US5 (quickstart Scenario E): repeat infringement costs the ability
 * to publish — and only that.
 *
 * Every strike here is made the way a real one is: a notice filed through the
 * public DMCA form by somebody with no account, which disables the content and
 * so counts. The restoration is made the way a real one is: an administrator
 * resolving the case. Nothing is written to the database by hand, because
 * FR-020's whole point is that nobody has to.
 */

test.describe.configure({ mode: "serial" });

type Gql<T> = { data?: T; errors?: { message: string }[] };

const CASE_ID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

async function createItem(
  page: Page,
  worldId: string,
  name: string,
): Promise<string> {
  const created = await graphql<Gql<{ createItem: { id: string } }>>(
    page,
    `
      mutation I($input: CreateItemInput!) {
        createItem(input: $input) {
          id
        }
      }
    `,
    { input: { worldId, name, description: null } },
  );
  const id = created.data?.createItem.id;
  expect(id, JSON.stringify(created.errors)).toBeTruthy();
  return id as string;
}

async function shareItem(
  page: Page,
  itemId: string,
): Promise<Gql<{ createItemShareLink: { shareCode: string } }>> {
  return graphql<Gql<{ createItemShareLink: { shareCode: string } }>>(
    page,
    `
      mutation S($itemId: UUID!, $attestation: AttestationInput!) {
        createItemShareLink(itemId: $itemId, attestation: $attestation) {
          shareCode
        }
      }
    `,
    {
      itemId,
      attestation: { termsVersionId: await currentSharingTermsVersion(page) },
    },
  );
}

/** A real notice, through the public form, from somebody with no account. */
async function fileTakedown(
  browser: Browser,
  itemId: string,
  what: string,
): Promise<string> {
  const context = await browser.newContext();
  const claimant = await context.newPage();
  try {
    await claimant.goto("/legal/dmca");
    await expect(claimant.getByTestId("takedown-notice-form")).toBeVisible();
    await claimant.getByLabel("Content type").click();
    await claimant.getByRole("option", { name: "Item" }).click();
    await claimant.locator("#dmca-entity-id").fill(itemId);
    await claimant.locator("#dmca-claimant-name").fill("Jane Claimant");
    await claimant
      .locator("#dmca-claimant-contact")
      .fill("jane.claimant@example.test");
    await claimant
      .locator("#dmca-work-description")
      .fill("An original work, registered copyright.");
    await claimant.locator("#dmca-infringing-location").fill(what);
    await claimant.locator("#dmca-good-faith").click();
    await claimant.locator("#dmca-accuracy").click();
    await claimant.locator("#dmca-signature").fill("Jane Claimant");
    await claimant.getByTestId("takedown-notice-submit").click();
    const accepted = claimant.getByTestId("takedown-notice-accepted");
    await expect(accepted).toBeVisible({ timeout: 15_000 });
    const caseId = (await accepted.locator("code").innerText()).trim();
    expect(caseId).toMatch(CASE_ID_PATTERN);
    return caseId;
  } finally {
    await context.close();
  }
}

test.describe("Spec 039 US5: strikes cost publishing before they cost the account", () => {
  test.setTimeout(300_000);

  test("two strikes pause sharing and nothing else, and a restoration brings it back", async ({
    page,
    browser,
  }) => {
    const suffix = uniqueSuffix();
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Standing ${suffix}`,
      "e2estanding",
    );
    const taken = [
      await createItem(page, worldId, `Borrowed Map ${suffix}`),
      await createItem(page, worldId, `Borrowed Seal ${suffix}`),
    ];
    const mine = await createItem(page, worldId, `My Own Lantern ${suffix}`);

    // Before anything: sharing works, and the standing page says so.
    const before = await shareItem(page, mine);
    expect(
      before.data?.createItemShareLink?.shareCode,
      JSON.stringify(before.errors),
    ).toBeTruthy();

    const caseIds: string[] = [];
    for (const [index, itemId] of taken.entries()) {
      caseIds.push(
        await fileTakedown(browser, itemId, `Item ${index + 1} of ${suffix}`),
      );
    }

    // FR-029 and FR-028: the person can see both strikes, and was told.
    await page.goto("/settings/standing");
    await expect(page.getByTestId("standing-suspended")).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.getByTestId("standing-strike")).toHaveCount(2);
    await expect(
      page.locator(
        '[data-testid="standing-notice"][data-kind="strike_recorded"]',
      ),
      "told of each strike, not only the one that cost something",
    ).toHaveCount(2);
    await expect(
      page.locator(
        '[data-testid="standing-notice"][data-kind="publishing_suspended"]',
      ),
    ).toHaveCount(1);

    // Scenario 1: refused, and told why in terms somebody can act on.
    const refused = await shareItem(page, mine);
    expect(refused.data?.createItemShareLink).toBeFalsy();
    const message = refused.errors?.[0]?.message ?? "";
    expect(message).toContain("Sharing is paused");
    expect(message, "pointed at the process, not at a reload").toContain(
      "counter-notice",
    );

    // Scenario 2: losing publishing is not losing the work. The account can
    // still open what it made and still make more.
    await page.goto(`/world/${worldId}/item/${mine}/view`);
    await expect(
      page.getByRole("heading", { name: `My Own Lantern ${suffix}` }),
    ).toBeVisible({ timeout: 15_000 });
    await createItem(page, worldId, `Still Making Things ${suffix}`);

    // Scenario 3: the standing changes under the existing process, and
    // publishing comes back without anybody editing a database by hand.
    const adminContext = await browser.newContext();
    const admin = await adminContext.newPage();
    try {
      await loginAsAdmin(admin);
      await admin.waitForURL(/\/(admin|welcome)$/, { timeout: 20_000 });
      const resolved = await graphql<
        Gql<{ resolveModerationCase: { currentStatus: string } }>
      >(
        admin,
        `
          mutation R($caseId: UUID!, $resolution: ModerationActionType!) {
            resolveModerationCase(caseId: $caseId, resolution: $resolution) {
              currentStatus
            }
          }
        `,
        { caseId: caseIds[0], resolution: "CONTENT_RESTORED" },
      );
      expect(
        resolved.data?.resolveModerationCase?.currentStatus,
        JSON.stringify(resolved.errors),
      ).toBe("CONTENT_RESTORED");
    } finally {
      await adminContext.close();
    }

    const after = await shareItem(page, mine);
    expect(
      after.data?.createItemShareLink?.shareCode,
      `sharing is back: ${JSON.stringify(after.errors)}`,
    ).toBeTruthy();
    await page.goto("/settings/standing");
    await expect(page.getByTestId("standing-may-publish")).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.getByTestId("standing-strike")).toHaveCount(1);
  });
});

// ---------------------------------------------------------------------------
// US7 (T081, quickstart Scenario G): three strikes, the window, both remedies,
// and both ways back.
// ---------------------------------------------------------------------------

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * Runs SQL against this shard's database — only ever to move a recorded time,
 * which is the one thing the product does not and should not expose. Every
 * strike, appeal and decision below is made through the product. Every value
 * interpolated is checked against a strict pattern first.
 */
function sql(statement: string): string {
  const container =
    process.env.THUNDERFORGE_POSTGRES_CONTAINER ?? "thunderforge-postgres";
  const database = process.env.THUNDERFORGE_DB_NAME ?? "thunderforge";
  const dbUser = process.env.THUNDERFORGE_DB_USER ?? "postgres";
  return execFileSync(
    "docker",
    [
      "exec",
      "-i",
      container,
      "psql",
      "-U",
      dbUser,
      "-d",
      database,
      "-v",
      "ON_ERROR_STOP=1",
      "-t",
      "-A",
    ],
    { input: statement, encoding: "utf-8", stdio: ["pipe", "pipe", "inherit"] },
  );
}

function uuid(value: string): string {
  if (!UUID_PATTERN.test(value)) {
    throw new Error(`Refusing to put a non-UUID into SQL: ${value}`);
  }
  return value;
}

/** The window's end, moved into the past: the thirty days, elapsed. */
function elapseTheWindow(accountId: string): void {
  sql(
    `UPDATE account_terminations SET deletion_due_at = NOW() - INTERVAL '1 day' ` +
      `WHERE account_id = '${uuid(accountId)}' AND closed_at IS NULL;`,
  );
}

/** One case, a year and a bit ago: the lookback, elapsed. */
function ageTheCase(caseId: string): void {
  sql(
    `UPDATE content_moderation_actions SET created_at = NOW() - INTERVAL '400 days' ` +
      `WHERE case_id = '${uuid(caseId)}';`,
  );
}

function accountExists(accountId: string): boolean {
  return (
    sql(
      `SELECT count(*) FROM users WHERE id = '${uuid(accountId)}';`,
    ).trim() === "1"
  );
}

async function myAccountId(page: Page): Promise<string> {
  const me = await graphql<Gql<{ me: { id: string } | null }>>(
    page,
    `
      query Me {
        me {
          id
        }
      }
    `,
    {},
  );
  const id = me.data?.me?.id;
  expect(id, JSON.stringify(me.errors)).toBeTruthy();
  return id as string;
}

type StandingView = {
  disabled: boolean;
  strikeCount: number;
  termination: { deletionDueAt: string; appealState: string } | null;
};

async function myStanding(page: Page): Promise<StandingView> {
  const read = await graphql<Gql<{ myStanding: StandingView }>>(
    page,
    `
      query S {
        myStanding {
          disabled
          strikeCount
          termination {
            deletionDueAt
            appealState
          }
        }
      }
    `,
    {},
  );
  expect(read.data?.myStanding, JSON.stringify(read.errors)).toBeTruthy();
  return read.data?.myStanding as StandingView;
}

/** The sweep runs wherever an administrator looks (T072). */
async function sweep(admin: Page): Promise<void> {
  const flags = await graphql<Gql<{ repeatInfringerFlags: string[] }>>(
    admin,
    `
      query F {
        repeatInfringerFlags
      }
    `,
    {},
  );
  expect(
    flags.data?.repeatInfringerFlags,
    JSON.stringify(flags.errors),
  ).toBeTruthy();
}

async function adminMutation(
  admin: Page,
  query: string,
  variables: Record<string, unknown>,
): Promise<void> {
  const result = await graphql<Gql<unknown>>(admin, query, variables);
  expect(result.errors, JSON.stringify(result.errors)).toBeFalsy();
}

test.describe("Spec 039 US7: three strikes, the window, and both remedies", () => {
  test.setTimeout(600_000);

  test("disabled on the third, both remedies kept, and restored either way", async ({
    page,
    browser,
  }) => {
    const adminContext = await browser.newContext();
    const admin = await adminContext.newPage();
    await loginAsAdmin(admin);
    await admin.waitForURL(/\/(admin|welcome)$/, { timeout: 20_000 });

    try {
      const suffix = uniqueSuffix();
      const worldId = await registerAndCreateWorld(
        page,
        `E2E Window ${suffix}`,
        "e2ewindow",
      );
      const accountId = await myAccountId(page);
      const items: string[] = [];
      for (const name of ["Map", "Seal", "Crown", "Ring"]) {
        items.push(
          await createItem(page, worldId, `Borrowed ${name} ${suffix}`),
        );
      }

      // Scenario 2: the third strike disables the account and opens the
      // window, and the person is told plainly what happens at the end.
      const cases: string[] = [];
      for (const itemId of items.slice(0, 3)) {
        cases.push(await fileTakedown(browser, itemId, `Item ${suffix}`));
      }

      await page.goto("/settings/standing");
      await expect(page.getByTestId("standing-disabled")).toBeVisible({
        timeout: 15_000,
      });
      await expect(page.getByTestId("standing-window")).toBeVisible();
      await expect(page.getByTestId("standing-deletion-date")).toBeVisible();
      await expect(
        page.locator(
          '[data-testid="standing-notice"][data-kind="account_disabled"]',
        ),
        "told at the start of the window, first sentence, that it is irreversible",
      ).toContainText("permanently and irreversibly deleted");

      // Scenario 3: the remedies and nothing else. Anywhere in the product
      // sends a disabled account back to them; anything else is refused.
      await page.goto("/welcome");
      await expect(page).toHaveURL(/\/settings\/standing$/, {
        timeout: 15_000,
      });
      const refused = await graphql<Gql<{ myWorlds: unknown }>>(
        page,
        `
          query W {
            myWorlds {
              id
            }
          }
        `,
        {},
      );
      expect(refused.errors?.[0]?.message ?? "").toContain(
        "This account is disabled",
      );

      // Scenario 4: downloading gets the data and moves nothing in the window.
      const before = await myStanding(page);
      const download = await page.request.get("/api/user/data/export");
      expect(download.status(), "the download is a remedy, not a refusal").toBe(
        200,
      );
      const exported = (await download.json()) as { user: { id: string } };
      expect(exported.user.id).toBe(accountId);
      expect((await myStanding(page)).termination?.deletionDueAt).toBe(
        before.termination?.deletionDueAt,
      );

      // The appeal, through the page. Filing it does not cost the download.
      await page
        .locator("#standing-appeal")
        .fill("These items are mine; the notices named the wrong author.");
      await page.getByRole("button", { name: "File appeal" }).click();
      await expect(page.getByTestId("standing-appeal-open")).toBeVisible({
        timeout: 15_000,
      });
      expect(
        (await page.request.get("/api/user/data/export")).status(),
        "and appealing does not cost the download (FR-032)",
      ).toBe(200);

      // FR-034: an appeal open when the window ends pauses the deletion.
      elapseTheWindow(accountId);
      await sweep(admin);
      expect(
        accountExists(accountId),
        "nothing deleted while an appeal is open",
      ).toBe(true);

      // Scenario 5: upheld — restored, deletion cancelled, the overturned
      // strike no longer counts.
      await adminMutation(
        admin,
        `mutation R($accountId: UUID!) {
          resolveAppeal(accountId: $accountId, upheld: true) { appealState }
        }`,
        { accountId },
      );
      const restored = await myStanding(page);
      expect(restored.disabled).toBe(false);
      expect(restored.termination).toBeNull();
      expect(restored.strikeCount).toBe(2);
      await page.goto("/welcome");
      await expect(page).toHaveURL(/\/welcome$/, { timeout: 15_000 });

      // Scenario 7: a fourth takedown disables it again; then a strike ages
      // out while disabled, and the account comes back without anyone asking.
      cases.push(await fileTakedown(browser, items[3], `Item 4 of ${suffix}`));
      expect((await myStanding(page)).disabled).toBe(true);
      ageTheCase(cases[1]);
      await sweep(admin);
      const agedOut = await myStanding(page);
      expect(agedOut.disabled).toBe(false);
      expect(agedOut.strikeCount).toBe(2);
      await page.goto("/settings/standing");
      await expect(
        page.locator(
          '[data-testid="standing-notice"][data-kind="account_restored"]',
        ),
      ).toHaveCount(1, { timeout: 15_000 });

      // Scenario 6: a window that ends with no successful appeal ends in a
      // real deletion — here with the shipped default, a person deciding.
      const otherContext = await browser.newContext();
      const other = await otherContext.newPage();
      try {
        const otherWorld = await registerAndCreateWorld(
          other,
          `E2E Window Ends ${suffix}`,
          "e2ewindowend",
        );
        const otherId = await myAccountId(other);
        for (const name of ["Cup", "Blade", "Shield"]) {
          const itemId = await createItem(
            other,
            otherWorld,
            `${name} ${suffix}`,
          );
          await fileTakedown(browser, itemId, `${name} of ${suffix}`);
        }
        elapseTheWindow(otherId);
        await adminMutation(
          admin,
          `mutation E($accountId: UUID!) { executeTermination(accountId: $accountId) }`,
          { accountId: otherId },
        );
        expect(accountExists(otherId), "real deletion (FR-036)").toBe(false);
      } finally {
        await otherContext.close();
      }
    } finally {
      await adminContext.close();
    }
  });
});
