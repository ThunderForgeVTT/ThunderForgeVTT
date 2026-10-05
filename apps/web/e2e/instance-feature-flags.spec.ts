import { expect, test, type Page } from "./fixtures/test";
import {
  openAdminPage,
  readSetting,
  writeSetting,
  writeSettingOrThrow,
} from "./fixtures/admin";
import { freshCredentials, graphql, register } from "./fixtures/helpers";

/**
 * Spec 068 Story 2 (SC-006): a flag changes what the server allows and what
 * the web app shows, with no rebuild and no restart.
 *
 * The first flag is `feature.book_import`. Off, the library offers no
 * importer and the server refuses the mutation whoever sends it; on, both
 * come back. The backend in this test is one process from start to end, so
 * every change it sees is a change made at runtime.
 *
 * # What is asked of the mutation
 *
 * A book for a system nobody has installed. With the flag on, that is refused
 * for *its own* reason — the system declares nothing — which shows the flag's
 * refusal is gone without importing anything; a whole import is
 * `book-import-commit.spec.ts`'s to walk.
 *
 * # Why this changes an instance setting and puts it back
 *
 * A flag is instance-wide and every spec in the shard shares the instance.
 * The shard runs one worker, so nothing else is importing while this is off;
 * what it found is restored afterwards, as `instance-settings.spec.ts` does.
 */

const FLAG = "feature.book_import";
const SWITCHED_OFF = /switched off on this instance/i;

const IMPORT = `
  mutation ($input: CreateCompendiumFromImportInput!) {
    createCompendiumFromImport(input: $input) {
      id
    }
  }
`;

/** What the server says to an import, as one string; empty when it agreed. */
async function importRefusal(page: Page): Promise<string> {
  const result = await graphql<{ errors?: { message: string }[] }>(
    page,
    IMPORT,
    {
      input: {
        bookTitle: "A Book Nobody Wrote",
        sourceHash: "0".repeat(64),
        systemId: "no-such-system",
        parserVersion: "e2e",
        pageCount: 1,
        silentPageCount: 0,
        entries: [],
      },
    },
  );
  return (result.errors ?? []).map((error) => error.message).join("; ");
}

/** Go to the library the way a link would, without loading the app again. */
async function walkToLibrary(page: Page): Promise<void> {
  await page.evaluate(() => {
    window.history.pushState({}, "", "/library");
    window.dispatchEvent(new PopStateEvent("popstate"));
  });
  await expect(
    page.getByRole("heading", { name: "Your library" }),
  ).toBeVisible();
}

let admin: Page;
let original: string | null = null;

test.describe.configure({ mode: "serial" });

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

test.describe("Spec 068: a feature the instance can switch off", () => {
  test("a flag nobody has set reads as its declared default, in a Features group", async () => {
    await writeSettingOrThrow(admin, FLAG, null);
    const flag = await readSetting(admin, FLAG);
    expect(flag.source).toBe("DEFAULT");
    expect(flag.value).toBe("true");
    expect(flag.group).toBe("Features");
  });

  test("off: the library offers no importer and the server refuses; on: both come back", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eflag"));
    await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });

    await page.goto("/library");
    await expect(page.getByTestId("import-book")).toBeVisible();
    expect(await importRefusal(page)).not.toMatch(SWITCHED_OFF);

    await writeSettingOrThrow(admin, FLAG, "false");

    // The next request plays by it: no restart, and no reload of this page.
    expect(await importRefusal(page)).toMatch(SWITCHED_OFF);
    await page.goto("/library");
    await expect(
      page.getByRole("heading", { name: "Your library" }),
    ).toBeVisible();
    await expect(page.getByTestId("new-collection")).toBeVisible();
    await expect(page.getByTestId("import-book")).toHaveCount(0);

    await writeSettingOrThrow(admin, FLAG, "true");
    expect(await importRefusal(page)).not.toMatch(SWITCHED_OFF);
    await page.goto("/library");
    await expect(page.getByTestId("import-book")).toBeVisible();
  });

  test("the administrator who flips it sees it take without loading the app again", async () => {
    await writeSettingOrThrow(admin, FLAG, "true");
    await admin.goto("/admin/instance?group=features");
    const box = admin.getByTestId(`setup-setting-${FLAG}`);
    await expect(box).toBeChecked();

    await box.uncheck();
    await admin.getByTestId(`instance-setting-save-${FLAG}`).click();
    await expect(
      admin.getByTestId(`instance-setting-source-${FLAG}`),
    ).toContainText(/instance/i);

    await walkToLibrary(admin);
    await expect(admin.getByTestId("new-collection")).toBeVisible();
    await expect(admin.getByTestId("import-book")).toHaveCount(0);

    // And the change is on record, with who made it.
    const flag = await readSetting(admin, FLAG);
    expect(flag.value).toBe("false");
    expect(flag.source).toBe("INSTANCE");
  });

  test("a visitor is told only the public flags, and this one is not public", async ({
    page,
  }) => {
    // `/api/graphql` turns a visitor away before any resolver runs; this is
    // the route the app itself asks when nobody is signed in.
    await page.goto("/login");
    const result = await page.evaluate(async () => {
      const response = await fetch("/api/graphql/public", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ query: "query { featureFlags { key on } }" }),
      });
      return (await response.json()) as {
        data?: { featureFlags: { key: string; on: boolean }[] };
      };
    });
    expect(result.data?.featureFlags).toBeDefined();
    expect(
      result.data?.featureFlags.map((flag) => flag.key) ?? [],
    ).not.toContain(FLAG);
  });
});
