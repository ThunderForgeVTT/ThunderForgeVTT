import { expect, test, type Page } from "./fixtures/test";
import {
  openAdminPage,
  readSetting,
  writeSettingOrThrow,
} from "./fixtures/admin";

/**
 * Spec 088 US7: the mail page is one form.
 *
 * - Save is off while nothing has changed (FR-052).
 * - Two changed keys are two `updateInstanceSetting` requests, counted on the
 *   wire, and nothing is sent for the keys left alone (SC-006).
 * - Leaving through the settings page's own section links asks first while
 *   anything is unsaved, and does not ask once it is saved (FR-054).
 * - A key the server refuses stays dirty, with the server's reason beside it,
 *   and the page says how many of how many were saved (FR-053).
 *
 * The keys it writes are put back in `afterAll`, through the API, so the mail
 * specs beside it start from what the stack had.
 */

test.describe.configure({ mode: "serial" });

const KEYS = ["mail.host", "mail.from_name", "mail.from_address"] as const;

let admin: Page;
const original: Record<string, string | null> = {};
let mutations: string[] = [];

async function gotoMail(): Promise<void> {
  await admin.goto("/admin/mail");
  await expect(admin.getByTestId("mail-panel")).toBeVisible({
    timeout: 20_000,
  });
}

test.beforeAll(async ({ browser }) => {
  admin = await openAdminPage(browser);
  for (const key of KEYS) {
    original[key] = (await readSetting(admin, key)).value;
  }
  admin.on("request", (request) => {
    const body = request.postData() ?? "";
    if (request.method() === "POST" && body.includes("updateInstanceSetting")) {
      try {
        const parsed = JSON.parse(body) as { variables?: { key?: string } };
        mutations.push(parsed.variables?.key ?? "?");
      } catch {
        mutations.push("?");
      }
    }
  });
});

test.afterAll(async () => {
  if (!admin) return;
  for (const key of KEYS) {
    await writeSettingOrThrow(admin, key, original[key]);
  }
  await admin.context().close();
});

test.describe("Spec 088 US7: the mail settings form", () => {
  test("Save is off when clean, and two changes are exactly two requests", async () => {
    await gotoMail();
    const save = admin.getByTestId("mail-form-save");
    await expect(save).toBeDisabled();
    await expect(admin.getByTestId("mail-form-discard")).toBeDisabled();
    await expect(admin.getByTestId("mail-form-unsaved")).toHaveText(
      "No unsaved changes",
    );
    // No row has a Save of its own any more.
    await expect(
      admin.locator('[data-testid^="instance-setting-save-"]'),
    ).toHaveCount(0);

    const host = `smtp-${Date.now()}.e2e.example.org`;
    await admin.getByTestId("setup-setting-mail.host").fill(host);
    await admin.getByTestId("setup-setting-mail.from_name").fill("Form Tester");
    await expect(admin.getByTestId("mail-form-unsaved")).toHaveText(
      "2 unsaved changes",
    );

    mutations = [];
    await save.click();
    await expect(admin.getByTestId("mail-form-result")).toHaveText(
      "2 of 2 saved",
      { timeout: 20_000 },
    );
    expect(mutations.sort()).toEqual(["mail.from_name", "mail.host"]);
    await expect(save).toBeDisabled();
    await expect(admin.getByTestId("mail-form-unsaved")).toHaveText(
      "No unsaved changes",
    );
    await expect(admin.getByTestId("instance-setting-mail.host")).toContainText(
      "Saved.",
    );

    expect((await readSetting(admin, "mail.host")).value).toBe(host);
    expect((await readSetting(admin, "mail.from_name")).value).toBe(
      "Form Tester",
    );

    // A reload shows what is stored, not a stale draft (FR-055).
    await gotoMail();
    await expect(admin.getByTestId("setup-setting-mail.host")).toHaveValue(
      host,
    );
    await expect(save).toBeDisabled();
  });

  test("leaving through a section link asks first while a change is unsaved", async () => {
    await gotoMail();
    await admin
      .getByTestId("setup-setting-mail.from_name")
      .fill("Unsaved Name");
    await expect(admin.getByTestId("mail-form-unsaved")).toHaveText(
      "1 unsaved change",
    );

    const readiness = admin.getByTestId("admin-nav-readiness");

    // Stay: the page and the draft are kept.
    admin.once("dialog", (dialog) => {
      expect(dialog.type()).toBe("confirm");
      expect(dialog.message()).toContain("unsaved changes");
      void dialog.dismiss();
    });
    await readiness.click();
    await expect(admin).toHaveURL(/\/admin\/mail$/);
    await expect(admin.getByTestId("setup-setting-mail.from_name")).toHaveValue(
      "Unsaved Name",
    );

    // Leave: the link goes through.
    admin.once("dialog", (dialog) => void dialog.accept());
    await readiness.click();
    await expect(admin).toHaveURL(/\/admin\/readiness$/);
    expect((await readSetting(admin, "mail.from_name")).value).toBe(
      "Form Tester",
    );

    // Clean, a section link asks nothing.
    await gotoMail();
    let asked = false;
    const onDialog = () => {
      asked = true;
    };
    admin.on("dialog", onDialog);
    await readiness.click();
    await expect(admin).toHaveURL(/\/admin\/readiness$/);
    admin.off("dialog", onDialog);
    expect(asked).toBe(false);
  });

  test("a refused key stays dirty with its error, and the page counts what saved", async () => {
    await gotoMail();
    await admin.getByTestId("setup-setting-mail.from_name").fill("Kept Name");
    await admin
      .getByTestId("setup-setting-mail.from_address")
      .fill("not-an-address");

    mutations = [];
    await admin.getByTestId("mail-form-save").click();
    await expect(admin.getByTestId("mail-form-result")).toHaveText(
      "1 of 2 saved",
      { timeout: 20_000 },
    );
    // Form order: the address is above the name.
    expect(mutations).toEqual(["mail.from_address", "mail.from_name"]);

    const refused = admin.getByTestId("instance-setting-mail.from_address");
    await expect(refused).toContainText("email address");
    await expect(
      refused.getByTestId("instance-setting-dirty-mail.from_address"),
    ).toBeVisible();
    await expect(
      admin.getByTestId("setup-setting-mail.from_address"),
    ).toHaveValue("not-an-address");
    await expect(admin.getByTestId("mail-form-unsaved")).toHaveText(
      "1 unsaved change",
    );
    await expect(
      admin.getByTestId("instance-setting-dirty-mail.from_name"),
    ).toHaveCount(0);
    expect((await readSetting(admin, "mail.from_name")).value).toBe(
      "Kept Name",
    );
    expect((await readSetting(admin, "mail.from_address")).value).toBe(
      original["mail.from_address"],
    );

    // Discard puts the stored value back, and Save goes off.
    await admin.getByTestId("mail-form-discard").click();
    await expect(admin.getByTestId("mail-form-unsaved")).toHaveText(
      "No unsaved changes",
    );
    await expect(admin.getByTestId("mail-form-save")).toBeDisabled();
    await expect(
      admin.getByTestId("setup-setting-mail.from_address"),
    ).toHaveValue(original["mail.from_address"] ?? "");
  });
});
