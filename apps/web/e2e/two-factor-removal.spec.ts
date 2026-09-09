import { expect, test } from "@playwright/test";
import { freshCredentials, login, register } from "./fixtures/helpers";
import { totpAt } from "./fixtures/totp";

/**
 * Spec 041 US4 (FR-012, FR-014): turning a second factor off, deliberately.
 *
 * # The defect this closes
 *
 * There was no way off. Beginning a *new* enrolment cleared the old factor as
 * a side effect, so the only route out ran through the button that turns it
 * on — and an account that abandoned an enrolment silently lost the factor it
 * already had, having been asked for nothing.
 *
 * # Why the price matters more than the button
 *
 * The assertions worth having here are the refusals. A removal control that
 * works is easy; one that cannot be driven from a session somebody left open
 * is the requirement. So this proves the password alone is not enough and a
 * wrong code is not enough, before it proves the happy path at all.
 */

async function enrol(page: import("@playwright/test").Page, password: string) {
  await page.goto("/settings/security");
  await expect(page.getByTestId("two-factor-enrolment-panel")).toBeVisible({
    timeout: 15_000,
  });
  await page.getByTestId("two-factor-password").fill(password);
  await page.getByTestId("two-factor-begin").click();

  const setupKey = page.getByTestId("two-factor-setup-key");
  await expect(setupKey).toBeVisible({ timeout: 15_000 });
  const secret = ((await setupKey.textContent()) ?? "").replace(/\s+/g, "");
  expect(secret.length).toBeGreaterThan(0);

  await page
    .getByTestId("two-factor-code")
    .fill(totpAt(secret, Math.floor(Date.now() / 1000)));
  await page.getByTestId("two-factor-confirm").click();
  await expect(
    page.getByTestId("two-factor-recovery-code").first(),
  ).toBeVisible({ timeout: 15_000 });

  return secret;
}

test.describe("Spec 041 US4: turning a second factor off", () => {
  test("costs a password and a code, and refuses either one alone", async ({
    page,
  }) => {
    const creds = freshCredentials("e2e2faoff");
    await register(page, creds);
    const secret = await enrol(page, creds.password);

    await page.goto("/settings/security");
    await page.getByTestId("two-factor-removal-open").click();

    // The password alone. This is the whole of FR-012: a session left open on
    // a machine that changed hands must not be able to make every future
    // sign-in cheaper.
    await page.getByTestId("two-factor-removal-password").fill(creds.password);
    await page.getByTestId("two-factor-removal-code").fill("000000");
    await page.getByTestId("two-factor-removal-submit").click();
    await expect(page.getByTestId("two-factor-removal-error")).toBeVisible({
      timeout: 15_000,
    });

    // The fields survive the refusal — a refusal here is often something
    // retyping cannot fix, and clearing the form would just hide that.
    await expect(page.getByTestId("two-factor-removal-password")).toHaveValue(
      creds.password,
    );

    // A correct code with the wrong password.
    await page
      .getByTestId("two-factor-removal-password")
      .fill("not-the-password");
    await page
      .getByTestId("two-factor-removal-code")
      .fill(totpAt(secret, Math.floor(Date.now() / 1000)));
    await page.getByTestId("two-factor-removal-submit").click();
    await expect(page.getByTestId("two-factor-removal-error")).toBeVisible({
      timeout: 15_000,
    });

    // Both, correctly.
    await page.getByTestId("two-factor-removal-password").fill(creds.password);
    await page
      .getByTestId("two-factor-removal-code")
      .fill(totpAt(secret, Math.floor(Date.now() / 1000) + 30));
    await page.getByTestId("two-factor-removal-submit").click();
    await expect(page.getByTestId("two-factor-removed")).toBeVisible({
      timeout: 15_000,
    });
  });

  test("and the factor really is gone: signing in no longer asks for a code", async ({
    page,
    context,
  }) => {
    const creds = freshCredentials("e2e2fagone");
    await register(page, creds);
    const secret = await enrol(page, creds.password);

    await page.goto("/settings/security");
    await page.getByTestId("two-factor-removal-open").click();
    await page.getByTestId("two-factor-removal-password").fill(creds.password);
    await page
      .getByTestId("two-factor-removal-code")
      .fill(totpAt(secret, Math.floor(Date.now() / 1000)));
    await page.getByTestId("two-factor-removal-submit").click();
    await expect(page.getByTestId("two-factor-removed")).toBeVisible({
      timeout: 15_000,
    });

    // The claim the whole feature makes, driven the way a person drives it.
    //
    // Deliberately not a bare `fetch` at the session endpoint: the CSRF check
    // is a double submit, so a request made after clearing cookies is refused
    // 403 **with an empty body** before the credentials are ever considered —
    // which reads as "sign-in failed" and would have this test passing or
    // failing for a reason unrelated to two-factor. Signing in through the
    // form gets the token the way the application gets it.
    await context.clearCookies();
    await login(page, creds.username, creds.password);

    // Arriving anywhere signed-in is the proof: an account still carrying a
    // factor is held at `/login` for its code and never reaches these.
    await page.waitForURL(/\/(welcome|admin|worlds)/, { timeout: 20_000 });
  });
});
