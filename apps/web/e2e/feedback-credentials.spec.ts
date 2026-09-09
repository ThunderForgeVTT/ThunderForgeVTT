import { readFile } from "node:fs/promises";
import path from "node:path";
import { expect, test, type Page } from "@playwright/test";
import {
  openAdminPage,
  readSetting,
  writeSettingOrThrow,
} from "./fixtures/admin";
import { graphql } from "./fixtures/helpers";

/**
 * Spec 037 US4 / spec 040 FR-019–FR-024: one application for every subsystem,
 * said out loud.
 *
 * # Why this can be driven at all
 *
 * The shard sets `FEEDBACK_GITHUB_APP_*` in the backend's environment and sets
 * nothing for `global` or `sync`. That is not an accident of the harness — it
 * is the exact shape US4 is about: one subsystem configured from the
 * environment, the others resolving from whatever the instance stores. So the
 * scenarios below are reachable by writing instance settings, which is what an
 * operator does.
 *
 * # The rule that matters
 *
 * An application resolves **whole**. A subsystem with a client id and no
 * private key does not borrow the global one's key — it is reported
 * incomplete and falls through to the global application *entire*. Spec 037's
 * delivery originally resolved field by field; spec 040 FR-021 overrode that,
 * because a client id from one registration with a key from another
 * authenticates as a bad key, which is the worst diagnostic a configuration
 * mistake can produce.
 */

test.describe.configure({ mode: "serial" });

const APPLICATIONS = `
  query GithubApplications {
    githubApplications {
      scope serves configured complete source clientId slug hasPrivateKey
      fields { field key set source fixedBy editable }
      missing actsFor resolvesTo steppedOver steppedOverGuidance
    }
  }
`;

interface Application {
  scope: string;
  configured: boolean;
  complete: boolean;
  source: string | null;
  clientId: string | null;
  hasPrivateKey: boolean;
  missing: string[];
  actsFor: string[];
  resolvesTo: string | null;
  steppedOver: string[];
  steppedOverGuidance: string | null;
}

/** A committed, worthless RSA key — the same fixture the crates' own tests use. */
const KEY_PATH = path.resolve(
  // `__dirname`, not `import.meta`: Playwright compiles these specs to
  // CommonJS, where `import.meta` is a syntax error that makes the file fail
  // to load *silently* — the run then reports "no tests found" rather than an
  // error. Playwright's cwd is `apps/web`, so a repository-relative path reads
  // as missing.
  __dirname,
  "../../../crates/thunderforge-repo-host/tests/fixtures/throwaway-test-app-key.pem",
);

let admin: Page;
const original: Record<string, string | null> = {};
const GLOBAL_KEYS = [
  "github_app.global.client_id",
  "github_app.global.slug",
  "github_app.global.private_key",
  "github_app.sync.client_id",
  "github_app.sync.slug",
  "github_app.sync.private_key",
];

async function applications(): Promise<Application[]> {
  const result = await graphql<{
    data?: { githubApplications: Application[] };
    errors?: { message: string }[];
  }>(admin, APPLICATIONS, {});
  expect(
    result.errors,
    `githubApplications did not answer: ${JSON.stringify(result.errors)}`,
  ).toBeFalsy();
  return result.data!.githubApplications;
}

const scope = (all: Application[], name: string): Application => {
  const found = all.find((a) => a.scope === name);
  expect(found, `no application reported for scope \`${name}\``).toBeTruthy();
  return found!;
};

test.beforeAll(async ({ browser }) => {
  admin = await openAdminPage(browser);
  for (const key of GLOBAL_KEYS) {
    original[key] = (await readSetting(admin, key)).value;
  }
});

test.afterAll(async () => {
  if (!admin) return;
  // Global credentials are instance-wide: leaving one behind would change what
  // every other spec on this shard resolves.
  for (const [key, value] of Object.entries(original)) {
    await writeSettingOrThrow(admin, key, value);
  }
  await admin.context().close();
});

test.describe("Spec 037 US4: one application, or one each", () => {
  test("a subsystem configured from the environment says so, and no key is ever rendered", async () => {
    const feedback = scope(await applications(), "feedback");

    // The shard sets `FEEDBACK_GITHUB_APP_*`, so this is the "scope outside,
    // source inside" case: the feedback subsystem, from the environment.
    expect(feedback.configured).toBe(true);
    expect(feedback.source).toBe("ENVIRONMENT");
    expect(feedback.resolvesTo).toBe("feedback");

    // FR-023 / SC-007. The client id is shown because GitHub publishes it; the
    // key is a boolean and that is the entire vocabulary this surface has.
    expect(feedback.hasPrivateKey).toBe(true);
    const rendered = JSON.stringify(feedback);
    expect(rendered).not.toContain("BEGIN RSA");
    expect(rendered).not.toContain("PRIVATE KEY");
  });

  test("a global application serves every subsystem that has none, and says which", async () => {
    const pem = await readFile(KEY_PATH, "utf-8");

    await writeSettingOrThrow(
      admin,
      "github_app.global.client_id",
      "Iv1.globaltest",
    );
    await writeSettingOrThrow(
      admin,
      "github_app.global.slug",
      "thunderforge-global",
    );
    await writeSettingOrThrow(admin, "github_app.global.private_key", pem);

    const all = await applications();
    const global = scope(all, "global");
    expect(global.complete).toBe(true);

    // Scenario 3: the operator is told what they are choosing *while* they
    // choose it. The list is computed by the server from the resolution
    // delivery actually gets, not restated in the client.
    expect(
      global.actsFor.length,
      "a configured global application names no subsystems it acts for",
    ).toBeGreaterThan(0);
    expect(global.actsFor).toContain("sync");

    // Scenario 1 and 2 together: sync has no application of its own, so it
    // resolves to the global one — while feedback keeps its own.
    expect(scope(all, "sync").resolvesTo).toBe("global");
    expect(scope(all, "feedback").resolvesTo).toBe("feedback");
  });

  test("a half-written subsystem application is stepped over whole, never completed from the global one", async () => {
    // A client id and nothing else. Under field-by-field resolution this would
    // silently borrow the global application's private key and authenticate as
    // a mismatched pair — the failure FR-021 exists to prevent.
    await writeSettingOrThrow(
      admin,
      "github_app.sync.client_id",
      "Iv1.halfwritten",
    );

    const all = await applications();
    const sync = scope(all, "sync");

    expect(
      sync.configured,
      "a half-written application reads as unconfigured",
    ).toBe(true);
    expect(sync.complete).toBe(false);
    expect(sync.missing.length).toBeGreaterThan(0);

    // Fell through to the *whole* global application, and said so.
    expect(sync.resolvesTo).toBe("global");
    expect(
      sync.steppedOver.length,
      "the operator is not told their half-written application was skipped",
    ).toBeGreaterThan(0);
    expect(sync.steppedOverGuidance ?? "").not.toBe("");

    // And the borrowed-key failure is specifically absent. `clientId` reports
    // what *this scope* has configured — the half-written id, correctly — so
    // the claim worth asserting is not about that field but about the key:
    // sync has none of its own and did not acquire one. Under field-by-field
    // resolution it would show a key here, borrowed from the global
    // application, and would then authenticate as a mismatched pair.
    expect(
      sync.hasPrivateKey,
      "the half-written application acquired a private key it never had",
    ).toBe(false);
    expect(sync.missing).toContain("github_app.sync.private_key");
  });
});
