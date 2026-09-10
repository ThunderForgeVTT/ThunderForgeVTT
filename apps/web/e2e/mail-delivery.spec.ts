import { expect, test, type Page } from "@playwright/test";
import {
  openAdminPage,
  readSetting,
  writeSettingOrThrow,
} from "./fixtures/admin";
import {
  clearInbox,
  expectNoMessage,
  inbox,
  mailpitSmtpPort,
  waitForMessage,
} from "./fixtures/mailpit";

/**
 * Spec 040 US4, quickstart Scenario D: an operator configures mail, proves it
 * works, breaks it, clears it, and finds that nothing was thrown away.
 *
 * # The level this file exists to add
 *
 * There are three levels of mail proof and this is the third
 * (contracts/e2e-fixtures.md). `mail/outbox_tests.rs` proves the state machine
 * against a capturing transport; `mail/smtp_integration_tests.rs` proves
 * `lettre` against a real Mailpit. Neither proves the sentence FR-013 actually
 * makes, which is about a person: *configure mail in the admin screens, press
 * a button, and the message arrives.* Every step below therefore goes through
 * the interface, and the message is read back out of a real SMTP server.
 *
 * # Why the settings are put back
 *
 * `mail.*` is global to the shard, and two other specs assert this instance
 * **cannot** send — `instance-mail.spec.ts` says so in its first, deliberately
 * load-bearing assertion. If this file left mail configured, those would fail
 * with a story about mail that was really a story about test ordering. So it
 * restores in `afterAll`, which Playwright runs whether or not the tests
 * passed, and the file is serial so no two tests hold different opinions about
 * the same key at once.
 *
 * # No body is ever read
 *
 * Not from the panel, which has no field for one, and not from Mailpit, which
 * would build the habit the product refuses. What is asserted is that a
 * message arrived, to whom, and — for the instance's own test message alone —
 * its subject.
 */

test.describe.configure({ mode: "serial" });

/** Every key this file writes, so restoring is a list and not a memory. */
const MAIL_KEYS = [
  "mail.enabled",
  "mail.host",
  "mail.port",
  "mail.security",
  "mail.from_address",
  "mail.from_name",
  "mail.password",
] as const;

/**
 * `example.org`, not `.test`.
 *
 * `Validator::NoReservedTld` refuses `.local`, `.example`, `.invalid` and
 * `.test` — correctly, since mail sent there never arrives. `example.org` is
 * reserved by the same RFC and is equally undeliverable, but the validator
 * does not refuse it, and the rest of the harness already relies on that. It
 * costs nothing here: the message never leaves this machine, because the only
 * SMTP server involved is this shard's own sink.
 */
const FROM_ADDRESS = "no-reply@thunderforge-e2e.example.org";
const RECIPIENT = "operator@thunderforge-e2e.example.org";
/** Planted so that a screen or a log leaking it is a failing test, not a review. */
const SMTP_PASSWORD = "e2e-smtp-password-9f2ab7c4";

/**
 * A port nothing listens on, for the step that breaks delivery deliberately.
 *
 * It was `smtpPort + 1` for one run, which is the *next shard's* Mailpit — a
 * real, working SMTP server. The "broken" configuration delivered, and the
 * test failed only because two shards happened to be running. A neighbour's
 * port is not an absence, and the ports this harness hands out are contiguous
 * by design.
 */
const CLOSED_PORT = 65_533;

let admin: Page;
let smtpPort: number;
const original: Record<string, string | null> = {};

/**
 * Write one setting through the panel, the way a person does.
 *
 * Not through `writeSetting`: that is the API this file exists to *stop*
 * standing in for the interface. The kinds are handled here rather than in the
 * spec bodies because a port is a number box and a security mode is a select,
 * and a test that typed into the wrong one would fail for a reason that has
 * nothing to do with mail.
 */
async function setThroughPanel(key: string, value: string): Promise<void> {
  const field = admin.getByTestId(`setup-setting-${key}`);
  await expect(field).toBeVisible({ timeout: 20_000 });

  const tag = await field.evaluate((element) => element.tagName.toLowerCase());
  const type = await field.getAttribute("type");

  if (tag === "select") {
    await field.selectOption(value);
  } else if (type === "checkbox") {
    await field.setChecked(value === "true");
  } else {
    await field.fill(value);
  }

  await admin.getByTestId(`instance-setting-save-${key}`).click();
  await expect(admin.getByTestId(`instance-setting-${key}`)).toContainText(
    "Saved.",
    { timeout: 20_000 },
  );
}

/** Configure this instance to talk to the shard's own Mailpit. */
async function configureWorkingMail(port: number): Promise<void> {
  await admin.goto("/admin/instance");
  await expect(admin.getByTestId("instance-settings-panel")).toBeVisible({
    timeout: 20_000,
  });
  await setThroughPanel("mail.host", "127.0.0.1");
  await setThroughPanel("mail.port", String(port));
  // Mailpit's SMTP listener is plaintext, which is what `none` is for.
  await setThroughPanel("mail.security", "none");
  await setThroughPanel("mail.from_address", FROM_ADDRESS);
  await setThroughPanel("mail.enabled", "true");
}

async function sendTestTo(to: string): Promise<void> {
  await admin.goto("/admin/mail");
  await expect(admin.getByTestId("mail-panel")).toBeVisible({
    timeout: 20_000,
  });
  await admin.getByTestId("mail-test-to").fill(to);
  await admin.getByTestId("mail-test-send").click();
  await expect(admin.getByTestId("mail-test-result")).toBeVisible({
    timeout: 30_000,
  });
}

test.beforeAll(async ({ browser }) => {
  admin = await openAdminPage(browser);
  smtpPort = mailpitSmtpPort();
  for (const key of MAIL_KEYS) {
    original[key] = (await readSetting(admin, key)).value;
  }
  await clearInbox();
});

test.afterAll(async () => {
  if (!admin) return;
  for (const key of MAIL_KEYS) {
    await writeSettingOrThrow(admin, key, original[key]);
  }
  await clearInbox();
  await admin.context().close();
});

test.describe("Spec 040 Scenario D: mail, end to end", () => {
  test("mail configured in the admin screens delivers a test message to a real SMTP server", async () => {
    await configureWorkingMail(smtpPort);
    await sendTestTo(RECIPIENT);

    await expect(admin.getByTestId("mail-test-result")).toHaveAttribute(
      "data-delivered",
      "true",
    );

    // The only assertion that settles it: a real SMTP server accepted it.
    const message = await waitForMessage(RECIPIENT);
    expect(message.From?.Address).toBe(FROM_ADDRESS);
    expect(message.Subject).toContain("mail is configured");
  });

  test("the outbox shows the message sent, with no body anywhere on the screen", async () => {
    await admin.goto("/admin/mail");
    const panel = admin.getByTestId("mail-panel");
    await expect(panel).toBeVisible({ timeout: 20_000 });

    await expect(admin.getByTestId("mail-availability")).toHaveAttribute(
      "data-ready",
      "true",
    );

    const entry = panel.locator('[data-testid^="outbox-entry-"]').first();
    await expect(entry).toHaveAttribute("data-state", "SENT");
    await expect(entry).toContainText(RECIPIENT);

    // FR-016. The body of the test message is a fixed string in
    // `mail/graphql.rs`; if any of it reached the screen, the screen has a
    // body field it should not have.
    await expect(panel).not.toContainText("This is a test message from a");
    await expect(panel).not.toContainText("no action is needed");
  });

  test("a broken port fails with something to act on, and the password appears nowhere", async () => {
    // A password that is actually set, so there is something real to leak.
    await admin.goto("/admin/instance");
    await setThroughPanel("mail.password", SMTP_PASSWORD);
    await setThroughPanel("mail.port", String(CLOSED_PORT));

    await clearInbox();
    await sendTestTo(RECIPIENT);

    const result = admin.getByTestId("mail-test-result");
    await expect(result).toHaveAttribute("data-delivered", "false");
    // The failure has to be actionable — a bare "failed" is FR-014's whole
    // complaint. It names the connection, not the credentials.
    await expect(result).not.toHaveText("");

    // SC-004: the check is the attempt to find it. The whole rendered page,
    // not just the panel — a leak into a tooltip or a hidden field counts.
    const page = (await admin.locator("body").innerText()).toLowerCase();
    expect(page).not.toContain(SMTP_PASSWORD.toLowerCase());
    // `innerText` does not include an input's value, so the fields are read
    // separately rather than assumed covered by the sweep above.
    const values = await admin
      .locator("input")
      .evaluateAll((nodes) =>
        nodes.map((node) => (node as HTMLInputElement).value).join(" "),
      );
    expect(values.toLowerCase()).not.toContain(SMTP_PASSWORD.toLowerCase());

    await expectNoMessage(RECIPIENT);
  });

  test("with mail cleared, readiness says what is missing and a message is held rather than discarded", async () => {
    await admin.goto("/admin/instance");
    await setThroughPanel("mail.enabled", "false");
    // Cleared, not merely disabled: step 6 says every `mail.*` setting.
    for (const key of ["mail.host", "mail.from_address", "mail.password"]) {
      await admin.getByTestId(`instance-setting-clear-${key}`).click();
      await expect(admin.getByTestId(`instance-setting-${key}`)).toContainText(
        "Cleared.",
        { timeout: 20_000 },
      );
    }

    await admin.goto("/admin/readiness");
    const capability = admin.getByTestId("readiness-capability-send_mail");
    await expect(capability).toBeVisible({ timeout: 20_000 });
    await expect(capability).toHaveAttribute("data-available", "false");
    await expect(
      capability.getByTestId("readiness-gap-mail.host"),
    ).toBeVisible();

    await clearInbox();
    await sendTestTo(RECIPIENT);
    await expect(admin.getByTestId("mail-test-result")).toHaveAttribute(
      "data-delivered",
      "false",
    );

    // FR-015: held and recorded, never discarded — and never delivered.
    const entry = admin.locator('[data-testid^="outbox-entry-"]').first();
    await expect(entry).toHaveAttribute("data-state", "BLOCKED");
    await expectNoMessage(RECIPIENT);
  });

  test("reconfiguring releases the held message without anybody asking it to", async () => {
    await configureWorkingMail(smtpPort);

    // Nothing is pressed here on purpose. `spawn_mail_task` ticks every 15
    // seconds and releases what mail being unavailable had held; the operator
    // fixed the settings and that is all they should have to do. Waiting for
    // the sink rather than for the panel is what makes this a claim about
    // delivery instead of about a state column.
    const message = await waitForMessage(RECIPIENT, { timeoutMs: 60_000 });
    expect(message.From?.Address).toBe(FROM_ADDRESS);

    // And exactly one — a release that re-sent the earlier attempts would be
    // an operator's inbox full of duplicates.
    const delivered = (await inbox()).filter((m) =>
      m.To.some((t) => t.Address.toLowerCase() === RECIPIENT.toLowerCase()),
    );
    expect(delivered).toHaveLength(1);
  });
});
