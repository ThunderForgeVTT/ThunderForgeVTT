import { expect, test, type Browser, type Page } from "@playwright/test";
import { ADMIN_USER } from "./fixtures/global-setup";
import {
  freshCredentials,
  loginAsAdmin,
  register,
  type Credentials,
} from "./fixtures/helpers";
import { totpAt } from "./fixtures/totp";

/**
 * Spec 041 US6 and US7, and quickstart Scenario I: what an operator may do to
 * somebody else's second factor.
 *
 * # What this file is really asserting
 *
 * That FR-024 is satisfied, and FR-024's own test is **"no step needed
 * `psql`"**. Until this surface existed, the only way to help somebody who had
 * lost both their authenticator and their recovery codes was a database edit
 * against a live table: unaudited by construction, unnotified, and performed
 * from memory at the worst possible moment. So every step below happens through
 * the interface an operator actually has, and nothing here reaches for a
 * connection string.
 *
 * The other half is what an operator must **not** be able to do. A reset that
 * handed back a secret, a set of recovery codes, or a session would let an
 * operator sign in as the account holder, and no amount of audit trail makes
 * that acceptable. That is asserted as directly as an e2e can: the operator's
 * own screen after the reset, and the account's own state.
 */

/** Seconds per code — `STEP_SECONDS` in `crates/thunderforge-axum-auth-core`. */
const STEP_SECONDS = 30;

/**
 * The code an enrolment should confirm with: the **previous** step's.
 *
 * Confirming an enrolment spends the step its code matched (FR-016), so a
 * sign-in immediately afterwards would offer a code the server has already
 * seen and be refused for a reason that has nothing to do with what is being
 * tested. The previous step is still inside the ±1 skew window so it confirms,
 * and it is lower than the current step so the next code is unspent — which
 * avoids a 30-second wait that would put these tests over Playwright's own
 * timeout. The short wait exists only to keep off a step boundary.
 *
 * A real person never meets any of this: they enrol and stay signed in.
 */
async function codeForConfirmingAnEnrolment(
  secretBase32: string,
): Promise<string> {
  const secondsIntoStep = (Date.now() / 1000) % STEP_SECONDS;
  if (secondsIntoStep > STEP_SECONDS - 3) {
    await new Promise((resolve) =>
      setTimeout(resolve, (STEP_SECONDS - secondsIntoStep + 1) * 1000),
    );
  }
  return totpAt(secretBase32, Date.now() / 1000 - STEP_SECONDS);
}

async function jsonHeaders(page: Page): Promise<Record<string, string>> {
  const csrf = (await page.context().cookies()).find(
    (cookie) => cookie.name === "csrf_token",
  )?.value;
  return {
    "Content-Type": "application/json",
    ...(csrf ? { "x-csrf-token": csrf } : {}),
  };
}

interface AuthCall {
  status: number;
  body: {
    status?: string;
    message?: string;
    otpauth_url?: string;
    login_two_factor_challenge_id?: string;
    events?: { event_type?: string; by_someone_else?: boolean }[];
  };
}

async function postAuth(
  page: Page,
  path: string,
  data: Record<string, unknown> = {},
): Promise<AuthCall> {
  const response = await page.request.post(`/api/authentication/${path}`, {
    headers: await jsonHeaders(page),
    data,
  });
  const text = await response.text();
  try {
    return { status: response.status(), body: JSON.parse(text) };
  } catch {
    throw new Error(
      `Non-JSON reply from /api/authentication/${path} (${response.status()}): ${text.slice(0, 300)}`,
    );
  }
}

function secretFromOtpauthUrl(otpauthUrl: string): string {
  const secret = new URL(otpauthUrl).searchParams.get("secret");
  expect(secret, `no secret in ${otpauthUrl}`).toBeTruthy();
  return secret as string;
}

/** Enrol through the API, because the *subject* of these tests is what an
 * operator does afterwards — the enrolment interface has its own spec. */
async function enrol(page: Page, creds: Credentials): Promise<string> {
  const start = await postAuth(page, "2fa/setup/start", {
    username: creds.username,
    password: creds.password,
  });
  expect(start.status, start.body.message).toBe(200);
  const secret = secretFromOtpauthUrl(start.body.otpauth_url ?? "");

  const confirm = await postAuth(page, "2fa/setup/confirm", {
    username: creds.username,
    password: creds.password,
    code: await codeForConfirmingAnEnrolment(secret),
  });
  expect(confirm.status, confirm.body.message).toBe(200);
  return secret;
}

async function logout(page: Page): Promise<void> {
  const response = await page.request.post("/api/authentication/logout", {
    headers: await jsonHeaders(page),
  });
  expect(response.status()).toBe(200);
}

async function isSignedIn(page: Page): Promise<boolean> {
  return (
    (await page.request.get("/api/authentication/session")).status() === 200
  );
}

/** The operator's own context, so nothing here shares a session with the
 * account being acted on — which is the whole point of the surface. */
async function asOperator(browser: Browser): Promise<Page> {
  const context = await browser.newContext();
  const page = await context.newPage();
  // Spec 041 FR-027: an administrator holds a second factor, so signing in
  // as one takes two steps. `loginAsAdmin` does both.
  await loginAsAdmin(page);
  // `/admin/security` — the section that holds the instance-wide enforcement
  // switch, and now the one-account controls beside it. Not
  // `/admin/configuration`, which is OAuth and the manifest.
  await page.goto("/admin/security");
  await expect(page.getByRole("heading", { name: /one account/i })).toBeVisible(
    {
      timeout: 30_000,
    },
  );
  return page;
}

/** Find one account on the operator's screen, by exact identifier. */
async function findAccount(operator: Page, username: string): Promise<void> {
  await operator.getByTestId("admin-account-identifier").fill(username);
  await operator.getByTestId("admin-account-find").click();
  await expect(operator.getByTestId("admin-account-username")).toHaveText(
    username,
    { timeout: 20_000 },
  );
}

test.describe("an operator and somebody else's second factor", () => {
  // Each of these runs two or three browser contexts through registration,
  // enrolment and the admin surface. The default 30 seconds is for a test that
  // does one thing; these do a whole scenario, and a timeout here reads as a
  // product failure when it is only arithmetic.
  test.setTimeout(180_000);

  /**
   * Quickstart Scenario I, end to end.
   *
   * The person has lost everything. What has to be true afterwards: the factor
   * is gone, **no credential reached the operator**, nobody was signed in by
   * the reset, the account can sign in on its password and is taken through
   * enrolment, and both parties can see what happened — the account holder on
   * their own security page, which is how somebody is told on an instance with
   * no mail configured.
   */
  test("resets a factor for somebody who lost everything, and hands the operator nothing", async ({
    page,
    browser,
  }) => {
    const creds = freshCredentials("e2e2fareset");
    await register(page, creds);
    await enrol(page, creds);
    await logout(page);

    const operator = await asOperator(browser);
    try {
      await findAccount(operator, creds.username);
      await expect(
        operator.getByTestId("admin-account-factor-state"),
      ).toContainText(/holds a confirmed second factor/i);

      // Armed on the first click, done on the second. The account it is about
      // stays on the screen while the operator decides, which is the point of
      // not using a modal.
      await operator.getByTestId("admin-account-reset").click();
      await operator.getByTestId("admin-account-reset").click();

      await expect(
        operator.getByTestId("admin-account-factor-state"),
      ).toContainText(/holds no second factor/i, { timeout: 20_000 });

      // FR-025: nothing an operator could sign in with. Asserted against the
      // whole page rather than one element, because the failure this guards
      // against is a credential appearing *somewhere* it was not designed to.
      const shown = await operator.locator("body").innerText();
      expect(shown, "a reset must not show a recovery code").not.toMatch(
        /[A-Z0-9]{4}-[A-Z0-9]{4}/,
      );
      expect(shown, "a reset must not show a shared secret").not.toMatch(
        /otpauth:\/\//,
      );
      expect(
        await isSignedIn(page),
        "the reset must not sign the subject in",
      ).toBe(false);
    } finally {
      await operator.context().close();
    }

    // The account signs in on its password, and is taken through enrolment
    // rather than refused — because the reset left it with no factor, not with
    // a broken one.
    const signIn = await postAuth(page, "login", {
      identifier: creds.username,
      password: creds.password,
    });
    expect(signIn.status, signIn.body.message).toBe(200);
    expect(await isSignedIn(page)).toBe(true);

    // FR-015, and the step that makes this honest on an instance with no mail:
    // the account holder can see what was done to them, and that it was
    // somebody else who did it.
    const history = await page.request.get("/api/authentication/2fa/history");
    expect(history.status()).toBe(200);
    const entries = ((await history.json()) as AuthCall["body"]).events ?? [];
    const reset = entries.find(
      (entry) => entry.event_type === "reset_by_operator",
    );
    expect(
      reset,
      "the account holder must be able to see that their factor was reset",
    ).toBeTruthy();
    expect(reset?.by_someone_else, "and that it was not their own doing").toBe(
      true,
    );
  });

  /**
   * US6 / FR-023: requiring a second factor of **one** account.
   *
   * The instance-wide switch is next door and reaches everybody, so the thing
   * worth proving is the boundary: this reaches the account named and nobody
   * else, and the account it reaches is taken through enrolment at its next
   * sign-in rather than refused.
   */
  test("requires a factor of one account, and leaves everybody else alone", async ({
    page,
    browser,
  }) => {
    const target = freshCredentials("e2e2fareq");
    await register(page, target);
    await logout(page);

    const bystanderContext = await browser.newContext();
    const bystander = await bystanderContext.newPage();
    const other = freshCredentials("e2e2fabys");
    await register(bystander, other);
    await logout(bystander);

    const operator = await asOperator(browser);
    try {
      await findAccount(operator, target.username);
      await operator.getByTestId("admin-account-toggle-required").click();
      await expect(operator.getByTestId("admin-account-notice")).toBeVisible({
        timeout: 20_000,
      });

      // FR-019: required and not enrolled means *enrol*, never a refusal.
      const required = await postAuth(page, "login", {
        identifier: target.username,
        password: target.password,
      });
      expect(required.status).toBe(401);
      expect(required.body.status).toBe("two_factor_enrolment_required");
      expect(
        required.body.login_two_factor_challenge_id,
        "the instruction has to come with the ticket that lets them act on it",
      ).toBeTruthy();

      // And the account nobody named signs straight in.
      const untouched = await postAuth(bystander, "login", {
        identifier: other.username,
        password: other.password,
      });
      expect(
        untouched.status,
        "a per-account requirement must not reach a second account",
      ).toBe(200);

      // Clearing it is the other half of the control being real.
      await findAccount(operator, target.username);
      await operator.getByTestId("admin-account-toggle-required").click();
      await expect(operator.getByTestId("admin-account-notice")).toBeVisible({
        timeout: 20_000,
      });

      const released = await postAuth(page, "login", {
        identifier: target.username,
        password: target.password,
      });
      expect(released.status, released.body.message).toBe(200);
    } finally {
      await operator.context().close();
      await bystanderContext.close();
    }
  });

  /**
   * FR-027 / ADR-094: an administrator's second factor is a property of the
   * role, so there is no switch to find.
   *
   * A sentence rather than a disabled control, because "why can I not turn this
   * off" deserves an answer and a greyed-out switch is not one.
   */
  test("offers an administrator no switch, and says why", async ({
    browser,
  }) => {
    const operator = await asOperator(browser);
    try {
      await findAccount(operator, ADMIN_USER.identifier);
      await expect(
        operator.getByTestId("admin-account-is-admin"),
      ).toContainText(/required by the role/i);
      await expect(
        operator.getByTestId("admin-account-toggle-required"),
      ).toHaveCount(0);
    } finally {
      await operator.context().close();
    }
  });
});
