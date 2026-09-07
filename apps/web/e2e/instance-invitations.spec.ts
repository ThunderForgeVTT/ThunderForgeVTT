import { expect, test, type Page } from "@playwright/test";
import { ADMIN_USER } from "./fixtures/global-setup";
import { graphql, login, uniqueSuffix } from "./fixtures/helpers";

/**
 * Spec 035 US2: an invitation lets one named person through a shut door.
 *
 * Redemption is exercised from a **fresh browser context that has never
 * signed in**, because that is the only thing that can prove the page works
 * for the person it was sent to — someone with no account and no session.
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
async function loginAsAdmin(page: Page): Promise<void> {
  await login(page, ADMIN_USER.identifier, ADMIN_USER.password);
  await page.waitForURL(/\/(admin|welcome)$/, { timeout: 20_000 });
}

/** Puts the instance back to open, signing in first only if needed. */
async function restoreOpen(page: Page): Promise<void> {
  try {
    await setPolicy(page, "OPEN");
  } catch {
    await loginAsAdmin(page);
    await setPolicy(page, "OPEN");
  }
}

async function issue(
  page: Page,
  note: string,
): Promise<{ id: string; inviteCode: string }> {
  const result = await graphql<{
    data: { createInstanceInvitation: { id: string; inviteCode: string } };
  }>(
    page,
    `
      mutation Issue($input: CreateInstanceInvitationInput!) {
        createInstanceInvitation(input: $input) {
          id
          inviteCode
        }
      }
    `,
    { input: { maxUses: 1, expiresInHours: 24, note } },
  );
  return result.data.createInstanceInvitation;
}

test.describe("spec 035 US2: invite one named person in", () => {
  test.afterEach(async ({ page }) => {
    // Leave the instance open: every other spec in the suite registers users,
    // and a shard that inherited a closed instance would fail everywhere at
    // once with an error about accounts rather than about itself.
    await restoreOpen(page);
  });

  test("an invite-only instance admits the invited and refuses everyone else, identically", async ({
    page,
    browser,
  }) => {
    test.setTimeout(180_000);
    const suffix = uniqueSuffix();

    await loginAsAdmin(page);
    await setPolicy(page, "INVITE_ONLY");

    const good = await issue(page, `Priya ${suffix}`);
    const doomed = await issue(page, `Revoked ${suffix}`);
    await graphql(
      page,
      `
        mutation Revoke($id: UUID!) {
          revokeInstanceInvitation(invitationId: $id)
        }
      `,
      { id: doomed.id },
    );

    const visitorContext = await browser.newContext();
    const visitor = await visitorContext.newPage();
    try {
      // The invited person, with no account and no session, lands on a page
      // that works — not a login redirect.
      await visitor.goto(`/invite/${good.inviteCode}`);
      await expect(visitor.getByTestId("invite-register")).toBeVisible({
        timeout: 20_000,
      });
      expect(
        new URL(visitor.url()).pathname,
        "the invite page must not redirect a signed-out visitor",
      ).toBe(`/invite/${good.inviteCode}`);

      const username = `invited${suffix}`;

      // Each attempt gets its own browser context, which is both faithful and
      // necessary: these are different strangers, and a context that has just
      // registered holds a session whose CSRF token later POSTs would need.
      // Reusing one context returned 403 on the second attempt — a real
      // finding about the harness, not about the gate.
      const register = async (code: string, user: string) => {
        const ctx = await browser.newContext();
        try {
          const response = await ctx.request.post(
            "/api/authentication/register",
            {
              headers: { "Content-Type": "application/json" },
              data: {
                username: user,
                email: `${user}@example.test`,
                password: "Sup3r-Secret-Passphrase!",
                invitation_code: code,
              },
            },
          );
          return { status: response.status(), body: await response.text() };
        } finally {
          await ctx.close();
        }
      };

      const admitted = await register(good.inviteCode, username);
      expect(
        admitted.status,
        "a valid invitation must admit despite the instance not being open",
      ).toBeLessThan(300);

      // US2-5 and US2-4: exhausted and revoked must be one refusal, and a
      // code that never existed must join them.
      const bodies: string[] = [];
      for (const [code, user] of [
        [good.inviteCode, `second${suffix}`],
        [doomed.inviteCode, `revoked${suffix}`],
        ["NOTAREALCODEATALL0", `unknown${suffix}`],
      ]) {
        const response = await register(code, user);
        expect(response.status).toBe(409);
        bodies.push(response.body);
      }
      expect(bodies[1], "revoked must read exactly like exhausted").toBe(
        bodies[0],
      );
      expect(bodies[2], "unknown must read exactly like revoked").toBe(
        bodies[0],
      );
      for (const body of bodies) {
        expect(body.toLowerCase()).not.toContain("revok");
        expect(body.toLowerCase()).not.toContain("expire");
        expect(body.toLowerCase()).not.toContain("exhaust");
      }

      // FR-018: the operator can see who came in on which link.
      const listed = await graphql<{
        data: {
          instanceInvitations: {
            id: string;
            state: string;
            remainingUses: number;
            redemptions: { username: string }[];
          }[];
        };
      }>(
        page,
        `
          query {
            instanceInvitations {
              id
              state
              remainingUses
              redemptions {
                username
              }
            }
          }
        `,
        {},
      );
      const redeemed = listed.data.instanceInvitations.find(
        (i) => i.id === good.id,
      );
      expect(redeemed?.state).toBe("EXHAUSTED");
      expect(redeemed?.remainingUses).toBe(0);
      expect(redeemed?.redemptions.map((r) => r.username)).toContain(username);
    } finally {
      await visitorContext.close();
    }
  });
});
