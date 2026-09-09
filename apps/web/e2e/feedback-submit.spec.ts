import { expect, test } from "@playwright/test";
import { freshCredentials, graphql, register } from "./fixtures/helpers";

/**
 * Spec 037 US1: reporting a problem, through the dialog a person actually
 * uses.
 *
 * # Why this exists alongside `feedback.spec.ts`
 *
 * That file drives the GraphQL mutation. It proves the server's rules — the
 * secret refusal, the rate limiter, cross-account isolation — and it proves
 * none of the things a person meets: that the launcher is on every screen,
 * that the fields are the kind's rather than a superset, that a review step
 * shows what will be sent, and that **a refusal does not throw away what they
 * wrote**.
 *
 * That last one is FR-006 and it is the reason this file is worth its runtime.
 * Losing four paragraphs to a validation error is how a person decides not to
 * report the next thing.
 */

test.describe("Spec 037 US1: reporting a problem", () => {
  test("the launcher is on an ordinary screen, and the kind decides the fields", async ({
    page,
  }) => {
    const creds = freshCredentials("e2efbui");
    await register(page, creds);
    await page.goto("/worlds");

    // FR-001: any screen. `main.tsx` mounts the launcher above the router
    // precisely so this is true of screens nobody thought about.
    const launcher = page.getByTestId("feedback-launcher");
    await expect(launcher).toBeVisible({ timeout: 20_000 });
    await launcher.click();

    const dialog = page.getByTestId("feedback-dialog");
    await expect(dialog).toBeVisible();

    // FR-003: a general message has no title, so there is no title field
    // sitting there disabled.
    await page.getByTestId("feedback-kind-general").click();
    await expect(page.getByTestId("feedback-summary")).toHaveCount(0);

    await page.getByTestId("feedback-kind-issue").click();
    await expect(page.getByTestId("feedback-summary")).toBeVisible();
  });

  test("a refusal keeps every word the person wrote", async ({ page }) => {
    const creds = freshCredentials("e2efbkeep");
    await register(page, creds);
    await page.goto("/worlds");

    // The rate limiter is the refusal a person can actually provoke from this
    // form. A secret in the *message* is not one: the redaction rules apply to
    // captured evidence, not to prose somebody knowingly typed — and the
    // server's own refusal text already promises "what you wrote is kept",
    // which is the sentence this test exists to hold it to.
    //
    // Five through the API, so the sixth — the one that matters — is the only
    // one driven through the form.
    for (let i = 0; i < 5; i += 1) {
      await graphql(
        page,
        `
          mutation Submit($input: SubmitFeedbackInput!) {
            submitFeedback(input: $input) {
              id
            }
          }
        `,
        {
          input: {
            kind: "GENERAL",
            message: `filling the window ${i}`,
            clientVersion: "e2e",
            browser: "Chromium on Linux",
            attachments: [],
          },
        },
      );
    }

    await page.getByTestId("feedback-launcher").click();
    await page.getByTestId("feedback-kind-issue").click();

    const written =
      "Tokens vanish when I switch scenes, and it only happens after the " +
      "third scene. I have tried it on two machines and a fresh profile.";
    await page
      .getByTestId("feedback-summary")
      .fill("Tokens vanish on scene change");
    await page.getByTestId("feedback-message").fill(written);
    await page.getByTestId("feedback-continue").click();

    await expect(page.getByTestId("feedback-review-message")).toBeVisible({
      timeout: 20_000,
    });
    await page.getByTestId("feedback-submit").click();

    // Refused, and said so.
    await expect(page.getByTestId("feedback-failure")).toBeVisible({
      timeout: 20_000,
    });

    // FR-006, the point of this test: going back finds the message intact
    // rather than an empty box. Losing four paragraphs to a refusal is how a
    // person decides not to report the next thing.
    await page.getByTestId("feedback-back").click();
    await expect(page.getByTestId("feedback-message")).toHaveValue(written);
    await expect(page.getByTestId("feedback-summary")).toHaveValue(
      "Tokens vanish on scene change",
    );
  });

  test("a report goes through the review and is acknowledged", async ({
    page,
  }) => {
    const creds = freshCredentials("e2efbsend");
    await register(page, creds);
    await page.goto("/worlds");

    await page.getByTestId("feedback-launcher").click();
    await page.getByTestId("feedback-kind-issue").click();
    await page.getByTestId("feedback-summary").fill("The compendium is slow");
    await page
      .getByTestId("feedback-message")
      .fill("Opening the compendium takes several seconds on my machine.");
    await page.getByTestId("feedback-continue").click();

    // FR-014: what happens to this is on screen while they decide, including
    // whether the destination is public. On this stack nothing is configured,
    // and the notice says so rather than staying silent.
    await expect(page.getByTestId("feedback-destination-notice")).toBeVisible({
      timeout: 20_000,
    });
    await expect(page.getByTestId("feedback-review-message")).toContainText(
      "Opening the compendium",
    );

    await page.getByTestId("feedback-submit").click();
    await expect(page.getByTestId("feedback-sent")).toBeVisible({
      timeout: 20_000,
    });

    // FR-018: it is kept, whatever the destination situation is. Asked of the
    // author's own screen rather than of the database.
    await page.goto("/settings/feedback");
    await expect(page.getByTestId("my-submissions")).toContainText(
      "The compendium is slow",
      { timeout: 20_000 },
    );
  });
});
