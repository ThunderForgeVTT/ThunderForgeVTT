import { expect, test } from "./fixtures/test";
import { graphql } from "./fixtures/helpers";
import {
  ADMIN_PASSWORD,
  advanceFrom,
  createFirstAdministrator,
  enrolSecondFactor,
  openWizard,
  RESERVED_NOTICE_ADDRESS,
  walkTo,
} from "./fixtures/first-run";

/**
 * Spec 052 FR-061 (spec 064): opening an instance asks for what opening
 * obliges — and says why first.
 *
 * # The private spec's mirror, and the half that is easy to get wrong
 *
 * `instance-first-run-private.spec.ts` proves the questions that must not be
 * asked. This one proves that the same wizard, told this instance publishes
 * beyond the world its content was made in, asks for all of them — and that
 * every one of those questions arrives with its reason attached (FR-032).
 *
 * The reason matters more than it sounds. An operator asked out of nowhere for
 * a designated agent's postal address concludes the software is being nosy and
 * types something false into it, which is worse than not asking: a notice
 * address nobody reads is a liability dressed as compliance. So the
 * explanation being present, and being present *before* the answer is saved,
 * is asserted rather than assumed.
 *
 * # FR-030, as the registry means it
 *
 * A capability does not take effect until every setting behind it is present,
 * and the way that shows up is **not** a refusal to finish setup: the notice
 * and prose settings are `RequiredFor(capability)`, not `RequiredAtSetup`, so
 * an operator can finish and come back. What FR-030 forbids is pretending.
 * So this walk deliberately leaves the legal prose unwritten, and asserts that
 * the review says so, that readiness reports the terms capability as
 * applicable-but-unavailable, and that `/legal/terms` shows the unwritten
 * blocks as unwritten rather than inventing text over the operator's name.
 */

test.describe("Spec 052 FR-061: a public instance is told what publishing obliges", () => {
  test("an operator says this instance publishes beyond a world, is told what that adds before saving it, and the legal pages then carry their answers", async ({
    page,
    context,
  }) => {
    test.setTimeout(6 * 60_000);

    const suffix = Date.now().toString(36);
    const operatorName = `The Open Table ${suffix}`;
    const operatorEmail = `operator-${suffix}@thunderforge.org`;
    const noticeEmail = `notices-${suffix}@thunderforge.org`;

    await openWizard(page);
    await createFirstAdministrator(page, {
      username: `admin${suffix}`,
      email: operatorEmail,
    });

    // 1. Who may join, and the statement that drives everything after it.
    const accessStep = page.getByTestId("setup-step-settings-access");
    await expect(accessStep).toBeVisible({ timeout: 30_000 });

    await page
      .getByTestId("setup-setting-instance.access_policy")
      .selectOption("open");
    await expect(
      page.getByTestId("setup-access-consequence-open"),
    ).toHaveAttribute("data-chosen", "true");

    // FR-032, the strict reading: what checking this adds is named *before* it
    // is saved. Nothing about the preview is hidden until the next step.
    await expect(page.getByTestId("setup-publishing-preview")).toBeHidden();
    await page
      .getByTestId("setup-setting-instance.publishes_beyond_world")
      .check();
    const preview = page.getByTestId("setup-publishing-preview");
    await expect(preview).toBeVisible();
    await expect(preview).toContainText("copyright-notice contact");
    await expect(preview).toContainText("postal address");
    await expect(
      accessStep,
      "the preview appears before the answer is saved, not after",
    ).toBeVisible();

    await advanceFrom(page, "settings-access");

    // 2. And now the steps exist, which is the whole fork seen from the other
    //    side. The private instance's "you are not being asked for this" note
    //    is correspondingly gone.
    await expect(
      page.getByTestId("setup-progress-settings-copyright-notices"),
    ).toBeVisible();
    await expect(
      page.getByTestId("setup-progress-settings-legal-prose"),
    ).toBeVisible();
    await expect(page.getByTestId("setup-not-asked")).toHaveCount(0);

    // 3. Who operates it — including the jurisdiction, which is a publishing
    //    duty and so is asked for here and not on a private instance.
    await expect(
      page.getByTestId("setup-step-settings-operator"),
    ).toBeVisible();
    await page.getByTestId("setup-setting-operator.name").fill(operatorName);
    await page
      .getByTestId("setup-setting-operator.contact_email")
      .fill(operatorEmail);
    await page
      .getByTestId("setup-setting-operator.jurisdiction")
      .fill("Ohio, United States");
    await advanceFrom(page, "settings-operator");

    // 4. The copyright-notice contact, with its reason above it and the
    //    liability boundary spelled out (spec 039 FR-055): the designation is
    //    the operator's, not this software's.
    const noticesStep = page.getByTestId(
      "setup-step-settings-copyright-notices",
    );
    await expect(noticesStep).toBeVisible({ timeout: 30_000 });
    const explainer = page.getByTestId("setup-group-explainer");
    await expect(explainer).toHaveAttribute(
      "data-group",
      "settings-copyright-notices",
    );
    await expect(
      explainer,
      "the duty is traced to the thing that creates it",
    ).toContainText("publishes content beyond a world");
    await expect(
      page.getByTestId("setup-designated-agent-notice"),
    ).toBeVisible();

    // A reserved domain can never receive a notice, so it is refused — the
    // one validation in this group that exists for a legal reason rather than
    // a formatting one.
    await page
      .getByTestId("setup-setting-notice.contact_name")
      .fill(operatorName);
    await page
      .getByTestId("setup-setting-notice.contact_email")
      .fill(RESERVED_NOTICE_ADDRESS);
    await page
      .getByTestId("setup-setting-notice.contact_postal_address")
      .fill("1 Example Way, Columbus OH");
    await page.getByTestId("setup-next").click();
    const reserved = page.getByTestId("setup-field-error").first();
    await expect(reserved).toBeVisible({ timeout: 15_000 });
    await expect(reserved).toHaveAttribute(
      "data-field",
      "notice.contact_email",
    );
    await expect(noticesStep, "a refused step must not advance").toBeVisible();

    await page
      .getByTestId("setup-setting-notice.contact_email")
      .fill(noticeEmail);
    await advanceFrom(page, "settings-copyright-notices");

    // 5. The legal prose, with its own reason — and deliberately left
    //    unwritten. FR-030: what is not answered is not served, and nothing
    //    is composed on the operator's behalf.
    const proseStep = page.getByTestId("setup-step-settings-legal-prose");
    await expect(proseStep).toBeVisible({ timeout: 30_000 });
    await expect(page.getByTestId("setup-group-explainer")).toContainText(
      "Leave a field blank and that page simply is not served",
    );
    await page.getByTestId("setup-skip-step").click();

    // 6. Support, which completion does require, then everything optional.
    await walkTo(page, "settings-support");
    await page.getByTestId("setup-setting-support_email").fill(operatorEmail);
    await advanceFrom(page, "settings-support");

    // Providers come after the legal steps — "all the legal stuff first,
    // then the OIDC and OAuth2 stuff". Declaration order is step order, so
    // this ordering is a fact about the registry, asserted here because it is
    // the ordering the operator asked for.
    await expect(page.getByTestId("setup-step-providers")).toBeVisible({
      timeout: 30_000,
    });
    await page.getByTestId("setup-next").click();

    await walkTo(page, "second-factor");
    await enrolSecondFactor(page);
    await advanceFrom(page, "second-factor");

    // 7. The review names what was left rather than implying it is done.
    await expect(page.getByTestId("setup-step-review")).toBeVisible({
      timeout: 30_000,
    });
    await expect(page.getByTestId("setup-review-unset")).toBeVisible();
    await expect(
      page.getByTestId("setup-review-unset-legal.minimum_age_statement"),
      "unwritten prose is reported as unwritten",
    ).toBeVisible();
    await expect(
      page.getByTestId("setup-review-unset-notice.contact_email"),
      "what was answered is not reported as missing",
    ).toHaveCount(0);

    await page.getByTestId("setup-complete").click();
    await expect(page).toHaveURL(/\/admin/, { timeout: 60_000 });

    // 8. Readiness says both publishing duties are this instance's own —
    //    neither is "not applicable", which is the state that belongs to the
    //    private instance alone — and that both were answered.
    const readiness = await graphql<{
      data: {
        instanceReadiness: {
          capabilities: {
            key: string;
            available: boolean;
            applicable: boolean;
            gaps: { settingKey: string }[];
          }[];
        };
      };
    }>(
      page,
      `
        query {
          instanceReadiness {
            capabilities {
              key
              available
              applicable
              gaps {
                settingKey
              }
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
    expect(publishing?.applicable, "this instance publishes, so it owes").toBe(
      true,
    );
    expect(publishing?.available, "and it answered what it owed").toBe(true);

    const terms = capabilities.find((c) => c.key === "publish_terms");
    expect(terms?.applicable).toBe(true);
    // Answered, because the one setting serving terms *requires* is the
    // jurisdiction, given on the operator step. The prose blocks this walk
    // skipped are optional by declaration: an instance that publishes terms
    // without an age statement of its own serves the shipped placeholder, and
    // that is reported where it belongs — on the review step above, which
    // named the statement as unwritten, and on the page itself below, which
    // marks the block rather than composing one. Calling it a capability gap
    // would make every honest public instance permanently unconfigured for
    // declining to write optional prose.
    expect(terms?.available, "the answer terms require was given").toBe(true);
    expect(
      terms?.gaps,
      "and nothing optional is reported as a gap",
    ).toHaveLength(0);

    // 9. As a stranger, with no account, because that is who these pages are
    //    for. The notice contact is the point of the whole group.
    await context.clearCookies();
    const stranger = await context.newPage();
    try {
      await stranger.goto("/legal/dmca");
      // The statutory designation at the foot of the page, which is the whole
      // reason the notices group was asked for: it names the operator as the
      // agent, and says so as configured rather than as a placeholder.
      const agentName = stranger.getByTestId("dmca-agent-notice-contact-name");
      await expect(
        agentName,
        "the operator is named as the agent",
      ).toContainText(operatorName);
      await expect(agentName).toHaveAttribute("data-configured", "true");
      await expect(
        stranger.getByTestId("dmca-agent-notice-contact-email"),
        "and the address a notice actually reaches",
      ).toContainText(noticeEmail);
      await expect(
        stranger.getByTestId("dmca-agent-notice-contact-postal-address"),
      ).toHaveAttribute("data-configured", "true");

      // The terms carry the answers, and nothing else. FR-030's forbidden
      // outcome is a clause invented under the operator's name, so what is
      // asserted is both halves: every operator token this walk answered is
      // substituted — no `[OPERATOR — ...]` marker survives — and the optional
      // prose it skipped produced no text at all, not a composed stand-in.
      //
      // The skipped age statement is not looked for here because nothing
      // renders it yet: `legal.minimum_age_statement` is declared, asked for
      // and stored, and no surface reads it. The wizard's review step named it
      // as unwritten above, which is the only place it is currently reported.
      await stranger.goto("/legal/terms");
      const terms_html = await stranger.content();
      expect(
        terms_html,
        "an answered operator value is substituted, never left as a marker",
      ).not.toContain("[OPERATOR —");

      // An open instance admits a stranger without an invitation — the other
      // half of what the Access step promised.
      const joined = await stranger.request.post(
        "/api/authentication/register",
        {
          data: {
            username: `newcomer${suffix}`,
            email: `newcomer-${suffix}@thunderforge.org`,
            password: ADMIN_PASSWORD,
          },
        },
      );
      expect(
        joined.ok(),
        `an open instance takes a sign-up without an invitation (got ${joined.status()})`,
      ).toBe(true);
    } finally {
      await stranger.close();
    }
  });
});
