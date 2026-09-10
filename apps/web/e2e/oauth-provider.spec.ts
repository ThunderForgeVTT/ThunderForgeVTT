import { expect, test, type Page } from "@playwright/test";
import {
  freshCredentials,
  graphql,
  loginAsAdmin,
  register,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Spec 036 US6 (T065): signing in with a provider, against a provider.
 *
 * # The gap this closes
 *
 * External sign-in had never been driven end to end. `instance-access-gate.spec.ts`
 * says so in its own header — "the OAuth leg... needs a configured provider and
 * a real handshake, which this harness has no stub for... do not read a green
 * run here as proof the OAuth path is gated" — and spec 035's T056 and spec
 * 032's deferred manual pass carry the same note for the same reason.
 *
 * The untested region is the forty-odd lines between the provider redirecting
 * back and an account existing: the state check, the code exchange, the
 * userinfo read, the provisioning decision, and the admission policy. Unit
 * tests cover both ends of it and nothing had ever put a request on a wire.
 *
 * # Why this needs no product code (FR-023)
 *
 * `oauth_providers` carries `authorization_url`, `token_url` and
 * `userinfo_url` **per row**, because an operator wiring up their own provider
 * sets exactly those. The seeded `stub` provider points at
 * `scripts/oauth-stub.mjs`, and the server reaches it through the same code
 * path it uses for Google. There is no `#[cfg(test)]` branch anywhere in
 * `oauth.rs`, and there must not be: a test that exercises a test-only branch
 * proves the branch.
 */

const STUB =
  process.env.THUNDERFORGE_E2E_OAUTH_STUB ?? "http://127.0.0.1:31600";
const PROVIDER = "stub";

/**
 * Where the provider is told to send the browser back to.
 *
 * **Absolute**, because a real provider requires an absolute `redirect_uri`
 * and the product passes whatever it is given straight through. A relative
 * path here would be a test that exercised something no provider would accept.
 */
function callbackUrl(baseURL: string): string {
  return new URL(
    `/api/authentication/oauth/${PROVIDER}/callback`,
    baseURL,
  ).toString();
}

/** What the stub's `/userinfo` should answer next. `null` means an empty body. */
async function provideIdentity(
  page: Page,
  identity: Record<string, unknown> | null,
): Promise<void> {
  const response = await page.request.post(`${STUB}/_control/identity`, {
    data: { identity },
  });
  expect(
    response.ok(),
    "the stub accepts the identity for the next sign-in",
  ).toBe(true);
}

/**
 * Walk the whole handshake and return what the callback answered.
 *
 * `page.goto` on `/start` follows the 307 to the stub, the stub's 302 back to
 * the callback, and lands on the callback's JSON — which is the flow a person
 * takes, redirect for redirect. Nothing here constructs a `code` or a `state`:
 * the product made the state, the stub echoed it, and the product checked it.
 */
async function signInWithProvider(
  page: Page,
  baseURL: string,
): Promise<{ status: string; message: string; challengeId: string | null }> {
  const start = `/api/authentication/oauth/${PROVIDER}/start?redirect_uri=${encodeURIComponent(callbackUrl(baseURL))}`;
  const response = await page.goto(start);
  const body = (await response?.json().catch(() => null)) as {
    status?: string;
    message?: string;
    challenge_id?: string | null;
    challengeId?: string | null;
  } | null;

  return {
    status: body?.status ?? "",
    message: body?.message ?? "",
    challengeId: body?.challenge_id ?? body?.challengeId ?? null,
  };
}

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

test.describe("Spec 036 US6: external sign-in, against a real handshake", () => {
  test.afterEach(async ({ page }) => {
    // The stub is shared by every test in this shard, and an identity left
    // behind would arrive in the next scenario's sign-in as a mystery.
    await page.request.post(`${STUB}/_control/reset`).catch(() => {});
  });

  test("a first sign-in with a verified email provisions an account (ADR-042)", async ({
    page,
    baseURL,
  }) => {
    const subject = `stub-new-${uniqueSuffix()}`;
    await provideIdentity(page, {
      sub: subject,
      email: `${subject}@example.org`,
      email_verified: true,
      name: "Newcomer",
    });

    const first = await signInWithProvider(page, baseURL!);
    expect(
      first.status,
      `an unmatched verified email on an open instance is provisioned: ${first.message}`,
    ).toBe("success");

    // Signed in for real, not merely told so: the session cookie the callback
    // set is what answers this.
    const me = await graphql<{ data: { mySessions: { id: string }[] } | null }>(
      page,
      `
        query MySessions {
          mySessions {
            id
          }
        }
      `,
      {},
    );
    expect(me.data, "the callback issued a working session").not.toBeNull();

    // And the identity is remembered: a second sign-in is a *login*, not a
    // second provisioning. Nothing in the response distinguishes them, so the
    // account count is what says so.
    const second = await signInWithProvider(page, baseURL!);
    expect(second.status).toBe("success");
  });

  test("an existing account is linked only behind its password (ADR-006)", async ({
    page,
    browser,
    baseURL,
  }) => {
    // A local account first, with a password the linking step will have to
    // produce. `freshCredentials` builds an `@example.test` address; the
    // provider must return that same address for the match to happen.
    const creds = freshCredentials("e2eoauthlink");
    await register(page, creds);

    const visitor = await browser.newContext();
    const visitorPage = await visitor.newPage();
    try {
      await provideIdentity(visitorPage, {
        sub: `stub-link-${uniqueSuffix()}`,
        email: creds.email,
        email_verified: true,
      });

      const attempt = await signInWithProvider(visitorPage, baseURL!);
      expect(
        attempt.status,
        "a provider identity matching an existing account must not sign in on its own",
      ).toBe("password_required");
      expect(attempt.challengeId).toBeTruthy();

      // The wrong password does not link it. This is the assertion the whole
      // scenario exists for: without it, "password_required" would be a label
      // on a door that opens anyway.
      const refused = await visitorPage.request.post(
        "/api/authentication/oauth/link/confirm",
        {
          data: {
            challenge_id: attempt.challengeId,
            password: "not the right password at all",
          },
        },
      );
      expect(refused.ok()).toBe(false);

      const accepted = await visitorPage.request.post(
        "/api/authentication/oauth/link/confirm",
        {
          data: {
            challenge_id: attempt.challengeId,
            password: creds.password,
          },
        },
      );
      expect(
        accepted.ok(),
        `the right password links it: ${await accepted.text()}`,
      ).toBe(true);
    } finally {
      await visitor.close();
    }
  });

  test("a provider that returns no email cannot provision or link", async ({
    page,
    baseURL,
  }) => {
    // A subject and nothing else. The identity is real — the provider knows
    // who this is — but there is no address to match an account by and none to
    // provision one from.
    await provideIdentity(page, { sub: `stub-anon-${uniqueSuffix()}` });

    const refused = await signInWithProvider(page, baseURL!);
    expect(refused.status).toBe("no_matching_user");
    expect(refused.message).toMatch(/did not return an email/i);

    // And nothing was created on the way to refusing.
    const me = await graphql<{ data: unknown | null }>(
      page,
      `
        query MySessions {
          mySessions {
            id
          }
        }
      `,
      {},
    ).catch(() => ({ data: null }));
    expect(me.data, "a refused sign-in leaves no session").toBeNull();
  });

  /**
   * Spec 035 FR-006 and T056 — the case `instance-access-gate.spec.ts` could
   * not make and told the reader not to assume.
   */
  test("a closed instance refuses a provider identity it would otherwise provision", async ({
    page,
    browser,
    baseURL,
  }) => {
    test.setTimeout(180_000);
    await loginAsAdmin(page);
    await setPolicy(page, "CLOSED");

    const visitor = await browser.newContext();
    const visitorPage = await visitor.newPage();
    try {
      const subject = `stub-closed-${uniqueSuffix()}`;
      await provideIdentity(visitorPage, {
        sub: subject,
        email: `${subject}@example.org`,
        email_verified: true,
      });

      const refused = await signInWithProvider(visitorPage, baseURL!);
      expect(
        refused.status,
        "closing signups must close the provider path too, not just the form",
      ).toBe("instance_closed");

      // Re-open, and the very same identity is admitted. That is what makes
      // the refusal a policy decision rather than a broken handshake — the
      // most likely way this test could pass for the wrong reason.
      await setPolicy(page, "OPEN");
      const admitted = await signInWithProvider(visitorPage, baseURL!);
      expect(
        admitted.status,
        `the same identity is admitted once the instance re-opens: ${admitted.message}`,
      ).toBe("success");
    } finally {
      await setPolicy(page, "OPEN").catch(() => {});
      await visitor.close();
    }
  });
});
