import { expect, test, type Page } from "@playwright/test";
import { openAdminPage } from "./fixtures/admin";

/**
 * Terms disputes and privacy requests, from a stranger's form to the
 * operator's queue.
 *
 * # The seam this is aimed at
 *
 * The form posts to `/api/graphql/public` and the queue reads
 * `/api/graphql`. Those are two transports with two different auth rules, and
 * the last time this product got that pairing wrong the symptom was invisible:
 * `publishedOperatorValues` went to the authenticated endpoint, answered 401,
 * and the legal pages fell back to their unset markers — which is *correct*
 * behaviour for an unconfigured instance, so a configured one looked identical
 * to one nobody had touched. A test that only submitted, or only read, would
 * miss exactly that. So this submits with no account at all and then reads the
 * result back as an administrator.
 *
 * # Why it submits signed out, deliberately
 *
 * Somebody disputing the terms of service is frequently disputing the terms
 * they were asked to accept. If this ever comes to require an account, the
 * feature is broken for the people it exists for, and this is the assertion
 * that would fail.
 */

test.describe.configure({ mode: "serial" });

const SUBJECT = `Clause 4 is unreasonable ${Date.now()}`;
const PRIVACY_SUBJECT = `What do you hold about me ${Date.now()}`;

let admin: Page;

test.afterAll(async () => {
  if (admin) await admin.context().close();
});

test.describe("Legal enquiries", () => {
  test.use({ storageState: { cookies: [], origins: [] } });

  test("a stranger files a terms dispute without an account", async ({
    page,
  }) => {
    await page.goto("/legal/terms");

    const form = page.getByTestId("legal-enquiry-terms");
    await expect(form).toBeVisible({ timeout: 20_000 });

    // No address is published on this page — that is the point of the form.
    // The copyright page is the exception and has its own test below.
    await expect(page.locator("body")).not.toContainText("@thunderforge");

    await page
      .getByTestId("legal-enquiry-terms-name")
      .fill("A Concerned Person");
    await page
      .getByTestId("legal-enquiry-terms-contact")
      .fill("concerned@thunderforge-e2e.example.org");
    await page.getByTestId("legal-enquiry-terms-subject").fill(SUBJECT);
    await page
      .getByTestId("legal-enquiry-terms-body")
      .fill(
        "Clause 4 appears to contradict clause 9, and I would like it explained.",
      );
    await page.getByTestId("legal-enquiry-terms-submit").click();

    await expect(page.getByTestId("legal-enquiry-terms-receipt")).toBeVisible({
      timeout: 20_000,
    });
  });

  test("an incomplete submission is refused and says what is missing", async ({
    page,
  }) => {
    await page.goto("/legal/privacy");
    await page.getByTestId("legal-enquiry-privacy-name").fill("A Person");
    // Contact left blank on purpose: an enquiry nobody can reply to cannot be
    // resolved, and a privacy request in particular obliges an answer.
    await page.getByTestId("legal-enquiry-privacy-subject").fill("A subject");
    await page.getByTestId("legal-enquiry-privacy-body").fill("A body");
    await page.getByTestId("legal-enquiry-privacy-submit").click();

    const error = page.getByTestId("legal-enquiry-privacy-error");
    await expect(error).toBeVisible({ timeout: 20_000 });
    await expect(error).toContainText("email address");

    // Refused, and the typed fields survive it. Retyping after a validation
    // message is the fastest way to make somebody give up on complaining.
    await expect(page.getByTestId("legal-enquiry-privacy-name")).toHaveValue(
      "A Person",
    );
  });

  test("a stranger files a privacy request", async ({ page }) => {
    await page.goto("/legal/privacy");
    await page.getByTestId("legal-enquiry-privacy-name").fill("A Data Subject");
    await page
      .getByTestId("legal-enquiry-privacy-contact")
      .fill("subject@thunderforge-e2e.example.org");
    await page
      .getByTestId("legal-enquiry-privacy-subject")
      .fill(PRIVACY_SUBJECT);
    await page
      .getByTestId("legal-enquiry-privacy-body")
      .fill("Please tell me what this instance stores about my account.");
    await page.getByTestId("legal-enquiry-privacy-submit").click();

    await expect(page.getByTestId("legal-enquiry-privacy-receipt")).toBeVisible(
      {
        timeout: 20_000,
      },
    );
  });

  test("the copyright page keeps its published agent, because the statute requires it", async ({
    page,
  }) => {
    await page.goto("/legal/dmca");
    // The form comes first — nearly everybody here wants to report something.
    const designation = page.getByTestId("dmca-agent-designation");
    await expect(designation).toBeVisible({ timeout: 20_000 });
    await expect(designation).toContainText("512(c)(2)");
    // And it is still a full designation, not a form in its place. Losing the
    // safe harbour to save an operator some spam is not a trade worth making.
    await expect(designation).toContainText("Mailing Address");
    await expect(designation).toContainText("Electronic Contact");
  });
});

test.describe("The operator's queue", () => {
  test("both enquiries arrive, grouped by kind, and can be worked", async ({
    browser,
  }) => {
    admin = await openAdminPage(browser);
    await admin.goto("/admin/legal");

    const panel = admin.getByTestId("legal-enquiries-panel");
    await expect(panel).toBeVisible({ timeout: 20_000 });
    await expect(panel).toContainText(SUBJECT);
    await expect(panel).toContainText(PRIVACY_SUBJECT);

    // Takedowns are not listed here; they are moderation cases. The panel says
    // where they live rather than showing a second, weaker view of them.
    await expect(panel).toContainText("Moderation");

    await admin.getByTestId("legal-enquiries-tab-TERMS").click();
    await expect(panel).toContainText(SUBJECT);
    await expect(panel).not.toContainText(PRIVACY_SUBJECT);

    // Work one, and confirm the state actually moved rather than the button
    // merely having been clickable.
    const row = panel.locator('[data-testid^="legal-enquiry-"]').first();
    await expect(row).toHaveAttribute("data-status", "open");
    const rowId = await row.getAttribute("data-testid");
    await row.getByRole("button", { name: "Close" }).click();

    // Closing takes it out of the working queue — that is what a queue is for,
    // and what "Show closed" exists to undo. Asserting it merely changed
    // colour in place would have described a different product.
    await expect(panel.locator(`[data-testid="${rowId}"]`)).toHaveCount(0, {
      timeout: 20_000,
    });

    await admin.getByTestId("legal-enquiries-toggle-closed").click();
    await expect(panel.locator(`[data-testid="${rowId}"]`)).toHaveAttribute(
      "data-status",
      "closed",
      { timeout: 20_000 },
    );
  });
});
