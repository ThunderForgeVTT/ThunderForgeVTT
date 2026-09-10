import { expect, test, type Page } from "@playwright/test";
import { DEMO_USER } from "./fixtures/global-setup";
import { graphql, login, loginAsAdmin, uniqueSuffix } from "./fixtures/helpers";

/**
 * Spec 035 US1 / ADR-072: a closed instance is closed on **every** path.
 *
 * # Why this has to be end to end
 *
 * The unit tests prove the gate decides correctly. Only a browser can prove
 * the decision reaches the person: that the sign-in surface says why, that a
 * direct POST is refused with a body that reveals nothing, and that a
 * pre-existing user is untouched throughout.
 *
 * # The OAuth leg lives next door now
 *
 * This header used to end with a warning: FR-006's headline case — a provider
 * identity with an unmatched verified email being refused — needed a
 * configured provider and a real handshake, "which this harness has no stub
 * for", so a green run here was not proof the OAuth path was gated.
 *
 * It has one. Spec 036 US6 added `scripts/oauth-stub.mjs` and a seeded
 * provider row, and `oauth-provider.spec.ts` walks that case on every run —
 * closing the instance, completing the whole handshake, asserting the refusal,
 * then re-opening and asserting *the same identity* is admitted. The local
 * route is still this file's subject; the provider route is that one's.
 */

async function setPolicy(page: Page, policy: string): Promise<void> {
  await graphql(
    page,
    `
      mutation SetPolicy($policy: InstanceAccessPolicy!) {
        setInstanceAccessPolicy(policy: $policy) {
          policy
        }
      }
    `,
    { policy },
  );
}

/**
 * Signs in as the seeded platform administrator.
 *
 * Waits for `/admin` **or** `/welcome`: `redirectAfterLogin` sends an admin to
 * the admin area, and a first draft of this spec waited only for `/welcome`
 * and timed out on a login that had entirely succeeded.
 */
/** Puts the instance back to open, signing in first only if needed. */
async function restoreOpen(page: Page): Promise<void> {
  try {
    await setPolicy(page, "OPEN");
  } catch {
    await loginAsAdmin(page);
    await setPolicy(page, "OPEN");
  }
}

test.describe("spec 035 US1: a closed instance is actually closed", () => {
  test.afterEach(async ({ page }) => {
    // Leave the instance open: every other spec in the suite registers users,
    // and a shard that inherited a closed instance would fail everywhere at
    // once with an error about accounts rather than about itself.
    await restoreOpen(page);
  });

  test("registration is refused by policy, reveals nothing, and leaves existing users alone", async ({
    page,
    browser,
  }) => {
    test.setTimeout(180_000);
    const suffix = uniqueSuffix();

    await loginAsAdmin(page);

    await setPolicy(page, "CLOSED");

    const visitorContext = await browser.newContext();
    const visitor = await visitorContext.newPage();
    try {
      // FR-011: a taken address and a free one must answer identically. If a
      // closed instance distinguished them it would be an account-existence
      // oracle for anyone who could read two status codes.
      const attempt = async (email: string) => {
        const response = await visitor.request.post(
          "/api/authentication/register",
          {
            headers: { "Content-Type": "application/json" },
            data: {
              username: `walkin${suffix}${Math.random().toString(36).slice(2, 6)}`,
              email,
              password: "Sup3r-Secret-Passphrase!",
            },
          },
        );
        return { status: response.status(), body: await response.text() };
      };

      const taken = await attempt("admin@example.test");
      const free = await attempt(`nobody-${suffix}@example.test`);

      expect(taken.status, "a closed instance refuses registration").toBe(409);
      expect(free.status).toBe(taken.status);
      expect(
        free.body,
        "the refusal must not differ for an address that already exists",
      ).toBe(taken.body);
      expect(taken.body).not.toContain("already");

      // FR-003a: the surface says why, rather than silently dropping sign-up.
      await visitor.goto("/login");
      await expect(visitor.getByTestId("instance-not-open-notice")).toBeVisible(
        { timeout: 20_000 },
      );

      // US1-5 / SC-002: an existing account is entirely unaffected — it can
      // still sign in and reach its worlds while the instance is closed.
      //
      // Deliberately the ordinary demo user rather than the administrator.
      // Signing the *same* account in from a second context invalidates the
      // first session, so an earlier draft closed the instance, logged the
      // admin in twice, and then got a 401 reading its own audit log — a
      // finding about session handling, not about the gate.
      await login(visitor, DEMO_USER.identifier, DEMO_USER.password);
      await visitor.waitForURL(/\/welcome$/, { timeout: 20_000 });
    } finally {
      await visitorContext.close();
    }

    // FR-012: the refusals are visible to the operator, and carry no address.
    const events = await graphql<{
      data: {
        instanceAccessEvents: {
          eventType: string;
          attemptedRoute: string | null;
          policyAtAttempt: string | null;
        }[];
      };
    }>(
      page,
      `
        query {
          instanceAccessEvents(limit: 25) {
            eventType
            attemptedRoute
            policyAtAttempt
          }
        }
      `,
      {},
    );
    const refusals = events.data.instanceAccessEvents.filter(
      (e) => e.eventType === "admission_refused",
    );
    expect(refusals.length).toBeGreaterThanOrEqual(2);
    expect(refusals[0].attemptedRoute).toBe("local");
    expect(refusals[0].policyAtAttempt).toBe("closed");
    expect(JSON.stringify(refusals)).not.toContain("@");
  });
});
