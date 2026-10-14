import { expect, test, type Page } from "./fixtures/test";
import {
  openAdminPage,
  readSetting,
  writeSetting,
  writeSettingOrThrow,
} from "./fixtures/admin";

/**
 * Spec 074 SC-006, against a running server: with `feature.demo` off, `/demo`
 * is not found and the sign-in page draws no link to it; with it on, a
 * signed-out stranger opens the demo from the sign-in page.
 *
 * The backend is one process throughout and is started with a built demo
 * (`DEMO_DIR`, see `scripts/e2e/demo.mjs`), so the door is decided by the
 * setting alone, per request, as FR-015 says. `anonymous-shell.spec.ts` pins
 * the off half on its own; this file pins both halves of the same instance,
 * one after the other, which is the claim a switch makes.
 *
 * Each test has a context of its own with no storage state, so each is a
 * stranger who asks the instance afresh which flags are on. The setting is
 * instance-wide and the shard runs one worker; what was found is put back
 * afterwards, as `instance-feature-flags.spec.ts` does.
 */

const FLAG = "feature.demo";
const DEMO_LINK = "Enter demo workspace";
/** The demo's one world; `apps/demo/e2e/support.ts` names it the same. */
const DEMO_WORLD_ID = "d0000000-0000-4000-0002-000000000001";

let admin: Page;
let original: string | null = null;

test.describe.configure({ mode: "serial" });
test.use({ storageState: { cookies: [], origins: [] } });

test.beforeAll(async ({ browser }) => {
  admin = await openAdminPage(browser);
  const found = await readSetting(admin, FLAG);
  test.skip(
    found.source === "ENVIRONMENT",
    `${FLAG} is fixed by ${found.fixedBy}; nothing here can change it`,
  );
  original = found.source === "INSTANCE" ? found.value : null;
});

test.afterAll(async () => {
  if (!admin) return;
  await writeSetting(admin, FLAG, original);
  await admin.context().close();
});

test("off: /demo is not found, and the sign-in page draws no link to it", async ({
  page,
}) => {
  await writeSettingOrThrow(admin, FLAG, "false");

  const door = await page.request.get("/demo/");
  expect(door.status()).toBe(404);

  await page.goto("/login");
  await expect(page.getByTestId("footer-legal-links")).toBeVisible();
  await expect(page.getByRole("button", { name: "Menu" })).toHaveCount(0);
  await expect(page.getByTestId("app-footer")).not.toContainText(DEMO_LINK);
});

test("on: a signed-out stranger opens the demo from the sign-in page", async ({
  page,
}) => {
  await writeSettingOrThrow(admin, FLAG, "true");

  await page.goto("/login");
  await page
    .getByTestId("app-footer")
    .getByRole("link", { name: DEMO_LINK })
    .click();

  // The demo is a page load, not a route of this client: it lands on its one
  // world's dashboard with the standing notice (FR-004), which no page of the
  // real client draws.
  await expect(page).toHaveURL(new RegExp(`/demo/world/${DEMO_WORLD_ID}$`), {
    timeout: 30_000,
  });
  await expect(page.getByTestId("demo-notice")).toContainText(
    "Nothing is saved anywhere but this browser",
    { timeout: 30_000 },
  );

  // And the same door, asked directly, is open now.
  const door = await page.request.get("/demo/");
  expect(door.status()).toBe(200);
});
