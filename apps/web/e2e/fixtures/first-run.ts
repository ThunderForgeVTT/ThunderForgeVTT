import { expect, type Page } from "./test";
import { readFileSync } from "node:fs";
import { totpNow } from "./totp";

/**
 * The walk every first-run spec shares.
 *
 * # Why this is a module and not three copies
 *
 * There are three specs in the first-run lane now rather than one — the
 * original end-to-end walk, a private instance and a public one (spec 064,
 * satisfying spec 052 FR-060 and FR-061) — and all three start identically:
 * find the bootstrap code in the server's log, open the wizard, create the
 * first administrator, and sooner or later enrol a second factor. Those are
 * mechanics, not claims. A spec's own file should hold what it asserts, and
 * the three of them asserting different things about the same four mechanics
 * is exactly what a fixture is for.
 *
 * Nothing here asserts anything a spec is responsible for. `openWizard` pins
 * the setup link's shape because that assertion belongs to whoever gets there
 * first and is the same assertion for all of them; everything else is a
 * sequence of actions that throws if the UI it is driving is not there.
 *
 * # Why this lane exists at all
 *
 * Every other spec runs against a stack cloned from a seeded template where
 * `demo_accounts.sql` has already created the platform administrator and
 * marked setup complete. Against that stack `/setup` redirects and first run
 * is not merely hard to test but unobservable. So `e2e-parallel.mjs` builds a
 * second template that is migrated and deliberately unseeded, starts a stack
 * of its own per first-run spec, and runs the `first-run` Playwright project
 * there.
 */

/** Where `e2e-parallel.mjs` tees this stack's backend output. */
export const BACKEND_LOG = process.env.THUNDERFORGE_E2E_BACKEND_LOG;

/** The one address that must be refused: a reserved domain cannot receive a notice. */
export const RESERVED_NOTICE_ADDRESS = "dmca@thunderforge.example";

/** The provider stub this shard talks to. See `scripts/oauth-stub.mjs`. */
export const OAUTH_STUB =
  process.env.THUNDERFORGE_E2E_OAUTH_STUB ?? "http://127.0.0.1:31600";

/** A password that satisfies every policy this instance ships with. */
export const ADMIN_PASSWORD = "Sup3r-Secret-Passphrase!";

/**
 * The setup link the server printed, waited for rather than read once.
 *
 * From the log because that is the only place it exists in plaintext: the
 * database stores an Argon2 hash of it, deliberately. Reading it out of the
 * log is not a test convenience — it is exactly what an operator does with a
 * container's output.
 *
 * The stack answers `/api/readyz` before `ensure_admin_bootstrap_code` has
 * necessarily logged, so a single read races the line it is looking for.
 */
export async function waitForSetupCode(): Promise<string> {
  if (!BACKEND_LOG) {
    throw new Error(
      "THUNDERFORGE_E2E_BACKEND_LOG is unset. This spec runs only in the " +
        "first-run lane, which `scripts/e2e-parallel.mjs` starts; running it " +
        "against a seeded stack could not work, because setup is already done there.",
    );
  }
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    let log = "";
    try {
      log = readFileSync(BACKEND_LOG, "utf-8");
    } catch {
      // Not written yet.
    }
    // The path form is what an instance with no public URL logs; the full-link
    // form is what this lane produces, because the runner sets
    // THUNDERFORGE_PUBLIC_URL. Accepting both means this keeps working if that
    // ever changes, and `openWizard`'s assertion is what pins the behaviour.
    const found = /\/setup\/([A-Za-z0-9_-]{8,})/.exec(log);
    if (found) {
      return found[1];
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(`no setup link in ${BACKEND_LOG} after 30s`);
}

/**
 * A GraphQL read over the anonymous transport.
 *
 * `/api/graphql` is wrapped in `require_authenticated_user`, so the ordinary
 * helper cannot express "a stranger asks" — it would assert a 401 rather than
 * the answer. `/api/graphql/public` is the endpoint the anonymous readers use.
 *
 * Load a page in the context first. The CSRF check is a double submit: it
 * compares the `csrf_token` cookie against the header, and a fresh or
 * just-cleared context has neither until it has loaded something. A bare POST
 * comes back 403 with an empty body, which surfaces as a JSON parse error and
 * looks nothing like the refusal it is.
 */
export async function graphqlPublic<T>(page: Page, query: string): Promise<T> {
  const csrf = (await page.context().cookies()).find(
    (cookie) => cookie.name === "csrf_token",
  )?.value;
  const response = await page.request.post("/api/graphql/public", {
    headers: {
      "Content-Type": "application/json",
      ...(csrf ? { "x-csrf-token": csrf } : {}),
    },
    data: { query, variables: {} },
  });
  const text = await response.text();
  try {
    return JSON.parse(text) as T;
  } catch {
    throw new Error(
      `Non-JSON response (status ${response.status()}): ${text.slice(0, 300)}`,
    );
  }
}

/** Move to the next step and wait for the walk to actually advance. */
export async function advanceFrom(page: Page, stepId: string) {
  await page.getByTestId("setup-next").click();
  await expect(page.getByTestId(`setup-step-${stepId}`)).toBeHidden({
    timeout: 15_000,
  });
}

/**
 * An unconfigured instance sends you to setup, and the link it printed names
 * the instance you are actually running on.
 *
 * The second half is a real defect pinned in place: a hard-coded
 * `http://127.0.0.1:5173/setup/{code}` is the first thing a containerised
 * operator sees, and it is wrong for every one of them.
 */
export async function openWizard(page: Page): Promise<string> {
  await page.goto("/");
  await expect(page).toHaveURL(/\/setup/, { timeout: 30_000 });

  const code = await waitForSetupCode();
  const log = readFileSync(BACKEND_LOG!, "utf-8");
  expect(
    log,
    "the setup link must name this instance's public URL, not a hard-coded dev host",
  ).toContain(`/setup/${code}`);
  expect(log).not.toContain("127.0.0.1:5173/setup/");

  await page.goto(`/setup/${code}`);
  await expect(page.getByTestId("setup-wizard")).toBeVisible({
    timeout: 30_000,
  });
  return code;
}

/**
 * Create the first administrator, acknowledgement and all.
 *
 * This no longer ends setup — ADR-093 — which is the whole reason the rest of
 * a first-run spec can run at all. Spec 039 US8 (FR-041, FR-042): the person
 * becoming the operator is shown what they take on, and setup will not create
 * them until they acknowledge it, so the acknowledgement is clicked here
 * rather than assumed.
 */
export async function createFirstAdministrator(
  page: Page,
  { username, email }: { username: string; email: string },
): Promise<void> {
  await page.getByTestId("setup-account-username").fill(username);
  await page.getByTestId("setup-account-email").fill(email);
  await page.getByTestId("setup-account-password").fill(ADMIN_PASSWORD);
  await page
    .getByTestId("setup-account-password-confirmation")
    .fill(ADMIN_PASSWORD);
  await expect(page.getByTestId("setup-operator-statement")).toContainText(
    "Running this instance makes you responsible for it",
    { timeout: 30_000 },
  );
  await page.getByTestId("setup-operator-acknowledge").click();
  await page.getByTestId("setup-account-submit").click();
  await expect(page.getByTestId("setup-step-account")).toBeHidden({
    timeout: 30_000,
  });
}

/**
 * Enrol and confirm a second factor, which FR-002a makes a condition of
 * completing setup. Spec 041's flow, reused rather than reimplemented.
 */
export async function enrolSecondFactor(page: Page): Promise<void> {
  await expect(page.getByTestId("setup-step-second-factor")).toBeVisible({
    timeout: 30_000,
  });
  await page.getByTestId("setup-second-factor-start").click();
  const setupKey = page.getByTestId("two-factor-setup-key");
  await expect(setupKey).toBeVisible({ timeout: 30_000 });
  const secret = ((await setupKey.textContent()) ?? "").replace(/\s+/g, "");
  expect(secret.length).toBeGreaterThan(0);

  await page.getByTestId("two-factor-code").fill(totpNow(secret));
  await page.getByTestId("two-factor-confirm").click();

  const recoveryCodes = page.getByTestId("two-factor-recovery-code");
  await expect(recoveryCodes.first()).toBeVisible({ timeout: 30_000 });
  expect(await recoveryCodes.count()).toBe(10);
  await page.getByTestId("two-factor-acknowledge-codes").click();

  await expect(page.getByTestId("setup-second-factor-confirmed")).toBeVisible({
    timeout: 30_000,
  });
}

/**
 * Walk forward, skipping whatever is skippable, until `stepId` is on screen.
 *
 * Deliberately generic rather than naming the steps in between: the wizard is
 * driven by the settings registry, so a declaration added later lands in this
 * walk without a spec being edited. A spec that cares about a particular step
 * asserts it on the way past instead of listing every step to get there.
 */
export async function walkTo(
  page: Page,
  stepId: string,
  limit = 12,
): Promise<void> {
  const target = page.getByTestId(`setup-step-${stepId}`);
  for (let guard = 0; guard < limit; guard += 1) {
    if (await target.isVisible().catch(() => false)) {
      return;
    }
    const skip = page.getByTestId("setup-skip-step");
    if (await skip.isVisible().catch(() => false)) {
      await skip.click();
      continue;
    }
    await page.getByTestId("setup-next").click();
  }
  await expect(target).toBeVisible({ timeout: 30_000 });
}

/**
 * Open one provider's editor on the providers step and return it.
 *
 * Found by its displayed name and addressed by suffix, because every control
 * in the editor is keyed by the provider's **id** — a UUID the operator's
 * browser learns at runtime and a spec cannot write down. Suffix selectors
 * inside the open editor row are the stable way to reach them, and they stay
 * correct if the key ever stops being a UUID.
 */
export async function openProviderEditor(page: Page, displayName: string) {
  const table = page.getByTestId("oauth-providers-table");
  await expect(table).toBeVisible({ timeout: 30_000 });
  const row = table
    .locator('[data-testid^="oauth-provider-row-"]')
    .filter({ hasText: displayName })
    .first();
  const toggle = row.getByRole("button", { name: /^(Configure|Edit|Done)$/ });
  if ((await toggle.getAttribute("aria-expanded")) !== "true") {
    await toggle.click();
  }
  const id = (await row.getAttribute("data-testid"))?.replace(
    "oauth-provider-row-",
    "",
  );
  const editor = page.locator(`#oauth-provider-editor-${id}`);
  await expect(editor).toBeVisible({ timeout: 15_000 });
  return editor;
}
