import { execFileSync } from "node:child_process";
import { expect, test, type Browser, type Page } from "@playwright/test";
import { ADMIN_USER, DEMO_USER } from "./fixtures/global-setup";
import {
  graphql,
  inviteAndJoinAsPlayer,
  login,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * specs/015-dmca-notice-takedown, the second half of the statutory flow.
 *
 * `dmca-takedown.spec.ts` covers intake: a sufficient notice disables the
 * content, an incomplete one is rejected. Everything § 512(g) requires
 * *after* that had never been driven in a browser, and the constitution's
 * DMCA guardrail asks for a program that is fully operational, not one that
 * only knows how to take things down. This file covers the way back up:
 *
 *  - the owner files a counter-notice from the disabled content's own page
 *    (`ModeratedContentBanner` → `CounterNoticeForm`), and it attaches to
 *    the *existing* case rather than opening a new one;
 *  - filing forwards the counter-notice and schedules a restoration date
 *    (§ 512(g)(2)(B)); it does not restore anything immediately;
 *  - once that date passes, the content comes back on the next read, and the
 *    restoration is written down as a real `content_restored` event rather
 *    than being inferred (the lazy auto-restoration in `moderation/mod.rs`);
 *  - only the content's own GM may file — a player of the same world and a
 *    stranger are both refused, in the UI and at the resolver;
 *  - compliance staff can stop a restoration before the clock runs out
 *    (`/admin/moderation` → "Block restoration"), and that decision beats
 *    the timer permanently;
 *  - the repeat-infringer count (FR-009) counts only cases that are upheld
 *    *and* unrestored: a case under counter-notice review stops counting,
 *    a restored one stops counting, a staff-blocked one counts again.
 *
 * # How the waiting period is simulated
 *
 * The waiting period is 14 days (`counter_notice_waiting_period_days`), which
 * no e2e can wait out, and the mechanism is a stored timestamp
 * (`content_moderation_actions.restoration_due_at`) compared against `now()`
 * on every read. The Rust test `forwarded_counter_notice_past_due_auto_restores`
 * simulates elapsed time by writing a due date in the past; `elapseWaitingPeriod`
 * below does exactly the same thing to a case this test already created through
 * the real UI — one `UPDATE` moving one already-recorded timestamp backwards.
 *
 * It deliberately does not create, delete or reclassify any moderation event:
 * every case here is opened by a real notice through `/legal/dmca`, every
 * counter-notice is filed by its real owner, and every staff decision is made
 * by a real admin. The only thing the database is asked for is the passage of
 * time, which is not something the product exposes and should not be — the
 * alternative considered was starting the stack with
 * `MODERATION_COUNTER_NOTICE_WAITING_PERIOD_DAYS=0`, and that was rejected
 * because it is a whole-stack setting: with a zero-day period nothing is ever
 * "still within the waiting period", so the staff-block and
 * still-disabled-while-forwarded scenarios could not be tested at all.
 */

const CASE_ID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * Moves a forwarded counter-notice's recorded restoration date one day into
 * the past — the waiting period, elapsed. See the file header for why this
 * is done here rather than through a product surface.
 */
function elapseWaitingPeriod(caseId: string): void {
  if (!CASE_ID_PATTERN.test(caseId)) {
    throw new Error(`Refusing to run SQL for a non-UUID case id: ${caseId}`);
  }
  // Same connection convention as e2e/fixtures/global-setup.ts, so this
  // reaches the per-shard database under scripts/e2e-parallel.mjs.
  const container =
    process.env.THUNDERFORGE_POSTGRES_CONTAINER ?? "thunderforge-postgres";
  const database = process.env.THUNDERFORGE_DB_NAME ?? "thunderforge";
  const dbUser = process.env.THUNDERFORGE_DB_USER ?? "postgres";

  const output = execFileSync(
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
    {
      input:
        "UPDATE content_moderation_actions " +
        "SET restoration_due_at = NOW() - INTERVAL '1 day' " +
        `WHERE case_id = '${caseId}' AND action_type = 'counter_notice_forwarded' ` +
        "RETURNING id;",
      encoding: "utf-8",
      stdio: ["pipe", "pipe", "inherit"],
    },
  );

  const updatedIds = output
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => CASE_ID_PATTERN.test(line));
  if (updatedIds.length !== 1) {
    throw new Error(
      `Expected exactly one forwarded counter-notice for case ${caseId}, ` +
        `psql reported: ${JSON.stringify(output)}`,
    );
  }
}

interface GraphQLResponse<T> {
  data?: T;
  errors?: { message: string }[];
}

function unwrap<T>(response: GraphQLResponse<T>): T {
  if (!response.data) {
    throw new Error(`GraphQL call failed: ${JSON.stringify(response.errors)}`);
  }
  return response.data;
}

async function createItem(
  page: Page,
  worldId: string,
  name: string,
): Promise<string> {
  const created = await graphql<
    GraphQLResponse<{ createItem: { id: string } }>
  >(
    page,
    `
      mutation CreateItem($input: CreateItemInput!) {
        createItem(input: $input) {
          id
        }
      }
    `,
    { input: { worldId, name } },
  );
  return unwrap(created).createItem.id;
}

async function accountId(page: Page): Promise<string> {
  const me = await graphql<GraphQLResponse<{ me: { id: string } | null }>>(
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
  const id = unwrap(me).me?.id;
  if (!id) {
    throw new Error("Expected a signed-in account");
  }
  return id;
}

/**
 * Files a real takedown through the public intake channel from a logged-out
 * context (FR-002), and returns the case reference the claimant is shown.
 * Mirrors collection-moderation.spec.ts's helper, extended to hand back the
 * case id — which is what every scenario below then acts on.
 */
async function fileTakedown(
  claimant: Page,
  entityId: string,
  what: string,
): Promise<string> {
  await claimant.goto("/legal/dmca");
  await expect(claimant.getByTestId("takedown-notice-form")).toBeVisible();

  await claimant.getByLabel("Content type").click();
  await claimant.getByRole("option", { name: "Item" }).click();
  await claimant.locator("#dmca-entity-id").fill(entityId);
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
}

interface ModerationCaseView {
  caseId: string;
  entityId: string;
  currentStatus: string;
  events: { actionType: string; restorationDueAt: string | null }[];
}

/** The staff-only case view — the audit trail, read as compliance staff. */
async function readCase(
  admin: Page,
  caseId: string,
): Promise<ModerationCaseView> {
  const response = await graphql<
    GraphQLResponse<{ moderationCase: ModerationCaseView | null }>
  >(
    admin,
    `
      query Case($caseId: UUID!) {
        moderationCase(caseId: $caseId) {
          caseId
          entityId
          currentStatus
          events {
            actionType
            restorationDueAt
          }
        }
      }
    `,
    { caseId },
  );
  const record = unwrap(response).moderationCase;
  if (!record) {
    throw new Error(`No moderation case ${caseId}`);
  }
  return record;
}

async function repeatInfringerFlags(admin: Page): Promise<string[]> {
  const response = await graphql<
    GraphQLResponse<{ repeatInfringerFlags: string[] }>
  >(
    admin,
    `
      query Flags {
        repeatInfringerFlags
      }
    `,
    {},
  );
  return unwrap(response).repeatInfringerFlags;
}

/** Files a counter-notice at the resolver, for the cases whose subject is the
 * bookkeeping rather than the form — the form itself is driven through the UI
 * in the first test. */
async function fileCounterNoticeViaApi(
  owner: Page,
  caseId: string,
): Promise<
  GraphQLResponse<{ submitCounterNotice: { currentStatus: string } }>
> {
  return graphql<
    GraphQLResponse<{ submitCounterNotice: { currentStatus: string } }>
  >(
    owner,
    `
      mutation CounterNotice($input: SubmitCounterNoticeInput!) {
        submitCounterNotice(input: $input) {
          caseId
          currentStatus
        }
      }
    `,
    {
      input: {
        caseId,
        removedMaterialDescription:
          "The item this notice named, which is my own original work.",
        goodFaithMistakeStatement: true,
        consentToJurisdiction: true,
        contactInformation: "owner@example.test",
        signature: "Owen Owner",
      },
    },
  );
}

async function openAdminPage(browser: Browser): Promise<Page> {
  const context = await browser.newContext();
  const page = await context.newPage();
  await login(page, ADMIN_USER.identifier, ADMIN_USER.password);
  await page.waitForURL(/\/(admin|welcome)$/, { timeout: 20_000 });
  return page;
}

test.describe("spec 015 US2: the counter-notice half of notice-and-takedown", () => {
  test("a counter-notice is forwarded and scheduled, and the content returns only once the waiting period has run out", async ({
    page,
    browser,
  }) => {
    test.setTimeout(240_000);

    const suffix = uniqueSuffix();
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Counter-Notice ${suffix}`,
      "e2ecnowner",
    );
    const itemName = `Contested Chalice ${suffix}`;
    const itemId = await createItem(page, worldId, itemName);
    const itemPath = `/world/${worldId}/item/${itemId}/view`;

    const claimantContext = await browser.newContext();
    const claimant = await claimantContext.newPage();
    const admin = await openAdminPage(browser);

    try {
      const caseId = await fileTakedown(
        claimant,
        itemId,
        `Item "${itemName}" in a ThunderForge world.`,
      );

      // The owner-facing half of FR-005: the disabled content's own page is
      // where the counter-notice is filed from, and only the owner is offered it.
      await page.goto(itemPath);
      await expect(page.getByText("Content disabled")).toBeVisible({
        timeout: 15_000,
      });
      const form = page.getByTestId("counter-notice-form");
      await expect(form).toBeVisible();

      await page
        .locator("#counter-notice-material")
        .fill(`The item "${itemName}", which is my own original work.`);
      await page.locator("#counter-notice-contact").fill("owner@example.test");
      await page.locator("#counter-notice-good-faith").click();
      await page.locator("#counter-notice-jurisdiction").click();
      await page.locator("#counter-notice-signature").fill("Owen Owner");
      await page.getByTestId("counter-notice-submit").click();

      // It joins the case the notice opened; a counter-notice that started a
      // second case would leave the claimant's notice unanswered on the first.
      const confirmation = page.getByText(/Counter-notice filed/i);
      await expect(confirmation).toBeVisible({ timeout: 15_000 });
      await expect(
        page.locator("code").filter({ hasText: caseId }),
      ).toHaveCount(1);

      // § 512(g)(2)(B)/(C): filing forwards the counter-notice and starts a
      // clock. It does not put the content back — that is the claimant's
      // window to go to court, and skipping it would be the platform deciding
      // the dispute itself.
      await page.goto(itemPath);
      await expect(page.getByText("Content disabled")).toBeVisible({
        timeout: 15_000,
      });
      await expect(page.getByRole("heading", { name: itemName })).toHaveCount(
        0,
      );

      const forwarded = await readCase(admin, caseId);
      expect(forwarded.currentStatus).toBe("COUNTER_NOTICE_FORWARDED");
      expect(forwarded.events.map((event) => event.actionType)).toEqual([
        "NOTICE_RECEIVED",
        "CONTENT_DISABLED",
        "COUNTER_NOTICE_RECEIVED",
        "COUNTER_NOTICE_FORWARDED",
      ]);
      const dueAt = forwarded.events.at(-1)?.restorationDueAt;
      if (!dueAt) {
        throw new Error(
          "The forwarded counter-notice carries no restoration date",
        );
      }
      const daysOut = (new Date(dueAt).getTime() - Date.now()) / 86_400_000;
      // The statutory floor is 10 business days; the configured default is 14.
      // Anything at or below zero would mean the content was scheduled to
      // return before the claimant had any chance to respond.
      expect(daysOut).toBeGreaterThan(10);
      expect(daysOut).toBeLessThanOrEqual(15);

      elapseWaitingPeriod(caseId);

      // FR-007: with the window closed and no court action recorded, the very
      // next read of the content restores it.
      await page.goto(itemPath);
      await expect(page.getByRole("heading", { name: itemName })).toBeVisible({
        timeout: 15_000,
      });
      await expect(page.getByText("Content disabled")).toHaveCount(0);

      // And the restoration is a durable, auditable fact rather than something
      // recomputed on every read — the audit trail has to be able to say when
      // the content came back, a year later, without replaying the clock.
      const restored = await readCase(admin, caseId);
      expect(restored.currentStatus).toBe("CONTENT_RESTORED");
      expect(
        restored.events.filter(
          (event) => event.actionType === "CONTENT_RESTORED",
        ),
      ).toHaveLength(1);
    } finally {
      await claimantContext.close();
      await admin.context().close();
    }
  });

  test("only the content's own GM may file the counter-notice", async ({
    page,
    browser,
  }) => {
    test.setTimeout(240_000);

    const suffix = uniqueSuffix();
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Counter-Notice Standing ${suffix}`,
      "e2ecnstand",
    );
    const itemName = `Disputed Lantern ${suffix}`;
    const itemId = await createItem(page, worldId, itemName);

    const claimantContext = await browser.newContext();
    const claimant = await claimantContext.newPage();
    const strangerContext = await browser.newContext();
    const stranger = await strangerContext.newPage();

    try {
      const caseId = await fileTakedown(
        claimant,
        itemId,
        `Item "${itemName}" in a ThunderForge world.`,
      );

      // A member of the same world who is not its GM. They can see that the
      // content was disabled — that is the point of the banner — but the
      // counter-notice is a sworn legal statement about ownership, and it is
      // not theirs to swear.
      const player = await inviteAndJoinAsPlayer(
        browser,
        page,
        worldId,
        "e2ecnplayer",
      );
      await player.goto(`/world/${worldId}/item/${itemId}/view`);
      await expect(player.getByText("Content disabled")).toBeVisible({
        timeout: 15_000,
      });
      await expect(player.getByTestId("counter-notice-form")).toHaveCount(0);

      const playerAttempt = await fileCounterNoticeViaApi(player, caseId);
      expect(playerAttempt.data?.submitCounterNotice).toBeUndefined();
      expect(playerAttempt.errors?.[0]?.message ?? "").toMatch(
        /owning GM may submit a counter-notice/i,
      );

      // And someone with no connection to the world at all.
      await login(stranger, DEMO_USER.identifier, DEMO_USER.password);
      await stranger.waitForURL(/\/welcome$/, { timeout: 20_000 });
      const strangerAttempt = await fileCounterNoticeViaApi(stranger, caseId);
      expect(strangerAttempt.data?.submitCounterNotice).toBeUndefined();
      expect(strangerAttempt.errors?.[0]?.message ?? "").toMatch(
        /owning GM may submit a counter-notice/i,
      );

      // Nothing either of them did moved the case.
      const admin = await openAdminPage(browser);
      try {
        const record = await readCase(admin, caseId);
        expect(record.currentStatus).toBe("CONTENT_DISABLED");
      } finally {
        await admin.context().close();
      }

      await player.context().close();
    } finally {
      await claimantContext.close();
      await strangerContext.close();
    }
  });

  test("compliance staff can block a restoration before the clock runs out, and that decision outlasts the clock", async ({
    page,
    browser,
  }) => {
    test.setTimeout(360_000);

    const suffix = uniqueSuffix();
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Blocked Restoration ${suffix}`,
      "e2ecnblock",
    );
    const ownerAccountId = await accountId(page);

    // Four cases, because `/admin/moderation` reaches a case only through a
    // repeat-infringer flag and the threshold is three: with three, filing the
    // counter-notice would drop this account off the list and the "Block
    // restoration" button would be unreachable in the UI (see the report note).
    const items: { id: string; name: string; caseId: string }[] = [];
    const claimantContext = await browser.newContext();
    const claimant = await claimantContext.newPage();
    const admin = await openAdminPage(browser);

    try {
      for (let i = 0; i < 4; i += 1) {
        const name = `Blocked Relic ${i} ${suffix}`;
        const id = await createItem(page, worldId, name);
        const caseId = await fileTakedown(
          claimant,
          id,
          `Item "${name}" in a ThunderForge world.`,
        );
        items.push({ id, name, caseId });
      }

      const contested = items[0];
      const untouched = items[1];

      // The owner files a counter-notice on one of them, through the page.
      await page.goto(`/world/${worldId}/item/${contested.id}/view`);
      await expect(page.getByTestId("counter-notice-form")).toBeVisible({
        timeout: 15_000,
      });
      await page
        .locator("#counter-notice-material")
        .fill(`The item "${contested.name}", which is my own work.`);
      await page.locator("#counter-notice-contact").fill("owner@example.test");
      await page.locator("#counter-notice-good-faith").click();
      await page.locator("#counter-notice-jurisdiction").click();
      await page.locator("#counter-notice-signature").fill("Owen Owner");
      await page.getByTestId("counter-notice-submit").click();
      await expect(page.getByText(/Counter-notice filed/i)).toBeVisible({
        timeout: 15_000,
      });

      // Staff reach the case the way the product actually offers it.
      await admin.goto("/admin/moderation");
      const flagRow = admin
        .getByRole("listitem")
        .filter({ hasText: ownerAccountId });
      await expect(flagRow).toBeVisible({ timeout: 20_000 });
      await flagRow.getByRole("button", { name: "View history" }).click();

      const contestedRow = admin
        .getByRole("listitem")
        .filter({ hasText: contested.caseId });
      await expect(contestedRow).toContainText("COUNTER_NOTICE_FORWARDED", {
        timeout: 20_000,
      });

      // Only a case with a pending counter-notice offers the block — there is
      // nothing to block on a case whose content is already staying down.
      const untouchedRow = admin
        .getByRole("listitem")
        .filter({ hasText: untouched.caseId });
      await expect(untouchedRow).toContainText("CONTENT_DISABLED");
      await expect(
        untouchedRow.getByRole("button", { name: /Block restoration/i }),
      ).toHaveCount(0);

      await contestedRow
        .getByRole("button", { name: /Block restoration/i })
        .click();
      await expect(
        admin.getByRole("listitem").filter({ hasText: contested.caseId }),
      ).toContainText("CONTENT_REMAINS_DISABLED", { timeout: 20_000 });

      // The clock then runs out anyway. A staff decision made inside the
      // window is the answer to the dispute; a timer must not overturn it.
      elapseWaitingPeriod(contested.caseId);

      await page.goto(`/world/${worldId}/item/${contested.id}/view`);
      await expect(page.getByText("Content disabled")).toBeVisible({
        timeout: 15_000,
      });
      await expect(
        page.getByRole("heading", { name: contested.name }),
      ).toHaveCount(0);

      const record = await readCase(admin, contested.caseId);
      expect(record.currentStatus).toBe("CONTENT_REMAINS_DISABLED");
      expect(
        record.events.some((event) => event.actionType === "CONTENT_RESTORED"),
        "a blocked case must never acquire a restoration event",
      ).toBe(false);
    } finally {
      await claimantContext.close();
      await admin.context().close();
    }
  });

  test("the repeat-infringer count reflects upheld, unrestored cases only", async ({
    page,
    browser,
  }) => {
    test.setTimeout(360_000);

    const suffix = uniqueSuffix();
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Repeat Infringer ${suffix}`,
      "e2ecnrepeat",
    );
    const ownerAccountId = await accountId(page);

    const claimantContext = await browser.newContext();
    const claimant = await claimantContext.newPage();
    const admin = await openAdminPage(browser);

    try {
      // Exactly the default threshold (3), so every subsequent change of one
      // case's outcome is visible as the account appearing or disappearing.
      const items: { id: string; name: string; caseId: string }[] = [];
      for (let i = 0; i < 3; i += 1) {
        const name = `Strike ${i} ${suffix}`;
        const id = await createItem(page, worldId, name);
        const caseId = await fileTakedown(
          claimant,
          id,
          `Item "${name}" in a ThunderForge world.`,
        );
        items.push({ id, name, caseId });
      }

      expect(await repeatInfringerFlags(admin)).toContain(ownerAccountId);

      // A case under counter-notice review is not a strike: the account has
      // disputed it and the platform has not yet decided.
      const disputed = items[0];
      const disputedResult = await fileCounterNoticeViaApi(
        page,
        disputed.caseId,
      );
      expect(disputedResult.data?.submitCounterNotice.currentStatus).toBe(
        "COUNTER_NOTICE_FORWARDED",
      );
      expect(await repeatInfringerFlags(admin)).not.toContain(ownerAccountId);

      // Staff uphold the takedown despite the counter-notice: it counts again.
      const resolved = await graphql<
        GraphQLResponse<{ resolveModerationCase: { currentStatus: string } }>
      >(
        admin,
        `
          mutation Resolve($caseId: UUID!, $resolution: ModerationActionType!) {
            resolveModerationCase(caseId: $caseId, resolution: $resolution) {
              currentStatus
            }
          }
        `,
        { caseId: disputed.caseId, resolution: "CONTENT_REMAINS_DISABLED" },
      );
      expect(unwrap(resolved).resolveModerationCase.currentStatus).toBe(
        "CONTENT_REMAINS_DISABLED",
      );
      expect(await repeatInfringerFlags(admin)).toContain(ownerAccountId);

      // A case that ends in restoration is not a strike at all — the content
      // came back, so nothing about it may count towards terminating the
      // account (FR-009: repeat *infringement*, not repeat accusation).
      const restoredCase = items[1];
      const restoredResult = await fileCounterNoticeViaApi(
        page,
        restoredCase.caseId,
      );
      expect(restoredResult.data?.submitCounterNotice.currentStatus).toBe(
        "COUNTER_NOTICE_FORWARDED",
      );
      elapseWaitingPeriod(restoredCase.caseId);

      await page.goto(`/world/${worldId}/item/${restoredCase.id}/view`);
      await expect(
        page.getByRole("heading", { name: restoredCase.name }),
      ).toBeVisible({ timeout: 15_000 });
      expect((await readCase(admin, restoredCase.caseId)).currentStatus).toBe(
        "CONTENT_RESTORED",
      );

      expect(await repeatInfringerFlags(admin)).not.toContain(ownerAccountId);
    } finally {
      await claimantContext.close();
      await admin.context().close();
    }
  });
});
