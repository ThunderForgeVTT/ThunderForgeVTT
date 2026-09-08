import { test, expect } from "@playwright/test";
import { freshCredentials, register } from "./fixtures/helpers";
import { totpAt } from "./fixtures/totp";

/**
 * Spec 041 US1: turning two-factor on, through the screen a person would use.
 *
 * `two-factor.spec.ts` drives the endpoints, because what it tests is the
 * protocol — the window, the challenge, replay. This drives the interface,
 * because what it tests is that a human being can reach the protocol at all.
 * Until 2026-09-07 they could not: the setup endpoints were called from
 * nowhere in the app, so a capability MVP.md lists as shipped was reachable
 * only by hand-rolled requests.
 */

test.describe("Spec 041 US1: enrolling from the account's own screen", () => {
  test("a person turns two-factor on, mistypes once, and keeps their recovery codes", async ({
    page,
  }) => {
    const creds = freshCredentials("e2e2faui");
    await register(page, creds);

    await page.goto("/settings/security");

    // Nothing is on yet, and the screen says so rather than guessing — which
    // is why `GET /2fa/status` had to exist before this screen could.
    await expect(page.getByTestId("two-factor-enrolment-panel")).toBeVisible({
      timeout: 15_000,
    });

    await page.getByTestId("two-factor-password").fill(creds.password);
    await page.getByTestId("two-factor-begin").click();

    // The typeable key is always present. A desktop authenticator or a
    // password manager has no camera, and FR-002 asks for both forms.
    const setupKey = page.getByTestId("two-factor-setup-key");
    await expect(setupKey).toBeVisible({ timeout: 15_000 });
    const secret = ((await setupKey.textContent()) ?? "").replace(/\s+/g, "");
    expect(secret.length).toBeGreaterThan(0);

    // A wrong code must not cost the enrolment — no re-scanning, no starting
    // again (FR-001c). Chosen so it cannot collide with a real one.
    await page.getByTestId("two-factor-code").fill("000000");
    await page.getByTestId("two-factor-confirm").click();
    await expect(page.getByTestId("field-error").first()).toBeVisible({
      timeout: 15_000,
    });
    await expect(setupKey).toBeVisible();

    await page
      .getByTestId("two-factor-code")
      .fill(totpAt(secret, Math.floor(Date.now() / 1000)));
    await page.getByTestId("two-factor-confirm").click();

    // Shown once, and said to be shown once.
    const codes = page.getByTestId("two-factor-recovery-code");
    await expect(codes.first()).toBeVisible({ timeout: 15_000 });
    expect(await codes.count()).toBe(10);

    // And the factor is now genuinely in force: signing in again is stopped
    // at the second step. That is the claim the whole screen exists to make.
    //
    // The CSRF token is echoed the way the page's own code does — a
    // double-submit check compares the `csrf_token` cookie against the header,
    // so a request without it is refused 403 before the credentials are ever
    // considered, which would look exactly like a passing test for the wrong
    // reason if the expectation were "not 200".
    const csrf = (await page.context().cookies()).find(
      (cookie) => cookie.name === "csrf_token",
    )?.value;
    const challenge = await page.request.post("/api/authentication/login", {
      headers: csrf ? { "x-csrf-token": csrf } : {},
      data: { identifier: creds.username, password: creds.password },
    });
    expect(challenge.status()).toBe(401);
    expect(await challenge.text()).toContain("two_factor_required");
  });
});
