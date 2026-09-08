import { expect, test, type Page } from "@playwright/test";
import { openAdminPage, type GqlResult } from "./fixtures/admin";
import { freshCredentials, graphql, register } from "./fixtures/helpers";

/**
 * Spec 040 US4: whether this instance can send mail, and what happens to a
 * message when it cannot.
 *
 * # This stack has no mail transport, and that is what is asserted
 *
 * `scripts/e2e-parallel.mjs` sets no `THUNDERFORGE_SMTP_*` variable and the
 * seeded database holds no `mail.*` row, so `MailSeam::transport` builds
 * `mail::Unconfigured`. Rather than skip US4 for want of an SMTP server, this
 * file asserts the behaviour an unconfigured instance is *required* to have —
 * which is the state most instances are in on their first day, and the state
 * FR-015 was written for: a message is held and recorded, never discarded,
 * and the record says what to set.
 *
 * The first assertion in the first test is therefore load-bearing: if this
 * stack ever gains a transport, that test fails loudly instead of quietly
 * proving nothing.
 *
 * # What is deliberately not here
 *
 * Successful delivery. It needs a real SMTP server; `mail/smtp_integration_tests.rs`
 * covers it against Mailpit, and this harness has no mail catcher.
 */

const MAIL_AVAILABILITY = `
  query MailAvailability {
    mailAvailability { ready missing limitedFeatures }
  }
`;

const SEND_TEST_MAIL = `
  mutation SendTestMail($to: String!) {
    sendTestMail(to: $to) { delivered reason outboxId }
  }
`;

const MAIL_OUTBOX = `
  query MailOutbox($state: OutboxState, $limit: Int) {
    mailOutbox(state: $state, limit: $limit) {
      id
      purpose
      toAddress
      subject
      state
      attempts
      lastFailureReason
      sentAt
    }
  }
`;

interface Availability {
  ready: boolean;
  missing: string[];
  limitedFeatures: string[];
}

interface OutboxEntry {
  id: string;
  purpose: string;
  toAddress: string;
  subject: string | null;
  state: string;
  attempts: number;
  lastFailureReason: string | null;
  sentAt: string | null;
}

let admin: Page;

async function availability(page: Page): Promise<Availability> {
  const result = await graphql<GqlResult<{ mailAvailability: Availability }>>(
    page,
    MAIL_AVAILABILITY,
    {},
  );
  if (!result.data?.mailAvailability) {
    throw new Error(
      `mailAvailability did not answer: ${JSON.stringify(result.errors ?? result)}`,
    );
  }
  return result.data.mailAvailability;
}

async function outbox(page: Page, state?: string): Promise<OutboxEntry[]> {
  const result = await graphql<GqlResult<{ mailOutbox: OutboxEntry[] }>>(
    page,
    MAIL_OUTBOX,
    { state: state ?? null, limit: 50 },
  );
  return result.data?.mailOutbox ?? [];
}

test.describe.configure({ mode: "serial" });

test.beforeAll(async ({ browser }) => {
  admin = await openAdminPage(browser);
});

test.afterAll(async () => {
  await admin?.context().close();
});

test.describe("Spec 040 US4: mail on an instance that has none", () => {
  test("availability says it cannot send, and names settings rather than values", async () => {
    const state = await availability(admin);

    expect(
      state.ready,
      "this harness configures no SMTP transport — if that has changed, every assertion below is about the wrong instance",
    ).toBe(false);

    // `mail.enabled` first, because a mail server that is fully described but
    // switched off is a deliberate state and the operator has to be told which
    // of the two they are in.
    expect(state.missing).toContain("mail.enabled");
    expect(state.missing).toContain("mail.host");
    // Setting **keys**, never a host, a credential or a fragment of one
    // (FR-027). Asserted by shape so a value smuggled in as a "helpful hint"
    // fails here.
    for (const key of state.missing) {
      expect(key, "a missing entry must be a declared setting key").toMatch(
        /^mail\.[a-z_]+$/,
      );
    }

    // FR-025 scenario 3: what is limited, in sentences a person can act on,
    // and specifically the promise FR-015 makes about held messages.
    expect(state.limitedFeatures.length).toBeGreaterThan(0);
    expect(state.limitedFeatures.join(" ")).toContain("Nothing is discarded");
  });

  test("a test message is recorded rather than discarded, and says what is missing", async () => {
    const to = `operator-${Date.now().toString(36)}@example.invalid`;

    const sent = await graphql<
      GqlResult<{
        sendTestMail: {
          delivered: boolean;
          reason: string | null;
          outboxId: string;
        };
      }>
    >(admin, SEND_TEST_MAIL, { to });

    expect(
      sent.errors,
      "an unconfigured instance answers rather than failing",
    ).toBeFalsy();
    const result = sent.data!.sendTestMail;

    // Not delivered, and it says so plainly rather than reporting success for
    // a message that went nowhere.
    expect(result.delivered).toBe(false);
    expect(result.reason ?? "").toContain("no mail server configured");
    // The refusal names the keys to fill in — `mail::Unconfigured::reason`
    // quotes them, which is what makes FR-015 actionable rather than merely
    // truthful.
    expect(result.reason ?? "").toContain("`mail.host`");
    expect(result.outboxId).toBeTruthy();

    // FR-015, the half a unit test cannot show: the row survives the request
    // and an operator can find it afterwards.
    const blocked = await outbox(admin, "BLOCKED");
    const row = blocked.find((entry) => entry.id === result.outboxId);
    expect(
      row,
      "an instance that cannot send must hold the message, not drop it",
    ).toBeTruthy();
    expect(row!.toAddress).toBe(to);
    // `blocked` rather than `failed`: "we cannot yet" and "we tried and gave
    // up" are different answers, and only one of them is released by
    // configuring mail.
    expect(row!.state).toBe("BLOCKED");
    expect(row!.sentAt).toBeNull();
    expect(row!.lastFailureReason ?? "").toContain("`mail.host`");
    // The instance's own test message is the one whose subject is
    // disclosable — a subject about a person would be content.
    expect(row!.purpose).toBe("test");
    expect(row!.subject ?? "").toContain("ThunderForge");
  });

  test("something that is not an address is refused before a row exists for it", async () => {
    const before = await outbox(admin);

    const refused = await graphql<GqlResult<{ sendTestMail: unknown }>>(
      admin,
      SEND_TEST_MAIL,
      { to: "operator-at-nowhere" },
    );
    expect(refused.data?.sendTestMail).toBeFalsy();
    expect(refused.errors?.[0]?.message ?? "").toContain("email address");

    // And nothing was written. A refusal that still enqueued would leave an
    // operator an outbox full of rows nobody asked to send.
    const after = await outbox(admin);
    expect(after.map((entry) => entry.toAddress)).not.toContain(
      "operator-at-nowhere",
    );
    expect(after.length).toBe(before.length);
  });

  test("mail is an administrator's surface and nobody else's", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2email"));

    const read = await graphql<GqlResult<{ mailAvailability: unknown }>>(
      page,
      MAIL_AVAILABILITY,
      {},
    );
    expect(read.errors?.[0]?.message ?? "").toContain(
      "Admin privileges required",
    );
    expect(read.data?.mailAvailability).toBeFalsy();

    const listed = await graphql<GqlResult<{ mailOutbox: unknown }>>(
      page,
      MAIL_OUTBOX,
      { state: null, limit: 5 },
    );
    expect(listed.errors?.[0]?.message ?? "").toContain(
      "Admin privileges required",
    );

    // The one that matters most: `sendTestMail` sends a message to an
    // arbitrary address on request, so without this it is an open relay with
    // a login page.
    const sent = await graphql<GqlResult<{ sendTestMail: unknown }>>(
      page,
      SEND_TEST_MAIL,
      { to: "stranger@example.invalid" },
    );
    expect(sent.data?.sendTestMail).toBeFalsy();
    expect(sent.errors?.[0]?.message ?? "").toContain(
      "Admin privileges required",
    );

    // Refused before anything was written, not after.
    expect((await outbox(admin)).map((entry) => entry.toAddress)).not.toContain(
      "stranger@example.invalid",
    );
  });
});
