import { test, expect, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { totpNow } from "./fixtures/totp";

/**
 * Spec 040 US1: an empty database becomes a usable, contactable instance.
 *
 * # Why this file needs a lane of its own
 *
 * Every other spec runs against a stack cloned from a seeded template, where
 * `demo_accounts.sql` has already created the platform administrator and
 * marked setup complete. Against that stack `/setup` redirects and first run
 * is not merely hard to test but unobservable — which is the actual reason no
 * setup spec existed before today, rather than anybody forgetting.
 *
 * So `e2e-parallel.mjs` builds a second template that is migrated and
 * deliberately unseeded, starts a stack of its own against it, and runs the
 * `first-run` Playwright project there. Nothing in this file is skipped when
 * that lane is absent — the whole file simply is not collected by the other
 * project, which is a partition rather than a filter.
 *
 * # Where the bootstrap code comes from
 *
 * From the server's log, because that is the only place it exists in
 * plaintext: the database stores an Argon2 hash of it, deliberately. Reading
 * it out of the log is not a test convenience — it is exactly what an operator
 * does with a container's output, which is why T074 asks for it this way.
 */

const BACKEND_LOG = process.env.THUNDERFORGE_E2E_BACKEND_LOG;

/** The one address that must be refused: a reserved domain cannot receive a notice. */
const RESERVED_NOTICE_ADDRESS = "dmca@thunderforge.example";

/**
 * The setup link the server printed, waited for rather than read once.
 *
 * The stack answers `/api/readyz` before `ensure_admin_bootstrap_code` has
 * necessarily logged, so a single read races the line it is looking for.
 */
async function waitForSetupCode(): Promise<string> {
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
    // ever changes, and the assertion below is what pins the behaviour.
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
 * the answer. `/api/graphql/public` is the endpoint the five existing
 * anonymous readers use, and FR-056 makes the notice contact the sixth.
 */
async function graphqlPublic<T>(page: Page, query: string): Promise<T> {
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
async function advanceFrom(page: Page, stepId: string) {
  await page.getByTestId("setup-next").click();
  await expect(page.getByTestId(`setup-step-${stepId}`)).toBeHidden({
    timeout: 15_000,
  });
}

test.describe("Spec 040 US1: from an empty database to a contactable instance", () => {
  test("an operator sets this instance up in one pass, and the legal pages then name them", async ({
    page,
    context,
  }) => {
    // Registration, world-less setup, an Argon2 hash and a TOTP enrolment.
    test.setTimeout(5 * 60_000);

    const suffix = Date.now().toString(36);
    const operatorName = `Bruno's Table ${suffix}`;
    const operatorEmail = `operator-${suffix}@thunderforge.org`;
    const noticeEmail = `notices-${suffix}@thunderforge.org`;
    const adminPassword = "Sup3r-Secret-Passphrase!";

    // 1. An unconfigured instance sends you to setup, not to a sign-in page
    //    for an account that cannot exist yet.
    await page.goto("/");
    await expect(page).toHaveURL(/\/setup/, { timeout: 30_000 });

    // 2. The link names the instance you are actually running on. The defect
    //    this pins is real and was live this morning: a hard-coded
    //    `http://127.0.0.1:5173/setup/{code}` is the first thing a
    //    containerised operator sees, and it is wrong for every one of them.
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

    // 3. The administrator's account. This no longer ends setup — ADR-093 —
    //    which is the whole reason the rest of this test can run at all.
    await page.getByTestId("setup-account-username").fill(`admin${suffix}`);
    await page.getByTestId("setup-account-email").fill(operatorEmail);
    await page.getByTestId("setup-account-password").fill(adminPassword);
    await page
      .getByTestId("setup-account-password-confirmation")
      .fill(adminPassword);
    await page.getByTestId("setup-account-submit").click();

    // 4. Who operates this instance — and it refuses to continue with the
    //    name blank, saying which field it wants (FR-003).
    const operatorStep = page.getByTestId("setup-step-settings-operator");
    await expect(operatorStep).toBeVisible({ timeout: 30_000 });

    await page.getByTestId("setup-next").click();
    const refusal = page.getByTestId("setup-field-error").first();
    await expect(refusal).toBeVisible({ timeout: 15_000 });
    await expect(refusal).toHaveAttribute("data-field", "operator.name");
    await expect(operatorStep, "a refused step must not advance").toBeVisible();

    await page.getByTestId("setup-setting-operator.name").fill(operatorName);
    await page
      .getByTestId("setup-setting-operator.contact_email")
      .fill(operatorEmail);
    await advanceFrom(page, "settings-operator");

    // 5. The contact for copyright notices, which is the step that exists
    //    because a reserved domain can never receive one. `.example` is
    //    refused; a real address is taken.
    const noticesStep = page.getByTestId(
      "setup-step-settings-copyright-notices",
    );
    await expect(noticesStep).toBeVisible({ timeout: 30_000 });

    // The obligation that is the operator's and not this software's
    // (spec 039 FR-055). A liability boundary, so it is asserted rather than
    // assumed to have been written.
    await expect(
      page.getByTestId("setup-designated-agent-notice"),
    ).toBeVisible();

    await page
      .getByTestId("setup-setting-notice.contact_email")
      .fill(RESERVED_NOTICE_ADDRESS);
    await page
      .getByTestId("setup-setting-notice.contact_name")
      .fill(operatorName);
    await page.getByTestId("setup-next").click();
    const reserved = page.getByTestId("setup-field-error").first();
    await expect(reserved).toBeVisible({ timeout: 15_000 });
    await expect(reserved).toHaveAttribute(
      "data-field",
      "notice.contact_email",
    );

    await page
      .getByTestId("setup-setting-notice.contact_email")
      .fill(noticeEmail);
    await advanceFrom(page, "settings-copyright-notices");

    // 6. Everything between here and the second factor is skippable, and the
    //    review step has to say what was left. Walking it generically rather
    //    than naming the steps keeps this test honest about T069: the wizard
    //    is driven by the registry, so a declaration added later appears here
    //    without this file being edited.
    const secondFactor = page.getByTestId("setup-step-second-factor");
    for (let guard = 0; guard < 10; guard += 1) {
      if (await secondFactor.isVisible().catch(() => false)) {
        break;
      }
      const skip = page.getByTestId("setup-skip-step");
      if (await skip.isVisible().catch(() => false)) {
        await skip.click();
        continue;
      }
      await page.getByTestId("setup-next").click();
    }
    await expect(secondFactor).toBeVisible({ timeout: 30_000 });

    // 7. FR-002a: setup will not complete without a confirmed second factor.
    //    This is spec 041's flow, reused rather than reimplemented.
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

    await expect(page.getByTestId("setup-second-factor-confirmed")).toBeVisible(
      {
        timeout: 30_000,
      },
    );
    await advanceFrom(page, "second-factor");

    // 8. The review says what is still unset rather than implying it is done.
    await expect(page.getByTestId("setup-step-review")).toBeVisible({
      timeout: 30_000,
    });
    await expect(page.getByTestId("setup-review-unset")).toBeVisible();

    await page.getByTestId("setup-complete").click();
    // Finishing setup lands on the administration screen rather than on a
    // summary the operator has to dismiss. Readiness is a permanent admin
    // section now, so the one-time copy of it had become a speed bump — the
    // by-hand pass of Scenario A said so, and this is that change.
    await page.waitForURL(/\/admin(\?|$)/, { timeout: 30_000 });

    // 9. And now the part that makes "contactable" mean something: a stranger,
    //    with no account, reads the legal pages and finds this operator named
    //    where a placeholder used to be.
    await context.clearCookies();
    const stranger = await context.newPage();

    // The page first, then the query — in that order because the CSRF check is
    // a double submit: it compares the `csrf_token` cookie against the header,
    // and a context whose cookies were just cleared has neither until it has
    // loaded something. A bare POST here comes back 403 with an empty body,
    // which surfaces as a JSON parse error and looks nothing like the refusal
    // it is. Loading the page first is also what a stranger actually does.
    await stranger.goto("/legal/terms");

    // Asked directly, and deliberately so. "The page shows a marker" has
    // two very different causes — the instance never stored the value, or it
    // stored it and the page did not render it — and a test that only looks at
    // the rendered page cannot tell them apart. This is T062's own claim
    // (spec 039 FR-056): reachable with no account at all.
    const publishedBody = await graphqlPublic<{
      data?: {
        publishedOperatorValues?: {
          operatorName: string | null;
          operatorContactEmail: string | null;
          noticeContactEmail: string | null;
        } | null;
      };
      errors?: { message: string }[];
    }>(
      stranger,
      `
        query {
          publishedOperatorValues {
            operatorName
            operatorContactEmail
            noticeContactEmail
          }
        }
      `,
    );
    expect(
      publishedBody.errors,
      "the notice contact must be readable without an account",
    ).toBeFalsy();
    expect(publishedBody.data?.publishedOperatorValues?.operatorName).toBe(
      operatorName,
    );
    expect(
      publishedBody.data?.publishedOperatorValues?.noticeContactEmail,
    ).toBe(noticeEmail);

    for (const path of ["/legal/terms", "/legal/privacy"]) {
      await stranger.goto(path);
      await stranger.reload();
      await expect(
        stranger.getByText(operatorName).first(),
        `${path} must name the operator this instance was set up with`,
      ).toBeVisible({ timeout: 30_000 });
    }

    await stranger.goto("/legal/dmca");
    await expect(stranger.getByText(noticeEmail).first()).toBeVisible({
      timeout: 30_000,
    });
    // The address that was refused must not have reached the page by some
    // other route — this is the half that would still pass if the refusal
    // above had silently stored the value anyway.
    await expect(stranger.getByText(RESERVED_NOTICE_ADDRESS)).toHaveCount(0);

    // 10. And the prose markers nobody was asked about are still visibly
    //     markers. That is correct, not a defect: a page rendering an empty
    //     string where an operator's own words belong is worse than one that
    //     says plainly it is unconfigured.
    await stranger.goto("/legal/terms");
    await expect(
      stranger.getByText(/\[OPERATOR —/).first(),
      "an unwritten prose block must still show its marker",
    ).toBeVisible({ timeout: 30_000 });
  });
});
