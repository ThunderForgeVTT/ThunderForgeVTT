import { expect, test } from "@playwright/test";
import { openAdminPage } from "./fixtures/admin";
import { freshCredentials, graphql, register } from "./fixtures/helpers";

/**
 * Spec 037 US5 (FR-016) and FR-021: the two screens that say what happened to
 * a report — the author's, and the operator's.
 *
 * # The assertion that matters most is a negative one
 *
 * The operator's queue must never show what somebody wrote. It is a diagnostic
 * surface: attempts, timings, a closed vocabulary of failure reasons. An
 * operator debugging their own credentials does not need to read a bug report
 * to do it, and a screen that showed it would make every delivery outage a
 * privacy question.
 *
 * So this plants a distinctive sentence in a submission and requires it to be
 * absent from the admin screen — while being present on the author's own.
 */

const SUBMIT = `
  mutation Submit($input: SubmitFeedbackInput!) {
    submitFeedback(input: $input) { id }
  }
`;

const PLANTED = "the treasure room lights flicker when Bramble casts";

test.describe("Spec 037 US5: what became of what I sent", () => {
  test("an author sees their own submission; another account sees none of it", async ({
    page,
    browser,
  }) => {
    const creds = freshCredentials("e2efbmine");
    await register(page, creds);

    await graphql(page, SUBMIT, {
      input: {
        kind: "ISSUE",
        message: PLANTED,
        clientVersion: "e2e",
        browser: "Chromium on Linux",
        attachments: [],
      },
    });

    await page.goto("/settings/feedback");
    const mine = page.getByTestId("my-submissions");
    await expect(mine).toBeVisible({ timeout: 20_000 });
    await expect(mine).toContainText(PLANTED);

    // This instance is the record (FR-018), so a report that arrived reads as
    // received — never as "failed to deliver", which would invite the person
    // to file it again.
    await expect(page.locator("body")).not.toContainText("failed");

    const stranger = await browser.newContext();
    const strangerPage = await stranger.newPage();
    await register(strangerPage, freshCredentials("e2efbother"));
    await strangerPage.goto("/settings/feedback");
    await expect(strangerPage.getByTestId("my-submissions-empty")).toBeVisible({
      timeout: 20_000,
    });
    await expect(strangerPage.locator("body")).not.toContainText(PLANTED);
    await stranger.close();
  });

  test("the operator sees that something is waiting, and not what it said", async ({
    page,
    browser,
  }) => {
    const creds = freshCredentials("e2efbop");
    await register(page, creds);

    await graphql(page, SUBMIT, {
      input: {
        kind: "ISSUE",
        message: PLANTED,
        clientVersion: "e2e",
        browser: "Chromium on Linux",
        attachments: [],
      },
    });

    const admin = await openAdminPage(browser);
    try {
      await admin.goto("/admin/legal");
      const panel = admin.getByTestId("undelivered-feedback-panel");
      await expect(panel).toBeVisible({ timeout: 20_000 });

      // First: the row is actually here. Without this the absence below is
      // vacuously true of an empty list — which is precisely how a test comes
      // to pass while proving nothing, and this repository has shipped that
      // mistake before.
      await expect(
        panel.locator('[data-testid^="undelivered-"]').first(),
        "nothing is queued, so the privacy assertion below would prove nothing",
      ).toBeVisible({ timeout: 20_000 });

      // And now the point of the screen, as an absence.
      await expect(panel).not.toContainText(PLANTED);
      await expect(admin.locator("body")).not.toContainText(PLANTED);
    } finally {
      await admin.context().close();
    }
  });
});
