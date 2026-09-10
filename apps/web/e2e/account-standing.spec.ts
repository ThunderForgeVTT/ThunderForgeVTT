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
