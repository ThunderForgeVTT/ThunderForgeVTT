import { expect, test } from "@playwright/test";
import { freshCredentials, register } from "./fixtures/helpers";

/**
 * Spec 037 US2 and SC-004: the evidence a report carries, and what is removed
 * from it before it leaves.
 *
 * # The assertion this file exists for
 *
 * SC-004: a secret planted in the page's own logs must not reach the review,
 * and therefore cannot reach a destination. `feedbackRedaction.test.ts` proves
 * the filter as a function and `feedback.spec.ts` proves the server refuses an
 * unfiltered bundle — neither proves the thing in between, which is that the
 * filter is actually wired into the capture a person triggers.
 *
 * So this plants a bearer token in `console.error` from the page itself, opens
 * the dialog, and requires the review to show the log bundle *without* it.
 *
 * # What the capture flags do and do not prove
 *
 * `playwright.config.ts` passes `--use-fake-ui-for-media-stream` and
 * `--auto-select-desktop-capture-source`, because `getDisplayMedia`'s picker is
 * browser chrome that Playwright cannot click. They make the capture path
 * testable. They specifically do **not** prove a person saw a picker — the
 * flag is what removed it — so that stays on the by-hand pass, named in
 * `quickstart.md` rather than assumed covered here.
 */

const BEARER = "Bearer abcdefgh12345678ABCDEFGH";

test.describe("Spec 037 US2: the evidence, and what is taken out of it", () => {
  test("a secret in the page's own logs never reaches the review", async ({
    page,
  }) => {
    const creds = freshCredentials("e2efbevid");
    await register(page, creds);
    await page.goto("/worlds");

    // Logged by the page, into the buffer the dialog captures from — not
    // injected into the payload. The filter has to be wired into capture for
    // this to be caught, which is exactly the seam being tested.
    await page.evaluate((token) => {
      console.error(`GET /api/worlds failed\nauthorization: ${token}`);
      console.warn("scene load retried");
    }, BEARER);

    await page.getByTestId("feedback-launcher").click();
    await page.getByTestId("feedback-kind-issue").click();
    await page.getByTestId("feedback-summary").fill("Something logged badly");
    await page
      .getByTestId("feedback-message")
      .fill("There was an error in the console when the scene loaded.");
    await page.getByTestId("feedback-continue").click();

    const review = page.getByTestId("feedback-dialog");
    await expect(review).toBeVisible({ timeout: 20_000 });

    // SC-004. The whole dialog, not just the bundle — a token echoed into a
    // count, a tooltip or a status line is still a token on screen.
    await expect(review).not.toContainText("abcdefgh12345678");
    await expect(review).not.toContainText(BEARER);

    // And the bundle is genuinely there, or the absence above proves nothing.
    await expect(page.getByTestId("feedback-review-log-counts")).toBeVisible();
  });

  test("evidence can be removed before sending, and removing it says so", async ({
    page,
  }) => {
    const creds = freshCredentials("e2efbrem");
    await register(page, creds);
    await page.goto("/worlds");

    await page.evaluate(() => {
      console.error("a perfectly ordinary error");
    });

    await page.getByTestId("feedback-launcher").click();
    await page.getByTestId("feedback-kind-general").click();
    await page
      .getByTestId("feedback-message")
      .fill("Adding a note without my logs attached.");
    await page.getByTestId("feedback-continue").click();

    // FR-008: what is attached is the person's decision, and the screen says
    // what it will be. Toggling the logs off must visibly change that.
    const toggle = page.getByTestId("feedback-toggle-logs");
    await expect(toggle).toBeVisible({ timeout: 20_000 });
    await toggle.click();
    await expect(page.getByTestId("feedback-removed-logs")).toBeVisible();

    await page.getByTestId("feedback-submit").click();
    await expect(page.getByTestId("feedback-sent")).toBeVisible({
      timeout: 20_000,
    });

    // Declining evidence still submits — the report is worth having without
    // it, and refusing to send would make the toggle a trap.
    await page.goto("/settings/feedback");
    await expect(page.getByTestId("my-submissions")).toContainText(
      "Adding a note without my logs",
      { timeout: 20_000 },
    );
  });
});
