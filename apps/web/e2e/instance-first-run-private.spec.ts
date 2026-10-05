import { expect, test } from "./fixtures/test";
import { graphql } from "./fixtures/helpers";
import {
  ADMIN_PASSWORD,
  advanceFrom,
  createFirstAdministrator,
  enrolSecondFactor,
  graphqlPublic,
  openProviderEditor,
  openWizard,
  OAUTH_STUB,
  walkTo,
} from "./fixtures/first-run";

/**
 * Spec 052 FR-060 (spec 064): a private instance is stood up without a legal
 * department.
 *
 * # What this proves that no other spec does
 *
 * That the fork is real. Until spec 064, every operator was asked for a
 * copyright-notice contact — a name, an email address and a postal address for
 * a designated agent — because `CAPABILITIES_SETUP_ASKS_ABOUT` listed the
 * publishing capability unconditionally. Somebody standing an instance up for
 * five friends was asked for a DMCA agent's postal address they owe to nobody.
 *
 * So the assertions here are mostly about questions that must **not** be
 * asked, which is a thing a test is uniquely good at and a code review is
 * uniquely bad at. And because "it was not asked" is also what a broken
 * wizard looks like, the absence is paired with the instance being told to say
 * so out loud: `setup-not-asked` (FR-011).
 *
 * # And that the instance that comes out is usable
 *
 * A wizard that gates the whole app and hands back an instance which cannot
 * store an image or let a second person in has not finished its job. So the
 * walk ends by proving the two things the operator asked for: storage answers,
 * and an invite link admits its holder **through a provider** — sign-in with
 * an account they already have, redeeming the invitation on the way past.
 * That path exists in `oauth.rs` today and had never been driven from a
 * freshly-set-up instance.
 */

/** The seeded issuer-derived provider. Its endpoints come from an issuer URL alone. */
const PROVIDER_NAME = "Keycloak";
const PROVIDER_KEY = "keycloak";

/**
 * What the bundled stack's object store answers to.
 *
 * Used only for whichever storage field the wizard actually offers. On a
 * deployment whose environment sets `RUSTFS_*` every one of them is fixed and
 * none of this is typed — which is why the walk below asks each field whether
 * it exists rather than assuming, and asserts on the probe's answer rather
 * than on a field count.
 */
const STORAGE_DEFAULTS: Record<string, string> = {
  "storage.endpoint": "http://localhost:9000",
  "storage.region": "us-east-1",
  "storage.access_key": "thunderforge-rustfs-root",
  "storage.secret_key": "thunderforge-rustfs-root-secret",
};

test.describe("Spec 052 FR-060: a private instance, set up without a legal department", () => {
  test("an operator says this instance publishes nothing, is never asked for a notice contact, and hands a friend a link that a provider admits", async ({
    page,
    browser,
  }) => {
    // A registration, an Argon2 hash, a TOTP enrolment, a storage probe and a
    // whole second browser context doing an OAuth handshake.
    test.setTimeout(6 * 60_000);

    const suffix = Date.now().toString(36);
    const operatorName = `The Quiet Table ${suffix}`;
    const operatorEmail = `operator-${suffix}@thunderforge.org`;

    await openWizard(page);
    await createFirstAdministrator(page, {
      username: `admin${suffix}`,
      email: operatorEmail,
    });

    // 1. Who may join — the first question now, because the answer decides
    //    what the rest of the wizard asks for.
    const accessStep = page.getByTestId("setup-step-settings-access");
    await expect(accessStep).toBeVisible({ timeout: 30_000 });

    // FR-032: the reason is in front of the operator before the question is.
    await expect(
      page.getByTestId("setup-group-explainer"),
      "the Access step explains what it is deciding before it asks",
    ).toBeVisible();

    // Each policy carries its consequence, and one of them is marked as the
    // recommendation — the operator's own preferred shape for a private table.
    await expect(page.getByTestId("setup-access-consequences")).toBeVisible();
    await expect(
      page.getByTestId("setup-access-consequence-invite_only"),
    ).toContainText("Recommended");

    await page.getByTestId("setup-access-consequence-invite_only").click();
    await expect(
      page.getByTestId("setup-access-consequence-invite_only"),
      "the chosen policy is marked as chosen, not merely listed",
    ).toHaveAttribute("data-chosen", "true");

    // Left unchecked, deliberately: this is the whole hinge of the fork.
    const publishes = page.getByTestId(
      "setup-setting-instance.publishes_beyond_world",
    );
    await expect(publishes).not.toBeChecked();
    await expect(
      page.getByTestId("setup-publishing-preview"),
      "nothing promises extra steps to an instance that has not asked for them",
    ).toBeHidden();

    await advanceFrom(page, "settings-access");

    // 2. FR-010 and FR-011 together: the legal steps do not exist, and the
    //    instance says why rather than leaving an operator to wonder whether
    //    it decided or forgot.
    await expect(
      page.getByTestId("setup-progress-settings-copyright-notices"),
      "a private instance is never asked for a copyright-notice contact",
    ).toHaveCount(0);
    await expect(
      page.getByTestId("setup-progress-settings-legal-prose"),
      "nor for public terms",
    ).toHaveCount(0);
    await expect(page.getByTestId("setup-not-asked")).toContainText(
      "copyright-notice contact",
    );

    // 3. Who operates it. Still required — FR-010 narrows what publishing
    //    obliges, and does not make an instance anonymous.
    await expect(
      page.getByTestId("setup-step-settings-operator"),
    ).toBeVisible();
    await page.getByTestId("setup-setting-operator.name").fill(operatorName);
    await page
      .getByTestId("setup-setting-operator.contact_email")
      .fill(operatorEmail);
    await expect(
      page.getByTestId("setup-setting-operator.jurisdiction"),
      "a jurisdiction is a publishing duty, so it is not asked for here",
    ).toHaveCount(0);
    await advanceFrom(page, "settings-operator");

    // 4. Support, which completion does require. Walked to generically: the
    //    wizard is registry-driven, so naming the steps in between would make
    //    this file an obstacle to adding a declaration.
    await walkTo(page, "settings-support");
    await page.getByTestId("setup-setting-support_email").fill(operatorEmail);
    await advanceFrom(page, "settings-support");

    // 5. Signing in with an account they already have. Keycloak is the
    //    issuer-derived kind: an issuer URL alone, and the product expands it
    //    into the three endpoints. Pointing it at this shard's stub means what
    //    gets exercised is `ProviderKind::derive_endpoints`, not a test's idea
    //    of what Keycloak's paths are.
    await expect(page.getByTestId("setup-step-providers")).toBeVisible({
      timeout: 30_000,
    });
    await expect(
      page.getByTestId("setup-providers-explainer"),
      "invite-only says plainly that a link is also redeemable by signing in",
    ).toContainText("invitation is redeemed either way");

    const editor = await openProviderEditor(page, PROVIDER_NAME);
    await editor
      .locator('[data-testid$="-issuer-url"]')
      .fill(`${OAUTH_STUB}/realms/thunderforge`);
    await editor
      .locator('[data-testid$="-client-id"]')
      .fill("thunderforge-private");
    await editor
      .locator('[data-testid$="-client-secret"]')
      .fill("private-instance-secret");
    await editor.locator('[data-testid$="-enabled"]').check();
    await editor.locator('[data-testid$="-save"]').click();
    await expect(editor.locator('[data-testid$="-status"]')).toBeVisible({
      timeout: 30_000,
    });

    await page.getByTestId("setup-next").click();

    // 6. The step the operator asked for: anything they meant to set, with
    //    storage among it, and a way to find out now rather than the first
    //    time somebody uploads a map.
    const extras = page.getByTestId("setup-step-settings-anything-else");
    await expect(extras).toBeVisible({ timeout: 30_000 });
    // Each group of extras is a section that stays closed until it is opened.
    await page.getByTestId("setup-extras-toggle-storage").click();
    for (const [key, value] of Object.entries(STORAGE_DEFAULTS)) {
      const field = page.getByTestId(`setup-setting-${key}`);
      if (await field.isVisible().catch(() => false)) {
        await field.fill(value);
      } else {
        // Fixed by the environment, which is the supported deployment shape
        // and must be *shown* as fixed rather than silently absent (FR-009).
        await expect(
          page.getByTestId(`setup-setting-fixed-${key}`),
          `${key} is either askable or declared fixed — never missing`,
        ).toBeVisible();
      }
    }

    await page.getByTestId("setup-storage-probe-run").click();
    await expect(page.getByTestId("setup-storage-probe-result")).toContainText(
      "Reached",
      { timeout: 60_000 },
    );
    await expect(page.getByTestId("setup-storage-probe-failure")).toHaveCount(
      0,
    );

    await walkTo(page, "second-factor");
    await enrolSecondFactor(page);
    await advanceFrom(page, "second-factor");

    await expect(page.getByTestId("setup-step-review")).toBeVisible({
      timeout: 30_000,
    });
    await page.getByTestId("setup-complete").click();
    await expect(page).toHaveURL(/\/admin/, { timeout: 60_000 });

    // 7. FR-011 on the readiness side: a duty this instance declined reports
    //    as not applicable, not as work left undone. The difference matters —
    //    a permanent amber warning for something you do not owe teaches an
    //    operator to ignore the panel.
    const readiness = await graphql<{
      data: {
        instanceReadiness: {
          fullyConfigured: boolean;
          capabilities: {
            key: string;
            available: boolean;
            applicable: boolean;
          }[];
        };
      };
    }>(
      page,
      `
        query {
          instanceReadiness {
            fullyConfigured
            capabilities {
              key
              available
              applicable
            }
          }
        }
      `,
      {},
    );
    const capabilities = readiness.data.instanceReadiness.capabilities;
    const publishing = capabilities.find(
      (c) => c.key === "publish_beyond_world",
    );
    expect(publishing?.applicable, "declined, not unfinished").toBe(false);
    expect(
      capabilities.find((c) => c.key === "store_assets")?.available,
      "the instance this wizard hands back can store a file",
    ).toBe(true);
    expect(
      capabilities.find((c) => c.key === "identify_operator")?.available,
      "the instance can say who runs it",
    ).toBe(true);
    // And the gaps that remain are only the ones this walk declined to answer.
    // `fullyConfigured` is deliberately not asserted true: it is a positive
    // claim over every *applicable* capability, and a private table that
    // skipped mail, lore sync and repository feedback has not made it — which
    // is the correct report, not a defect. What FR-011 is about is that
    // neither publishing capability is among the gaps.
    const gaps = capabilities
      .filter((c) => c.applicable && !c.available)
      .map((c) => c.key);
    expect(
      gaps,
      "a declined duty is never reported as an unfinished one",
    ).not.toContain("publish_beyond_world");
    expect(gaps).not.toContain("publish_terms");

    // 8. And nothing is published, because nothing was promised.
    const legal = await page.request.get("/legal/dmca");
    expect(
      [404, 200].includes(legal.status()),
      "an unpublished notice page is either absent or empty of answers",
    ).toBe(true);
    if (legal.status() === 200) {
      expect(await legal.text()).not.toContain(operatorEmail);
    }

    // 9. The payoff: a link, and a friend who gets in with an account they
    //    already have.
    const invitation = await graphql<{
      data: { createInstanceInvitation: { inviteCode: string } };
    }>(
      page,
      `
        mutation {
          createInstanceInvitation(
            input: { maxUses: 1, expiresInHours: 24, note: "a friend" }
          ) {
            inviteCode
          }
        }
      `,
      {},
    );
    const inviteCode = invitation.data.createInstanceInvitation.inviteCode;
    expect(inviteCode, "an invite-only instance can issue a link").toBeTruthy();

    const friendContext = await browser.newContext();
    const friend = await friendContext.newPage();
    try {
      // Who the stub will say arrived. Set before the handshake, because the
      // product reads userinfo rather than being told who signed in.
      const identity = {
        sub: `friend-${suffix}`,
        email: `friend-${suffix}@thunderforge.org`,
        preferred_username: `friend${suffix}`,
      };
      const control = await friend.request.post(
        `${OAUTH_STUB}/_control/identity`,
        {
          data: { identity },
        },
      );
      expect(control.ok()).toBe(true);

      // An uninvited stranger is refused outright — the policy the operator
      // chose, enforced rather than advertised.
      const uninvited = await friend.request.post(
        "/api/authentication/register",
        {
          data: {
            username: `gatecrasher${suffix}`,
            email: `gatecrasher-${suffix}@thunderforge.org`,
            password: ADMIN_PASSWORD,
          },
        },
      );
      expect(
        uninvited.ok(),
        "invite-only means uninvited registration is refused",
      ).toBe(false);

      await friend.goto(`/invite/${inviteCode}`);
      await expect(friend.getByTestId("invite-register")).toBeVisible({
        timeout: 30_000,
      });

      // The shape the operator asked for: the same link, redeemed by signing
      // in with the provider configured two steps ago.
      await friend.getByTestId(`invite-provider-${PROVIDER_KEY}`).click();
      await friend.waitForLoadState("networkidle");

      // Signed in, and in a world they can be at a table in — asserted
      // through the product's own answer rather than a cookie's presence.
      const me = await graphql<{
        data: { me: { id: string } | null };
      }>(
        friend,
        `
          query {
            me {
              id
            }
          }
        `,
        {},
      );
      expect(
        me.data.me?.id,
        "the invitation was redeemed by the provider sign-in",
      ).toBeTruthy();
    } finally {
      await friendContext.close();
    }

    // 10. And a stranger still cannot read a notice contact that was never
    //     given. Over the anonymous transport, because that is the one a
    //     stranger has.
    await page.context().clearCookies();
    await page.goto("/");
    const published = await graphqlPublic<{
      data: { publishedOperatorValues: Record<string, string | null> | null };
    }>(
      page,
      `query { publishedOperatorValues { operatorName noticeContactEmail } }`,
    );
    const values = published.data?.publishedOperatorValues ?? null;
    expect(
      values?.noticeContactEmail ?? null,
      "a private instance publishes no notice contact",
    ).toBeNull();
    expect(
      values?.operatorName ?? null,
      "an instance strangers cannot join does not tell one who runs it",
    ).toBeNull();
  });
});
