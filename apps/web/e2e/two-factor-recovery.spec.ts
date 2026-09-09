import { expect, test } from "@playwright/test";
import { freshCredentials, register } from "./fixtures/helpers";
import { totpNow } from "./fixtures/totp";

/**
 * Spec 041 US2 (FR-007, FR-008, FR-011, T035): getting back in without the
 * phone, through the screen a person would actually use.
 *
 * # The gap this closes
 *
 * Recovery codes were issued at enrolment, hashed with Argon2, and spendable
 * at `POST /authentication/2fa/verify` from the day they were written. And
 * they were **unreachable**: the sign-in screen's only field was validated
 * against `/^\d{6}$/` and `verifyTwoFactor` took a bare `code`, so the one
 * credential that exists for "my authenticator is gone" could not be typed by
 * the person it exists for. A second factor with no usable recovery is a way
 * to lose an account, which is the thing US2 is written to prevent.
 *
 * Every assertion below is made through the interface, because the server half
 * already had unit tests and the server half was never the problem.
 */

/** Enrol from the account's own screen and come away with the codes. */
async function enrolAndCollectCodes(
  page: import("@playwright/test").Page,
  password: string,
) {
  await page.goto("/settings/security");
  await expect(page.getByTestId("two-factor-enrolment-panel")).toBeVisible({
    timeout: 30_000,
  });

  await page.getByTestId("two-factor-password").fill(password);
  await page.getByTestId("two-factor-begin").click();

  const setupKey = page.getByTestId("two-factor-setup-key");
  await expect(setupKey).toBeVisible({ timeout: 30_000 });
  const secret = ((await setupKey.textContent()) ?? "").replace(/\s+/g, "");
  expect(secret.length).toBeGreaterThan(0);

  await page.getByTestId("two-factor-code").fill(totpNow(secret));
  await page.getByTestId("two-factor-confirm").click();

  const codeCells = page.getByTestId("two-factor-recovery-code");
  await expect(codeCells.first()).toBeVisible({ timeout: 30_000 });
  const codes = await codeCells.allTextContents();
  expect(codes).toHaveLength(10);

  return { secret, codes: codes.map((c) => c.trim()) };
}

/** Sign in as far as the second-factor challenge. */
async function reachTheChallenge(
  page: import("@playwright/test").Page,
  identifier: string,
  password: string,
) {
  await page.goto("/login");
  await page.locator("#login-identifier").fill(identifier);
  await page.locator("#login-password").fill(password);
  await page.getByRole("button", { name: /sign in/i }).click();
  await expect(page.getByTestId("login-two-factor")).toBeVisible({
    timeout: 30_000,
  });
}

test.describe("Spec 041 US2: signing in with a recovery code", () => {
  test("a person who has lost their authenticator can still get in, once per code", async ({
    page,
    browser,
  }) => {
    test.setTimeout(180_000);
    const creds = freshCredentials("e2erecov");
    await register(page, creds);
    const { codes } = await enrolAndCollectCodes(page, creds.password);

    // A separate context: this is somebody signing in fresh, with no session
    // and — the whole premise — no authenticator.
    const visitor = await browser.newContext();
    const visitorPage = await visitor.newPage();
    try {
      await reachTheChallenge(visitorPage, creds.username, creds.password);

      // FR-011. The route is offered on the challenge itself, not hidden
      // behind a support page: somebody reaching for it has already lost
      // their phone and should not also have to hunt.
      const toggle = visitorPage.getByTestId("login-use-recovery-code");
      await expect(toggle).toBeVisible();
      await toggle.click();

      const field = visitorPage.getByTestId("login-recovery-code");
      await expect(field).toBeVisible();
      await expect(
        visitorPage.getByTestId("login-two-factor"),
        "the authenticator field gives way to the recovery one",
      ).toHaveCount(0);

      await field.fill(codes[0]);
      await visitorPage.getByRole("button", { name: /verify/i }).click();

      await visitorPage.waitForURL(
        (url) => !url.pathname.startsWith("/login"),
        {
          timeout: 30_000,
        },
      );

      // FR-008: once. The same code, on a fresh sign-in, must be refused —
      // and this is the assertion that a "spend" which only *checked* the code
      // would fail.
      const second = await browser.newContext();
      const secondPage = await second.newPage();
      try {
        await reachTheChallenge(secondPage, creds.username, creds.password);
        await secondPage.getByTestId("login-use-recovery-code").click();
        await secondPage.getByTestId("login-recovery-code").fill(codes[0]);
        await secondPage.getByRole("button", { name: /verify/i }).click();

        await expect(
          secondPage.getByTestId("login-recovery-code"),
          "a spent recovery code must not admit anybody a second time",
        ).toBeVisible({ timeout: 30_000 });
        expect(new URL(secondPage.url()).pathname).toContain("/login");

        // A *different* code still works, so the refusal above was about that
        // code being spent rather than about recovery codes having stopped
        // working altogether.
        await secondPage.getByTestId("login-recovery-code").fill(codes[1]);
        await secondPage.getByRole("button", { name: /verify/i }).click();
        await secondPage.waitForURL(
          (url) => !url.pathname.startsWith("/login"),
          { timeout: 30_000 },
        );
      } finally {
        await second.close();
      }
    } finally {
      await visitor.close();
    }
  });

  test("the toggle goes both ways and does not carry a stale value across", async ({
    page,
  }) => {
    test.setTimeout(120_000);
    const creds = freshCredentials("e2erecovtog");
    await register(page, creds);
    await enrolAndCollectCodes(page, creds.password);

    await page.context().clearCookies();
    await reachTheChallenge(page, creds.username, creds.password);

    await page.getByTestId("login-two-factor").fill("123456");
    await page.getByTestId("login-use-recovery-code").click();
    await page.getByTestId("login-recovery-code").fill("not-a-real-code");

    // Back again. The authenticator field must be empty rather than holding
    // what was typed before the switch — submitting a value the visible branch
    // no longer owns is how somebody sends the wrong credential without ever
    // seeing it.
    await page.getByTestId("login-use-recovery-code").click();
    await expect(page.getByTestId("login-two-factor")).toHaveValue("");
  });
});
