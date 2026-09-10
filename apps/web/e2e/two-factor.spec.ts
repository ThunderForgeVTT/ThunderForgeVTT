import { expect, test, type Page } from "@playwright/test";
import { createHmac } from "node:crypto";
import { ADMIN_USER } from "./fixtures/global-setup";
import {
  freshCredentials,
  graphql,
  register,
  type Credentials,
} from "./fixtures/helpers";

/**
 * Two-factor authentication, end to end: enrolment, the login challenge, a
 * correct code, a refused one, the instance-wide policy switch, and the
 * (missing) way back off.
 *
 * # Why this file exists
 *
 * MVP.md Phase 1 lists two-factor as shipped, next to username/password,
 * OAuth, sessions and admin bootstrap. Every one of those has been driven in a
 * browser by some spec in this directory; two-factor had never been. The
 * server's own tests cover the arithmetic — `thunderforge-axum-auth-core`'s
 * `totp` module proves the skew window either side of a step — but nothing
 * proved a person holding an authenticator can actually get in, or that
 * someone without one is actually stopped.
 *
 * # What the product surface actually is
 *
 * Enrolment is **API only**. There is no page anywhere in `apps/web/src` that
 * calls `/authentication/2fa/setup/start` — the only 2FA code the SPA contains
 * is `LoginView`'s challenge step and the admin policy switch. So these tests
 * enrol over `page.request` (the same browser context, the same cookies) and
 * then drive the browser for the half a person can reach. That is not the test
 * taking a shortcut past a UI: the UI does not exist. See the report at the
 * bottom of this comment.
 *
 * The pieces, read out of `src/server/src/auth/two_factor.rs`:
 *
 *   - `POST /authentication/2fa/setup/start` — username + password *or* a
 *     `challenge_id` from the login response, no session needed. Generates 20
 *     random bytes, stores them encrypted, and
 *     hands back `otpauth://totp/ThunderForge:<user>?secret=<base32>&issuer=…`.
 *     The secret in that URI is **unpadded base32**, and that is what the
 *     codes below are computed from.
 *   - `POST /authentication/2fa/setup/confirm` — one correct code flips
 *     `two_factor_enabled` to true.
 *   - `POST /authentication/login` — answers `401 two_factor_required` with a
 *     `login_two_factor_challenge_id` when the account holds a second factor,
 *     and `401 two_factor_enrolment_required` with one when the instance (or
 *     an administrator) requires a factor the account has not got. Neither is
 *     a session, and neither is a refusal.
 *   - `POST /authentication/2fa/verify` — challenge id + code, and the session
 *     cookie is issued here. The challenge is consumed on success and expires
 *     after ten minutes.
 *   - `updateTwoFactorPolicy(requiredForAllUsers:)` — the admin GraphQL
 *     mutation behind /admin/security, which makes the challenge unconditional.
 *
 * # Known defects this file records rather than papers over
 *
 *   1. **There is still no way to switch two-factor off deliberately.**
 *      A deliberate removal — password **and** possession — now exists at
 *      `POST /authentication/2fa/disable` (FR-012, FR-014), and its cases live
 *      in `two-factor-removal.spec.ts` rather than being repeated here: this
 *      file is about the enrolment and the challenge, and that one is about
 *      the way off.
 *
 *      What *was* here — `setup/start` clearing the confirmed factor on a
 *      password alone, so the only route off was also a route round — is
 *      fixed as of 2026-09-07 (ADR-081). The final test asserted the correct
 *      behaviour while it was still broken and now passes unchanged.
 *   2. ~~Turning the instance policy on locks out every account that has not
 *      enrolled.~~ **Fixed on 2026-09-07 (spec 041 FR-019).** It did: they
 *      were handed a *verification* challenge, they had no secret, and
 *      `verify_two_factor_for_user` answers `false` for a user with no stored
 *      secret — a refusal with no way to fix it from the screen that issued
 *      it.
 *
 *      What the server does now is answer `401 two_factor_enrolment_required`
 *      (rather than `two_factor_required`) with the same
 *      `login_two_factor_challenge_id`, and that challenge authorises the
 *      enrolment that follows: `POST /authentication/2fa/setup/start` and
 *      `.../confirm` take **either** a username and password **or** a
 *      `challenge_id`. Confirmation spends the challenge, issues the session
 *      cookie and returns `signed_in: true`, so the interrupted sign-in
 *      finishes where it was going (FR-020). `LoginView` renders the shared
 *      enrolment steps in place, so the challenge screen now *does* offer a
 *      way to enrol from there.
 *
 *      The ticket is fenced to the case that needs it: a challenge for an
 *      account that already holds a confirmed factor is a challenge to verify
 *      it, and is refused as an enrolment authorisation.
 *
 *      The policy test below now walks the whole of it: the enrolment card
 *      appears, the code is computed from the typeable secret *the card
 *      shows*, the recovery codes are on the screen at the one moment they are
 *      free, and the sign-in that was interrupted finishes where it was going
 *      (FR-019, FR-020). Turning the policy back off then gets a
 *      *verification* challenge rather than none, which is FR-022: the factor
 *      never depended on the policy.
 *   3. ~~A code is not bound to the step it was minted for, so the same six
 *      digits verify against a *new* challenge for as long as the skew window
 *      lasts.~~ **Fixed on 2026-09-09 (FR-016).**
 *      `matched_step` returns which step matched and a conditional
 *      `UPDATE … WHERE two_factor_last_used_step IS NULL OR < $step` claims
 *      it, so a step is spent once and the previous step's still-valid code
 *      cannot be replayed either. Asserted below — a replayed code is refused
 *      with the same message as a wrong one, because saying which would
 *      confirm to an attacker that the code they intercepted was genuine.
 */

/** Seconds per code — `STEP_SECONDS` in `crates/thunderforge-axum-auth-core`. */
const STEP_SECONDS = 30;

/**
 * How far either side of "now" the server accepts a code (`SKEW_STEPS`).
 *
 * Used here to pick a code that is wrong *and stays wrong* while the request
 * is in flight, rather than trusting that "000000" happens not to be valid.
 */
const SKEW_STEPS = 1;

const BASE32_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/**
 * RFC 4648 base32, unpadded — the encoding `two_factor_setup_start` puts in
 * the `otpauth://` URI (`BASE32_NOPAD.encode`).
 *
 * Written out rather than pulled from npm on purpose: adding a dependency to
 * the web app so that a test can do six lines of bit-shifting is a worse trade
 * than the six lines, and an authenticator implementation is exactly the kind
 * of thing a test should not share with the code under test anyway.
 */
function decodeBase32(secret: string): Uint8Array {
  const bytes: number[] = [];
  let buffer = 0;
  let bits = 0;

  for (const character of secret.replace(/=+$/, "").toUpperCase()) {
    const index = BASE32_ALPHABET.indexOf(character);
    if (index < 0) {
      throw new Error(`Not base32: ${JSON.stringify(secret)}`);
    }
    buffer = (buffer << 5) | index;
    bits += 5;
    if (bits >= 8) {
      bits -= 8;
      bytes.push((buffer >>> bits) & 0xff);
    }
  }

  return Uint8Array.from(bytes);
}

/**
 * The six digits an authenticator app would show for `secretBase32` at
 * `unixSeconds`. RFC 6238 with the server's parameters: HMAC-SHA1, 30-second
 * step, 6 digits, dynamic truncation.
 */
function totpCodeAt(secretBase32: string, unixSeconds: number): string {
  const counter = new Uint8Array(8);
  new DataView(counter.buffer).setBigUint64(
    0,
    BigInt(Math.floor(unixSeconds / STEP_SECONDS)),
  );

  const digest = createHmac("sha1", decodeBase32(secretBase32))
    .update(counter)
    .digest();
  const offset = digest[digest.length - 1] & 0x0f;
  const truncated =
    ((digest[offset] & 0x7f) << 24) |
    ((digest[offset + 1] & 0xff) << 16) |
    ((digest[offset + 2] & 0xff) << 8) |
    (digest[offset + 3] & 0xff);

  return (truncated % 1_000_000).toString().padStart(6, "0");
}

/** The code that is valid right now. */
function currentTotpCode(secretBase32: string): string {
  return totpCodeAt(secretBase32, Date.now() / 1000);
}

/**
 * A code that is valid for no step the server would accept.
 *
 * Six digits collide once in a million, and "once in a million" across a suite
 * that runs for an hour is a flake nobody can reproduce. So this walks
 * candidates until it finds one that matches none of the steps inside the
 * acceptance window, with a step of margin either side for the time the
 * request spends in flight.
 */
function aCodeThatIsNeverValidNow(secretBase32: string): string {
  const now = Date.now() / 1000;
  const valid = new Set<string>();
  for (let step = -SKEW_STEPS - 2; step <= SKEW_STEPS + 2; step += 1) {
    valid.add(totpCodeAt(secretBase32, now + step * STEP_SECONDS));
  }

  for (let candidate = 0; candidate < 1000; candidate += 1) {
    const code = candidate.toString().padStart(6, "0");
    if (!valid.has(code)) {
      return code;
    }
  }

  throw new Error("Could not find a code outside the acceptance window");
}

/**
 * Headers for a state-changing call made as the browser.
 *
 * `require_csrf_for_session` only demands the double-submit token when a
 * session cookie is present, which is exactly the enrolment case — the person
 * is signed in while they enrol. Echoing the cookie into the header is what
 * the SPA's own `withCsrf` does.
 */
async function jsonHeaders(page: Page): Promise<Record<string, string>> {
  const csrfToken = (await page.context().cookies()).find(
    (cookie) => cookie.name === "csrf_token",
  )?.value;

  return {
    "Content-Type": "application/json",
    ...(csrfToken ? { "x-csrf-token": csrfToken } : {}),
  };
}

interface AuthPayload {
  status?: string;
  message?: string;
  login_two_factor_challenge_id?: string | null;
  otpauth_url?: string | null;
  session?: { authenticated: boolean } | null;
}

interface AuthCall {
  status: number;
  body: AuthPayload;
}

async function postAuth(
  page: Page,
  path: string,
  data: Record<string, unknown>,
): Promise<AuthCall> {
  const response = await page.request.post(`/api/authentication/${path}`, {
    headers: await jsonHeaders(page),
    data,
  });
  const text = await response.text();

  let body: AuthPayload;
  try {
    body = JSON.parse(text) as AuthPayload;
  } catch {
    throw new Error(
      `Non-JSON reply from /api/authentication/${path} (status ${response.status()}): ${text.slice(0, 300)}`,
    );
  }

  return { status: response.status(), body };
}

/** The secret out of an `otpauth://` URI, which is where enrolment hands it over. */
function secretFromOtpauthUrl(otpauthUrl: string): string {
  const secret = new URL(otpauthUrl).searchParams.get("secret");
  if (!secret) {
    throw new Error(`No secret in the enrolment URI: ${otpauthUrl}`);
  }
  return secret;
}

/**
 * Enrol `creds` in two-factor and return the secret, as an authenticator app
 * would hold it. `page` must be signed in as that account.
 */
async function enrolTwoFactor(page: Page, creds: Credentials): Promise<string> {
  const start = await postAuth(page, "2fa/setup/start", {
    username: creds.username,
    password: creds.password,
  });
  expect(start.status, start.body.message).toBe(200);
  expect(start.body.otpauth_url).toBeTruthy();

  const secret = secretFromOtpauthUrl(start.body.otpauth_url ?? "");
  const confirm = await postAuth(page, "2fa/setup/confirm", {
    username: creds.username,
    password: creds.password,
    code: currentTotpCode(secret),
  });
  expect(confirm.status, confirm.body.message).toBe(200);
  expect(confirm.body.status).toBe("success");

  return secret;
}

async function logout(page: Page): Promise<void> {
  const response = await page.request.post("/api/authentication/logout", {
    headers: await jsonHeaders(page),
  });
  expect(response.status()).toBe(200);
}

/** Is this browser context carrying a usable session? */
async function isSignedIn(page: Page): Promise<boolean> {
  const response = await page.request.get("/api/authentication/session");
  return response.status() === 200;
}

/** Fills the credential step of /login and submits it. */
async function submitCredentials(
  page: Page,
  identifier: string,
  password: string,
): Promise<void> {
  await page.goto("/login");
  await page.locator("#login-identifier").fill(identifier);
  await page.locator("#login-password").fill(password);
  await page.getByRole("button", { name: /^sign in$/i }).click();
}

/** The challenge card `LoginView` renders as its second step. */
function twoFactorField(page: Page) {
  return page.locator("#login-two-factor");
}

test.describe("two-factor enrolment", () => {
  test("a password proves who you are, and enrolment hands back a real authenticator secret", async ({
    page,
  }) => {
    const creds = freshCredentials("e2e2fa");
    await register(page, creds);

    // The wrong password must not mint a secret for somebody else's account:
    // `setup/start` is unauthenticated, so the password is the *only* thing
    // standing between a stranger and resetting an account's second factor.
    const impostor = await postAuth(page, "2fa/setup/start", {
      username: creds.username,
      password: "not-the-password",
    });
    expect(impostor.status).toBe(401);
    expect(impostor.body.otpauth_url ?? null).toBeNull();

    const start = await postAuth(page, "2fa/setup/start", {
      username: creds.username,
      password: creds.password,
    });
    expect(start.status, start.body.message).toBe(200);

    const otpauthUrl = start.body.otpauth_url ?? "";
    // The URI an authenticator app scans. Issuer and account name are part of
    // it because they are what the app labels the entry with — a code that
    // verifies against a row nobody can find in their phone is not usable 2FA.
    expect(otpauthUrl).toMatch(
      new RegExp(
        `^otpauth://totp/ThunderForge:${creds.username}\\?secret=[A-Z2-7]+&issuer=ThunderForge$`,
      ),
    );

    const secret = secretFromOtpauthUrl(otpauthUrl);
    // 20 bytes, the HMAC-SHA1 block size RFC 4226 asks for. A short secret
    // would still "work" in every functional sense, which is why it is checked
    // here rather than assumed.
    expect(decodeBase32(secret)).toHaveLength(20);

    // Enrolment is not finished until a code proves the person actually holds
    // the secret. A wrong one must not enable anything.
    const wrong = await postAuth(page, "2fa/setup/confirm", {
      username: creds.username,
      password: creds.password,
      code: aCodeThatIsNeverValidNow(secret),
    });
    expect(wrong.status).toBe(401);
    expect(wrong.body.status).toBe("two_factor_invalid");

    const confirmed = await postAuth(page, "2fa/setup/confirm", {
      username: creds.username,
      password: creds.password,
      code: currentTotpCode(secret),
    });
    expect(confirmed.status, confirmed.body.message).toBe(200);
    expect(confirmed.body.status).toBe("success");

    // And it took effect: the very next sign-in is no longer password-only.
    await logout(page);
    const attempt = await postAuth(page, "login", {
      identifier: creds.username,
      password: creds.password,
    });
    expect(attempt.status).toBe(401);
    expect(attempt.body.status).toBe("two_factor_required");
    expect(attempt.body.login_two_factor_challenge_id).toBeTruthy();
  });
});

test.describe("the login challenge", () => {
  test("an enrolled account is stopped at the second step, and the right code lets it through", async ({
    page,
  }) => {
    const creds = freshCredentials("e2e2fa");
    await register(page, creds);
    const secret = await enrolTwoFactor(page, creds);
    await logout(page);

    await submitCredentials(page, creds.username, creds.password);

    // The whole point: correct credentials alone do not sign anyone in.
    await expect(twoFactorField(page)).toBeVisible({ timeout: 15_000 });
    await expect(
      page.getByRole("heading", { name: "Two-factor code" }),
    ).toBeVisible();
    await expect(page).toHaveURL(/\/login/);
    expect(
      await isSignedIn(page),
      "no session may exist while the second factor is outstanding",
    ).toBe(false);
    // The credential fields are locked while the challenge stands, so the
    // second factor cannot be answered for a different account than the one
    // the challenge was issued for.
    await expect(page.locator("#login-identifier")).toBeDisabled();

    await twoFactorField(page).fill(currentTotpCode(secret));
    await page.getByRole("button", { name: /^verify$/i }).click();

    // A fresh account owns no world, so /welcome bounces it straight on to
    // world creation — either destination means the session exists.
    await page.waitForURL(/\/(welcome|worlds\/create)$/, { timeout: 20_000 });
    expect(
      await isSignedIn(page),
      "the verified code should have issued a session",
    ).toBe(true);
  });

  test("a wrong code is refused, says so, and does not burn the challenge", async ({
    page,
  }) => {
    const creds = freshCredentials("e2e2fa");
    await register(page, creds);
    const secret = await enrolTwoFactor(page, creds);
    await logout(page);

    await submitCredentials(page, creds.username, creds.password);
    await expect(twoFactorField(page)).toBeVisible({ timeout: 15_000 });

    await twoFactorField(page).fill(aCodeThatIsNeverValidNow(secret));
    await page.getByRole("button", { name: /^verify$/i }).click();

    // Told, in as many words, rather than left on a page that silently did
    // nothing — and still on /login with no session.
    await expect(page.getByText("That code was not accepted.")).toBeVisible({
      timeout: 15_000,
    });
    await expect(page).toHaveURL(/\/login/);
    expect(
      await isSignedIn(page),
      "a refused code must not leave a session behind",
    ).toBe(false);

    // A mistyped code is not a lockout: the same challenge still accepts the
    // right one. (If it did not, a fat-fingered digit would cost a whole
    // round trip through the credential step for no security gain.)
    await twoFactorField(page).fill(currentTotpCode(secret));
    await page.getByRole("button", { name: /^verify$/i }).click();
    await page.waitForURL(/\/(welcome|worlds\/create)$/, { timeout: 20_000 });
    expect(await isSignedIn(page)).toBe(true);
  });

  test("a stale code is refused, and a spent challenge cannot be replayed", async ({
    page,
  }) => {
    const creds = freshCredentials("e2e2fa");
    await register(page, creds);
    const secret = await enrolTwoFactor(page, creds);
    await logout(page);

    // A code from ten minutes ago is a code somebody read over a shoulder, or
    // out of a screenshot. It is outside the ±1 step window and must not work.
    const stale = await postAuth(page, "login", {
      identifier: creds.username,
      password: creds.password,
    });
    expect(stale.body.status).toBe("two_factor_required");
    const staleAttempt = await postAuth(page, "2fa/verify", {
      challenge_id: stale.body.login_two_factor_challenge_id,
      code: totpCodeAt(secret, Date.now() / 1000 - 600),
    });
    expect(staleAttempt.status).toBe(401);
    expect(staleAttempt.body.status).toBe("two_factor_invalid");
    expect(await isSignedIn(page)).toBe(false);

    const challenge = await postAuth(page, "login", {
      identifier: creds.username,
      password: creds.password,
    });
    expect(challenge.status).toBe(401);
    const challengeId = challenge.body.login_two_factor_challenge_id;
    expect(challengeId).toBeTruthy();

    const code = currentTotpCode(secret);
    const verified = await postAuth(page, "2fa/verify", {
      challenge_id: challengeId,
      code,
    });
    expect(verified.status, verified.body.message).toBe(200);
    expect(await isSignedIn(page)).toBe(true);

    // The same challenge and the same code, a second time. The challenge was
    // consumed on success, so this is the replay the `consumed_at` column
    // exists to stop — and it is stopped by the challenge, not by the code.
    const replay = await postAuth(page, "2fa/verify", {
      challenge_id: challengeId,
      code,
    });
    expect(replay.status).toBe(400);
    expect(replay.body.status).toBe("two_factor_challenge_invalid");
  });

  /**
   * SC-005 / FR-016: **the same six digits, a fresh challenge, inside the
   * window.**
   *
   * This is the replay the consumed challenge does *not* stop, and it is the
   * one that matters: a code is valid for thirty seconds either side of its
   * step, so somebody who reads it over a shoulder or out of a screenshot has
   * up to ninety seconds to start their own sign-in with it. The challenge is
   * new, so `consumed_at` has nothing to say — the refusal has to come from the
   * *code*, which is why `two_factor_last_used_step` exists and why claiming a
   * step is a conditional `UPDATE` rather than a read followed by a write.
   *
   * The refusal is deliberately the same one a wrong code gets. Telling the
   * caller "that code was right but already used" would confirm that the code
   * they intercepted was genuine, which is precisely the fact worth hiding
   * from the only person who would ever see this message.
   */
  test("the same code cannot be used twice, even on a new challenge inside its window", async ({
    page,
  }) => {
    const creds = freshCredentials("e2e2fa");
    await register(page, creds);
    const secret = await enrolTwoFactor(page, creds);
    await logout(page);

    const code = currentTotpCode(secret);

    const first = await postAuth(page, "login", {
      identifier: creds.username,
      password: creds.password,
    });
    expect(first.body.status).toBe("two_factor_required");
    const spent = await postAuth(page, "2fa/verify", {
      challenge_id: first.body.login_two_factor_challenge_id,
      code,
    });
    expect(spent.status, spent.body.message).toBe(200);
    expect(await isSignedIn(page)).toBe(true);

    // A second, entirely separate sign-in attempt — its own challenge, never
    // consumed — offering the code that has just been spent.
    await logout(page);
    const second = await postAuth(page, "login", {
      identifier: creds.username,
      password: creds.password,
    });
    expect(second.body.status).toBe("two_factor_required");
    const challengeId = second.body.login_two_factor_challenge_id;
    expect(
      challengeId,
      "the second attempt must get its own challenge, or this proves nothing",
    ).not.toBe(first.body.login_two_factor_challenge_id);

    const replayed = await postAuth(page, "2fa/verify", {
      challenge_id: challengeId,
      code,
    });
    expect(
      replayed.status,
      "a code already spent must be refused even against a challenge that was never used",
    ).toBe(401);
    expect(
      replayed.body.status,
      "and refused as an invalid code — not as a used one, which would confirm it was genuine",
    ).toBe("two_factor_invalid");
    expect(await isSignedIn(page)).toBe(false);
  });
});

test.describe("the instance-wide policy", () => {
  /**
   * Puts the instance back to "not required for everyone".
   *
   * Unconditional, and signing in again if the page lost its session, for the
   * same reason `instance-access-gate.spec.ts` restores the access policy: a
   * shard that inherited a mandatory-2FA instance would fail in every other
   * spec at once, with an error about codes rather than about itself.
   */
  test.afterEach(async ({ page }) => {
    try {
      await setPolicy(page, false);
    } catch {
      await signInAsAdmin(page);
      await setPolicy(page, false);
    }
  });

  async function setPolicy(page: Page, required: boolean): Promise<void> {
    const result = await graphql<{
      data?: {
        updateTwoFactorPolicy?: { twoFactorRequiredForAllUsers: boolean };
      };
      errors?: { message: string }[];
    }>(
      page,
      `
        mutation SetTwoFactorPolicy($required: Boolean!) {
          updateTwoFactorPolicy(requiredForAllUsers: $required) {
            twoFactorRequiredForAllUsers
          }
        }
      `,
      { required },
    );

    if (
      result.data?.updateTwoFactorPolicy?.twoFactorRequiredForAllUsers !==
      required
    ) {
      throw new Error(
        `Could not set the 2FA policy: ${JSON.stringify(result.errors ?? result)}`,
      );
    }
  }

  /** `redirectAfterLogin` sends an admin to /admin, so wait for either. */
  async function signInAsAdmin(page: Page): Promise<void> {
    await submitCredentials(page, ADMIN_USER.identifier, ADMIN_USER.password);
    await page.waitForURL(/\/(admin|welcome)$/, { timeout: 20_000 });
  }

  test("switching it on challenges an account that never enrolled, and switching it off releases them", async ({
    page,
    browser,
  }) => {
    test.setTimeout(180_000);

    // Registered before the policy changes, and in its own context: this is
    // the ordinary user who has done nothing with 2FA at all.
    const creds = freshCredentials("e2e2fapolicy");
    const userContext = await browser.newContext();
    const user = await userContext.newPage();

    try {
      await register(user, creds);
      await logout(user);

      await signInAsAdmin(page);
      await page.goto("/admin/security");
      await expect(
        page.getByRole("heading", { name: "Two-factor enforcement" }),
      ).toBeVisible({ timeout: 20_000 });

      // Through the switch an administrator actually has, rather than the
      // mutation behind it — the claim being tested is that the admin surface
      // enforces the policy, and a mutation call would skip the half of that
      // sentence this file is here to cover.
      // Scoped to the section rather than the page: the admin shell has
      // other switches (the OAuth provider forms), and a bare role lookup would
      // be a strict-mode violation the day one of them shares a screen.
      const enforcementSwitch = page.locator("#security").getByRole("switch");
      // Asserted rather than toggled blindly: the switch is a toggle, so a
      // run that inherited an already-enforcing instance would turn it *off*
      // here and then "prove" enforcement against a policy that is not on.
      await expect(enforcementSwitch).not.toBeChecked();
      await enforcementSwitch.click();
      await page
        .getByRole("button", { name: /update security policy/i })
        .click();
      await expect(
        page.getByText("2FA enforcement policy updated."),
      ).toBeVisible({ timeout: 20_000 });

      await submitCredentials(user, creds.username, creds.password);

      // Spec 041 FR-019, closed 2026-09-07: an account the policy catches is
      // taken through enrolment rather than refused. So what appears here is
      // the *enrolment* card, not the code field — this used to wait on
      // `#login-two-factor` and would now hang, having asked for the one
      // screen the fix replaced.
      await expect(user.getByTestId("login-two-factor-enrol")).toBeVisible({
        timeout: 15_000,
      });
      expect(
        await isSignedIn(user),
        "an instance that requires 2FA must not sign in an account that has not satisfied it",
      ).toBe(false);

      // FR-020, and the whole point of FR-019: the offer has to be one the
      // person can actually take, from this screen, and it has to land them
      // where they were going. Anything less is the lockout with a friendlier
      // caption.
      //
      // Every value used here comes off the card itself — the typeable secret
      // the person would scan or copy — so this drives the same path a person
      // does rather than a shortcut through the API.
      const shownSecret = (
        await user.getByTestId("two-factor-setup-key").innerText()
      ).replace(/\s+/g, "");
      expect(
        shownSecret.length,
        "the enrolment card must show a typeable secret, for somebody with no camera",
      ).toBeGreaterThan(16);

      await user
        .getByTestId("two-factor-code")
        .fill(currentTotpCode(shownSecret));
      await user.getByTestId("two-factor-confirm").click();

      // FR-006: the recovery codes are on this screen, at the one moment they
      // are free — a fresh account under a new policy has no second
      // administrator to ask and nowhere else to get them.
      await expect(
        user.getByTestId("two-factor-recovery-code").first(),
      ).toBeVisible({ timeout: 20_000 });
      await user.getByTestId("two-factor-acknowledge-codes").click();

      // And the sign-in the person started finishes, rather than dumping them
      // back at /login to do it again.
      await user.waitForURL(/\/(welcome|worlds\/create)$/, { timeout: 20_000 });
      expect(
        await isSignedIn(user),
        "confirming the enrolment must finish the sign-in it interrupted (FR-020)",
      ).toBe(true);

      // FR-022: turning the policy off does not disarm the factor that was
      // just confirmed under it. The account is now enrolled, so the same
      // password gets a *verification* challenge whatever the switch says —
      // which is the opposite of what a stored requirement would have done.
      await setPolicy(page, false);
      await user.context().clearCookies();

      await submitCredentials(user, creds.username, creds.password);
      await expect(twoFactorField(user)).toBeVisible({ timeout: 15_000 });
      await twoFactorField(user).fill(currentTotpCode(shownSecret));
      await user.getByRole("button", { name: /^verify$/i }).click();
      await user.waitForURL(/\/(welcome|worlds\/create)$/, { timeout: 20_000 });
      expect(await isSignedIn(user)).toBe(true);
    } finally {
      await userContext.close();
    }
  });
});

test.describe("switching two-factor back off", () => {
  /**
   * Spec 041 FR-013, fixed 2026-09-07 by ADR-081.
   *
   * This was an expected failure, and it recorded a real hole: there is no
   * disable endpoint at all, so the only route *off* two-factor was also a
   * route *round* it. `setup/start` asked for the password and nothing else,
   * then wrote a fresh secret over the live one and cleared
   * `two_factor_enabled` in the same statement — a stranger holding only the
   * password could start an enrolment, never confirm it, and leave the account
   * back on passwords alone. The second factor removed by somebody who proved
   * only the first.
   *
   * An enrolment in progress now lives beside the live factor rather than on
   * top of it, and is promoted only when a code proves the new secret works.
   * The assertion below did not change when the product was fixed; the
   * `test.fail()` above it was simply removed.
   *
   * The deliberate way off — password **and** possession — is
   * `two-factor-removal.spec.ts`. It is a separate file because it is a
   * separate act: this one asserts that starting an enrolment is *not* a way
   * to remove a factor, and that only means something while a real way to
   * remove one exists elsewhere.
   */
  test("a fresh enrolment request must not silently strip the confirmed factor", async ({
    page,
  }) => {
    const creds = freshCredentials("e2e2fa");
    await register(page, creds);
    await enrolTwoFactor(page, creds);
    await logout(page);

    // Only the password — no code, no session, no possession of the secret.
    const restart = await postAuth(page, "2fa/setup/start", {
      username: creds.username,
      password: creds.password,
    });
    expect(restart.status).toBe(200);

    const attempt = await postAuth(page, "login", {
      identifier: creds.username,
      password: creds.password,
    });
    expect(
      attempt.body.status,
      "an unconfirmed re-enrolment must leave the existing second factor in force",
    ).toBe("two_factor_required");
    expect(attempt.status).toBe(401);
  });
});
