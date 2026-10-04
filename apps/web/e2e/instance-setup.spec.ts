import { test, expect } from "./fixtures/test";
import { readFileSync } from "node:fs";
import { graphql } from "./fixtures/helpers";
import { totpNow } from "./fixtures/totp";
import {
  advanceFrom,
  BACKEND_LOG,
  graphqlPublic,
  RESERVED_NOTICE_ADDRESS,
  waitForSetupCode,
} from "./fixtures/first-run";

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
    // Spec 039 US8 (FR-041, FR-042): the person becoming the operator is
    // shown what they take on, and setup will not create them until they
    // acknowledge it.
    await expect(page.getByTestId("setup-operator-statement")).toContainText(
      "Running this instance makes you responsible for it",
      { timeout: 30_000 },
    );
    await page.getByTestId("setup-account-submit").click();
    await expect(
      page.getByTestId("setup-account-error"),
      "no administrator without the acknowledgement",
    ).toContainText("acknowledge");
    await page.getByTestId("setup-operator-acknowledge").click();
    await page.getByTestId("setup-account-submit").click();

    // 3a. Who may join, and whether this instance publishes beyond a world.
    //     Spec 064 made that second answer the hinge: a `false` here is what
    //     spares a private instance the copyright-notice and legal-prose
    //     steps entirely. This case is the instance that says yes, so that
    //     the steps below exist to be walked; the two first-run specs beside
    //     this one are the fork proper.
    const accessStep = page.getByTestId("setup-step-settings-access");
    await expect(accessStep).toBeVisible({ timeout: 30_000 });
    await page
      .getByTestId("setup-setting-instance.publishes_beyond_world")
      .check();
    await advanceFrom(page, "settings-access");

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
    const walkToSecondFactor = async () => {
      for (let guard = 0; guard < 12; guard += 1) {
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
    };
    await walkToSecondFactor();

    // 6a. An operator who reloads here — or comes back tomorrow — has lost
    //     the password the account step held in memory. The browser is still
    //     signed in as the administrator setup created, so the step asks the
    //     server for an enrolment ticket against the setup code rather than
    //     stranding them: nothing has to be typed again. (The password form
    //     is the fallback for a browser with no session, and stays on offer.)
    await page.reload();
    await expect(page.getByTestId("setup-next")).toBeVisible({
      timeout: 30_000,
    });
    await walkToSecondFactor();
    await expect(page.getByTestId("setup-second-factor-reenter")).toBeVisible();
    await expect(page.getByTestId("setup-second-factor-start")).toBeEnabled();

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

    // A reopened setup resumes at the first step still owed, so once the
    // factor is confirmed it goes on to the review by itself; a setup walked
    // in one sitting pauses on the confirmation first. Either is the factor
    // having been taken.
    const confirmedNotice = page.getByTestId("setup-second-factor-confirmed");
    const review = page.getByTestId("setup-step-review");
    await expect(confirmedNotice.or(review)).toBeVisible({ timeout: 30_000 });
    if (await confirmedNotice.isVisible()) {
      await advanceFrom(page, "second-factor");
    }

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
    // And stays there. `waitForURL` resolves on the first matching
    // navigation, so a landing that immediately bounces to `/login` satisfies
    // it — which is exactly what a stale auth context used to do here, and
    // what this run would otherwise keep passing through.
    await expect(page).toHaveURL(/\/admin(\?|$)/);

    // The landing says what comes next. Setup used to end on a dashboard for
    // an instance with one account and nothing naming how a second person
    // gets in; the card names it, in the terms of the access policy this
    // instance has, and links to where invitations are issued.
    const afterSetup = page.getByTestId("after-setup-card");
    await expect(afterSetup).toBeVisible({ timeout: 30_000 });
    await expect(afterSetup).toContainText("invite your players");
    await expect(page.getByTestId("after-setup-invite-link")).toHaveAttribute(
      "href",
      "/admin/access",
    );
    // Dismissing it is permanent for this landing: the parameter that showed
    // it is gone from the address, so a reload does not bring it back.
    await page.getByTestId("after-setup-dismiss").click();
    await expect(afterSetup).toHaveCount(0);
    await expect(page).not.toHaveURL(/bootstrap=complete/);
    await expect(page).toHaveURL(/\/admin(\?|$)/);

    // The enrolment ticket setup can hand an administrator who has no
    // password is a setup-time thing only. This browser is signed in as an
    // administrator and still holds the code, which is the most anybody could
    // ever present — and on a finished instance it is refused, because the
    // code stopped being valid when setup completed. Without this the route
    // would be "a session may enrol a second factor", which it must not be.
    const lateTicket = await page.evaluate(async (adminCode) => {
      const csrf =
        document.cookie
          .split("; ")
          .find((cookie) => cookie.startsWith("csrf_token="))
          ?.slice("csrf_token=".length) ?? "";
      const response = await fetch(
        "/api/authentication/setup/enrolment-ticket",
        {
          method: "POST",
          credentials: "same-origin",
          headers: {
            "Content-Type": "application/json",
            "x-csrf-token": decodeURIComponent(csrf),
          },
          body: JSON.stringify({ admin_code: adminCode }),
        },
      );
      const body = (await response.json().catch(() => null)) as {
        status?: string;
        login_two_factor_challenge_id?: string | null;
      } | null;
      return {
        http: response.status,
        status: body?.status ?? null,
        ticket: body?.login_two_factor_challenge_id ?? null,
      };
    }, code);
    expect(
      lateTicket.ticket,
      "no enrolment ticket is minted once setup is complete",
    ).toBeNull();
    expect(lateTicket.http).toBe(409);
    expect(lateTicket.status).toBe("setup_complete");

    // Spec 039 FR-043: the acknowledgement is on record — who, and which
    // version — in the same table a sharing agreement is, and of the words
    // this build ships.
    const acknowledgement = await graphql<{
      data?: {
        instanceOperatorAcknowledgement?: {
          isCurrent: boolean;
          acknowledgement: { subjectUsername: string | null } | null;
        };
      };
      errors?: { message: string }[];
    }>(
      page,
      `
        query {
          instanceOperatorAcknowledgement {
            isCurrent
            acknowledgement {
              subjectUsername
            }
          }
        }
      `,
      {},
    );
    expect(
      acknowledgement.data?.instanceOperatorAcknowledgement?.isCurrent,
      JSON.stringify(acknowledgement.errors),
    ).toBe(true);
    expect(
      acknowledgement.data?.instanceOperatorAcknowledgement?.acknowledgement
        ?.subjectUsername,
    ).toBe(`admin${suffix}`);

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

    // Spec 039 FR-046/FR-048: the copyright page names this instance's
    // operator as the party responsible, and says the project cannot act here.
    const responsible = stranger.locator('[data-testid^="dmca-section-who"]');
    await expect(responsible).toContainText(operatorName);
    await expect(responsible).toContainText("does not operate this instance");

    // FR-045: what the operator took on, readable without an account or a
    // repository.
    await stranger.goto("/legal/operator");
    await expect(
      stranger.getByTestId("legal-page-operator-responsibilities"),
    ).toContainText("Running this instance makes you responsible for it", {
      timeout: 30_000,
    });

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
