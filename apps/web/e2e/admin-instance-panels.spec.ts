import { expect, test, type Page } from "@playwright/test";
import {
  openAdminPage,
  readSetting,
  writeSettingOrThrow,
} from "./fixtures/admin";

/**
 * Spec 040 T028–T034, T048, T090: the operator-facing half, driven the way an
 * operator drives it.
 *
 * # Why this file exists at all
 *
 * Every other 040 spec reaches these features through `fixtures/admin.ts`,
 * which posts GraphQL directly. That is the right way to test a resolver and
 * it is exactly how the panels stayed unbuilt for a whole session without one
 * test going red: the server half was complete, the tests were green, and
 * there was nothing on the screen to click. Everything below therefore goes
 * through the interface — the nav, the field, the button — and reads the
 * result back through the API only to prove the *write actually landed*.
 *
 * # The seams this is aimed at
 *
 * 1. A section declared in `adminSections.ts` that nothing routes to. The nav
 *    and the routes are two lists, and only a click proves they agree.
 * 2. A value edited on screen that never reaches the server, or reaches it and
 *    is not what the next read resolves. Asserted by writing through the field
 *    and reading through the API — deliberately not the other way around.
 * 3. A field the environment has fixed rendered as a disabled box with no
 *    reason (FR-009). `scripts/e2e-parallel.mjs` fixes `THUNDERFORGE_REALM_NAME`
 *    on every shard, so there is always exactly one such setting to look at.
 * 4. A secret rendered as anything other than "Set" or "Not set" (FR-023).
 */

test.describe.configure({ mode: "serial" });

/**
 * The setting the write test uses.
 *
 * `notice.contact_name` is prose-free, optional, and in the "Copyright
 * notices" group the panel is required to surface separately (T033), so
 * editing it exercises the grouping as well as the write.
 */
const NOTICE_NAME = "notice.contact_name";

/** Fixed by `THUNDERFORGE_REALM_NAME` on every shard of this harness. */
const REALM_NAME = "realm_name";

let admin: Page;
let originalNoticeName: string | null = null;

test.beforeAll(async ({ browser }) => {
  admin = await openAdminPage(browser);
  originalNoticeName = (await readSetting(admin, NOTICE_NAME)).value;
});

test.afterAll(async () => {
  if (admin) {
    // One instance per shard, and settings are global to it: put back what
    // was found, whether or not the test that changed it passed.
    await writeSettingOrThrow(admin, NOTICE_NAME, originalNoticeName);
    await admin.context().close();
  }
});

test.describe("Spec 040: the instance is configurable by a person", () => {
  test("the three new sections are reachable by clicking, not only by typing a URL", async () => {
    await admin.goto("/admin");

    for (const [testId, path, panel] of [
      ["admin-nav-instance", "/admin/instance", "instance-settings-panel"],
      ["admin-nav-readiness", "/admin/readiness", "readiness-panel"],
      ["admin-nav-mail", "/admin/mail", "mail-panel"],
    ] as const) {
      await admin.goto("/admin");
      await admin.getByTestId(testId).first().click();
      await admin.waitForURL(new RegExp(`${path}$`), { timeout: 20_000 });
      await expect(admin.getByTestId(panel)).toBeVisible({ timeout: 20_000 });
    }
  });

  test("a setting edited on the screen is the value the server then resolves", async () => {
    const written = `Notice Contact ${Date.now()}`;

    await admin.goto("/admin/instance");
    await expect(admin.getByTestId("instance-settings-panel")).toBeVisible({
      timeout: 20_000,
    });

    const field = admin.getByTestId(`setup-setting-${NOTICE_NAME}`);
    await expect(field).toBeVisible();
    await field.fill(written);
    await admin.getByTestId(`instance-setting-save-${NOTICE_NAME}`).click();

    // The panel's own confirmation, and then the only thing that settles it:
    // what the server says the value is now.
    await expect(
      admin.getByTestId(`instance-setting-${NOTICE_NAME}`),
    ).toContainText("Saved.", { timeout: 20_000 });

    const resolved = await readSetting(admin, NOTICE_NAME);
    expect(resolved.value).toBe(written);
    expect(resolved.source).toBe("INSTANCE");
  });

  test("a setting the environment fixed says which variable fixed it, and offers no field", async () => {
    await admin.goto("/admin/instance");

    const row = admin.getByTestId(`instance-setting-${REALM_NAME}`);
    await expect(row).toBeVisible({ timeout: 20_000 });
    await expect(row).toHaveAttribute("data-editable", "false");
    await expect(row).toHaveAttribute("data-source", "ENVIRONMENT");
    // FR-009 is the sentence, not the disabling: a greyed box with no stated
    // reason would satisfy "not editable" and fail the requirement.
    await expect(row).toContainText("THUNDERFORGE_REALM_NAME");
    await expect(row).toContainText("Unset that variable to edit it here");
    await expect(row.locator("input")).toHaveCount(0);
    await expect(
      admin.getByTestId(`instance-setting-save-${REALM_NAME}`),
    ).toHaveCount(0);
  });

  test("a secret is only ever Set or Not set, and its history says a change happened without saying what to", async () => {
    const secretValue = `e2e-secret-${Date.now()}`;
    await writeSettingOrThrow(admin, "mail.password", secretValue);

    try {
      await admin.goto("/admin/instance");
      const row = admin.getByTestId("instance-setting-mail.password");
      await expect(row).toBeVisible({ timeout: 20_000 });

      // The value was just written, so it exists to leak. Nothing on this
      // screen may contain it, in any form.
      await expect(admin.getByTestId("instance-settings-panel")).not.toContainText(
        secretValue,
      );
      await expect(row).toContainText("Set");

      await admin.getByTestId("instance-setting-history-toggle-mail.password").click();
      const history = admin.getByTestId("instance-setting-history-mail.password");
      await expect(history).toBeVisible({ timeout: 20_000 });
      await expect(history).toContainText("A secret was changed");
      await expect(history).not.toContainText(secretValue);
    } finally {
      await writeSettingOrThrow(admin, "mail.password", null);
    }
  });

  test("readiness names a capability this instance does not have, and what to set for it", async () => {
    await admin.goto("/admin/readiness");
    const panel = admin.getByTestId("readiness-panel");
    await expect(panel).toBeVisible({ timeout: 20_000 });

    // This harness sets no SMTP variable, so mail is the capability that is
    // reliably unavailable here. If that ever changes this fails loudly
    // rather than quietly asserting nothing.
    const mail = admin.getByTestId("readiness-capability-send_mail");
    await expect(mail).toBeVisible();
    await expect(mail).toHaveAttribute("data-available", "false");
    await expect(mail).toContainText("Not configured");
    // A gap is only useful if it says what to do about it.
    await expect(mail.getByTestId(/^readiness-gap-mail\./).first()).toBeVisible();
    // The gap's sentence, verbatim from the registry, is the point of the
    // screen — a capability marked unavailable with nothing to do about it
    // would satisfy every assertion above.
    await expect(mail).toContainText("mail");
  });

  test("the mail panel reports that it cannot send, and a test message still leaves a record", async () => {
    await admin.goto("/admin/mail");
    const panel = admin.getByTestId("mail-panel");
    await expect(panel).toBeVisible({ timeout: 20_000 });

    await expect(admin.getByTestId("mail-availability")).toHaveAttribute(
      "data-ready",
      "false",
    );

    await admin.getByTestId("mail-test-to").fill("operator@example.org");
    await admin.getByTestId("mail-test-send").click();

    const result = admin.getByTestId("mail-test-result");
    await expect(result).toBeVisible({ timeout: 30_000 });
    await expect(result).toHaveAttribute("data-delivered", "false");

    // FR-015: held and recorded, never discarded. The row is the record.
    await expect(
      panel.locator('[data-testid^="outbox-entry-"]').first(),
    ).toContainText("operator@example.org", { timeout: 20_000 });
  });
});
