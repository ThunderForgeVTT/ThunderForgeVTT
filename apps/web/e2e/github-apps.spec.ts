import { readFileSync } from "node:fs";
import path from "node:path";
import { expect, test, type Page } from "@playwright/test";
import { openAdminPage, type GqlResult } from "./fixtures/admin";
import { freshCredentials, graphql, register } from "./fixtures/helpers";

/**
 * Spec 040 US5 — quickstart Scenario E. One GitHub application for
 * everything, or one per subsystem, and the operator knows which acts for
 * what.
 *
 * # What only a running instance can show
 *
 * `github_apps.rs` proves resolution against a resolver in process and
 * `mutations_github_apps.rs` proves the redaction rule against every field of
 * the rendering. What neither can show is that a save made through the real
 * mutation is what the *next* resolution answers with, that the screen an
 * operator opens renders the "acts for" sentence FR-020 is about, and — the
 * one that matters most — that **an existing `SYNC_GITHUB_APP_*` deployment
 * is unaffected by any of it** (FR-024).
 *
 * # Why the FR-024 case is first
 *
 * Somebody is running this today with five environment variables and nothing
 * else. A nicer configuration model is not worth their lore synchronisation,
 * so the first test asserts that adding a global application changes nothing
 * about what lore sync resolves — read through
 * `instanceRepositoryIntegration`, which calls the *unchanged*
 * `repo_host::registration_from_env()` and is therefore an independent
 * witness rather than a restatement of the new code.
 *
 * # Why these tests write settings and put them back
 *
 * There is one instance per shard and these settings are global to it. The
 * file is serial, everything it writes is recorded and cleared in
 * `afterAll`, and every case skips itself when the shard's environment has
 * fixed the values it needs to write — a key the environment fixes refuses
 * every write, and that is a fact about the stack rather than a failure.
 */

/**
 * A real RSA key. The same throwaway fixture the repo-host crate's own tests
 * use — committed, worthless, and the README beside it says so. Read from
 * disk rather than pasted here so the "no fragment of it appears anywhere"
 * assertions below are about the key that is actually stored.
 */
const KEY_PEM = readFileSync(
  // `__dirname`, not `import.meta.url`: Playwright compiles these specs to
  // CommonJS, where `import.meta` is a syntax error that fails the whole file
  // to load — the spec does not fail, it simply is not collected, and the run
  // reports "No tests found". `fixtures/global-setup.ts` resolves paths the
  // same way for the same reason.
  path.join(
    __dirname,
    "../../../crates/thunderforge-repo-host/tests/fixtures/throwaway-test-app-key.pem",
  ),
  "utf8",
);

/** The distinctive middle of the key, used as the needle for SC-007. */
const KEY_BODY = KEY_PEM.split("\n")
  .filter((line) => !line.startsWith("-----"))
  .join("");

interface ApplicationField {
  field: string;
  key: string;
  set: boolean;
  source: "ENVIRONMENT" | "INSTANCE" | "DEFAULT";
  fixedBy: string | null;
  editable: boolean;
}

interface GithubApplication {
  scope: string;
  serves: string;
  configured: boolean;
  complete: boolean;
  source: "ENVIRONMENT" | "INSTANCE" | null;
  clientId: string | null;
  slug: string | null;
  hasPrivateKey: boolean;
  fields: ApplicationField[];
  missing: string[];
  guidance: string[];
  actsFor: string[];
  resolvesTo: string | null;
  steppedOver: string[];
  steppedOverGuidance: string | null;
  lastCheckedAt: string | null;
  lastCheckOutcome: string | null;
}

const APPLICATION_FIELDS = `
  scope serves configured complete source clientId slug hasPrivateKey
  fields { field key set source fixedBy editable }
  missing guidance actsFor resolvesTo steppedOver steppedOverGuidance
  lastCheckedAt lastCheckOutcome
`;

const APPLICATIONS_QUERY = `
  query GithubApplications { githubApplications { ${APPLICATION_FIELDS} } }
`;

const SET_APPLICATION = `
  mutation SetGithubApplication($scope: String!, $field: String!, $value: String) {
    setGithubApplication(scope: $scope, field: $field, value: $value) { ${APPLICATION_FIELDS} }
  }
`;

const REPOSITORY_INTEGRATION = `
  query InstanceRepositoryIntegration {
    instanceRepositoryIntegration { configured operatorGuidance }
  }
`;

async function readApplications(page: Page): Promise<GithubApplication[]> {
  const result = await graphql<
    GqlResult<{ githubApplications: GithubApplication[] }>
  >(page, APPLICATIONS_QUERY, {});
  if (!result.data?.githubApplications) {
    throw new Error(
      `githubApplications did not answer: ${JSON.stringify(result.errors ?? result)}`,
    );
  }
  return result.data.githubApplications;
}

async function readApplication(
  page: Page,
  scope: string,
): Promise<GithubApplication> {
  const found = (await readApplications(page)).find((a) => a.scope === scope);
  if (!found) {
    throw new Error(`\`${scope}\` is not a scope this instance has`);
  }
  return found;
}

/** Raw, because half the callers are asserting a refusal. */
async function setField(
  page: Page,
  scope: string,
  field: string,
  value: string | null,
) {
  return graphql<GqlResult<{ setGithubApplication: GithubApplication }>>(
    page,
    SET_APPLICATION,
    { scope, field, value },
  );
}

async function setFieldOrThrow(
  page: Page,
  scope: string,
  field: string,
  value: string | null,
) {
  const result = await setField(page, scope, field, value);
  if (result.errors?.length) {
    throw new Error(
      `setting ${scope}.${field} was refused: ${result.errors
        .map((e) => e.message)
        .join("; ")}`,
    );
  }
}

async function writeWholeApplication(page: Page, scope: string, slug: string) {
  await setFieldOrThrow(page, scope, "client_id", `Iv1.e2e-${scope}`);
  await setFieldOrThrow(page, scope, "slug", slug);
  await setFieldOrThrow(page, scope, "private_key", KEY_PEM);
}

async function clearWholeApplication(page: Page, scope: string) {
  for (const field of ["client_id", "slug", "private_key"]) {
    await setField(page, scope, field, null);
  }
}

async function repositoryIntegration(page: Page) {
  const result = await graphql<
    GqlResult<{
      instanceRepositoryIntegration: {
        configured: boolean;
        operatorGuidance: string | null;
      };
    }>
  >(page, REPOSITORY_INTEGRATION, {});
  return result.data?.instanceRepositoryIntegration;
}

/** Which of a scope's fields the environment has fixed, by variable name. */
function fixedFields(app: GithubApplication): string[] {
  return app.fields
    .filter((f) => f.source === "ENVIRONMENT")
    .map((f) => `${f.key} (fixed by ${f.fixedBy})`);
}

let admin: Page;
let writable: Record<string, string[]> = {};
test.describe.configure({ mode: "serial" });

test.beforeAll(async ({ browser }) => {
  admin = await openAdminPage(browser);
  const applications = await readApplications(admin);
  writable = Object.fromEntries(
    applications.map((app) => [app.scope, fixedFields(app)]),
  );
  // Anything a previous run left behind. These tests assert about absence as
  // much as presence, so starting from a known-empty store matters.
  for (const scope of ["global", "feedback"]) {
    if (writable[scope]?.length === 0) {
      await clearWholeApplication(admin, scope);
    }
  }
});

test.afterAll(async () => {
  if (!admin) {
    return;
  }
  for (const scope of ["global", "feedback"]) {
    await clearWholeApplication(admin, scope);
  }
  await admin.context().close();
});

test.describe("Spec 040 US5: GitHub credentials at two scales", () => {
  /**
   * **Scenario E step 1, and the reason this file exists.** Whatever the shard
   * has in its environment, lore synchronisation answers the same before and
   * after a global application is configured. `instanceRepositoryIntegration`
   * calls `registration_from_env()` — the function that served these
   * deployments before this feature — so agreeing with it is agreeing with
   * yesterday.
   */
  test("a SYNC_GITHUB_APP_*-only deployment behaves exactly as before", async () => {
    const sync = await readApplication(admin, "sync");

    // The declarations are the sync family's, under their own names — not a
    // second set of variables invented for this feature (FR-024).
    expect(sync.fields.map((f) => f.key).sort()).toEqual([
      "github_app.sync.client_id",
      "github_app.sync.private_key",
      "github_app.sync.slug",
    ]);
    for (const field of sync.fields) {
      if (field.source === "ENVIRONMENT") {
        expect(field.fixedBy, field.key).toMatch(/^SYNC_GITHUB_APP_/);
        expect(field.editable, field.key).toBe(false);
      }
    }

    test.skip(
      writable.global.length > 0,
      `this stack fixes ${writable.global.join(", ")} in the environment`,
    );

    // **The premise, made true rather than assumed.**
    //
    // This test is about a deployment that *has* a sync application. The
    // harness deliberately clears `SYNC_GITHUB_APP_*` to `""` so credential
    // tests do not depend on the developer's own `.env` — which means sync
    // resolves to nothing here, and adding a global application legitimately
    // changes its answer to "global". That is the fallback working exactly as
    // designed, and asserting "unchanged" against it asserts the opposite of
    // the feature. It failed in the full run of 2026-09-09 for precisely that
    // reason, having never had its premise checked.
    if (sync.resolvesTo === null) {
      test.skip(
        writable.sync.length > 0,
        `this stack fixes ${writable.sync.join(", ")} in the environment`,
      );
      await writeWholeApplication(admin, "sync", "e2e-sync-app");
    }

    // Re-read *after* the premise holds. `loreSyncBefore` was captured in
    // `beforeAll`, which is before this test may have configured sync.
    const before = await readApplication(admin, "sync");
    const loreBefore = await repositoryIntegration(admin);
    expect(
      before.resolvesTo,
      "the premise: a deployment with a sync application of its own",
    ).toBe("sync");

    // Configure a global application, which is the change that could break
    // them, and confirm lore sync's own answer is untouched.
    await writeWholeApplication(admin, "global", "e2e-global-app");

    const after = await readApplication(admin, "sync");
    expect(
      after.resolvesTo,
      "adding a global application changed what lore sync resolves to",
    ).toBe(before.resolvesTo);
    expect(after.complete).toBe(before.complete);
    expect(await repositoryIntegration(admin)).toEqual(loreBefore);

    // Put sync back if this test configured it: every other test in this file
    // reads `global` and `feedback`, and a sync application left behind would
    // change what they fall back to.
    if (sync.resolvesTo === null) {
      await clearWholeApplication(admin, "sync");
    }
  });

  /**
   * Scenario E steps 2 and 3. One application, every subsystem, and the
   * sentence FR-020 asks for.
   */
  test("one global application serves every subsystem, and says which", async () => {
    test.skip(
      writable.global.length > 0 || writable.feedback.length > 0,
      "this stack fixes these applications in the environment",
    );

    await writeWholeApplication(admin, "global", "e2e-global-app");
    await clearWholeApplication(admin, "feedback");

    const [global, feedback] = await Promise.all([
      readApplication(admin, "global"),
      readApplication(admin, "feedback"),
    ]);

    expect(global.complete).toBe(true);
    expect(global.source).toBe("INSTANCE");
    expect(global.actsFor).toContain("feedback");
    expect(feedback.resolvesTo).toBe("global");
    expect(feedback.configured).toBe(false);
    // Not configured is not the same as half configured, and only the second
    // is a problem report.
    expect(feedback.steppedOver).toEqual([]);
  });

  /**
   * Scenario E step 3's second half, which is the case the spec left open:
   * scope is the outer axis and source the inner one, so a subsystem
   * application set in the screens wins over anything global.
   */
  test("a subsystem application wins for its subsystem, and the global serves the rest", async () => {
    test.skip(
      writable.global.length > 0 || writable.feedback.length > 0,
      "this stack fixes these applications in the environment",
    );

    await writeWholeApplication(admin, "global", "e2e-global-app");
    await writeWholeApplication(admin, "feedback", "e2e-feedback-app");

    const [global, feedback] = await Promise.all([
      readApplication(admin, "global"),
      readApplication(admin, "feedback"),
    ]);

    expect(feedback.complete).toBe(true);
    expect(feedback.resolvesTo).toBe("feedback");
    expect(feedback.clientId).toBe("Iv1.e2e-feedback");
    expect(global.actsFor).not.toContain("feedback");
    // The global one is still there and still serves anything without its own.
    expect(global.complete).toBe(true);
  });

  /**
   * **Scenario E step 4.** A half-written subsystem application is stepped
   * over whole. It never borrows the global one's private key — a client ID
   * from one registration with a key from another is an authentication
   * failure that reads like a bad key.
   */
  test("a half-configured subsystem falls back to the whole global application, and says so", async () => {
    test.skip(
      writable.global.length > 0 || writable.feedback.length > 0,
      "this stack fixes these applications in the environment",
    );

    await writeWholeApplication(admin, "global", "e2e-global-app");
    await clearWholeApplication(admin, "feedback");
    await setFieldOrThrow(admin, "feedback", "client_id", "Iv1.e2e-halfway");

    const feedback = await readApplication(admin, "feedback");

    expect(feedback.configured).toBe(true);
    expect(feedback.complete).toBe(false);
    expect(feedback.missing.sort()).toEqual([
      "github_app.feedback.private_key",
      "github_app.feedback.slug",
    ]);
    expect(feedback.resolvesTo).toBe("global");
    expect(feedback.steppedOver.sort()).toEqual([
      "github_app.feedback.private_key",
      "github_app.feedback.slug",
    ]);
    expect(feedback.steppedOverGuidance).toContain(
      "github_app.feedback.private_key",
    );
    // The half-written application's own client ID is not in play anywhere.
    expect(feedback.steppedOverGuidance).not.toContain("Iv1.e2e-halfway");

    // And the screen says it, which is where an operator will actually meet
    // this. FR-021 is about being told, not about the field existing.
    await admin.goto("/admin/configuration");
    const panel = admin.getByTestId("github-apps-panel");
    await expect(panel).toBeVisible({ timeout: 20_000 });
    await expect(
      admin.getByTestId("github-app-feedback-stepped-over"),
    ).toContainText("github_app.feedback.slug");
    await expect(
      admin.getByTestId("github-app-feedback-resolves-to"),
    ).toContainText("using the global application");
    // FR-020, on the same screen the global credentials are set on.
    await expect(admin.getByTestId("github-app-global-acts-for")).toContainText(
      /acting for|act for/,
    );
  });

  /**
   * **Scenario E step 5.** Parsed on save, by the parser resolution uses. A
   * credential discovered to be malformed at 3am when a delivery fails is
   * worse than one refused at the moment it is typed.
   */
  test("a value that is not a private key is refused on save and never stored", async () => {
    test.skip(
      writable.global.length > 0,
      `this stack fixes ${writable.global.join(", ")} in the environment`,
    );

    await clearWholeApplication(admin, "global");
    const notAKey = "not a key at all";
    const refusal = await setField(admin, "global", "private_key", notAKey);

    expect(refusal.errors?.length ?? 0).toBeGreaterThan(0);
    const message = refusal.errors!.map((e) => e.message).join(" ");
    // Names the setting, and quotes nothing back.
    expect(message).toContain("github_app.global.private_key");
    expect(message).not.toContain(notAKey);
    expect(message).not.toContain(String(notAKey.length));

    // Not stored, and not reported as configured.
    const global = await readApplication(admin, "global");
    expect(global.hasPrivateKey).toBe(false);
    expect(global.complete).toBe(false);
  });

  /**
   * **Scenario E step 7.** Read every diagnostic on the surface, with a real
   * key actually configured. Variable names, never a value, never a fragment,
   * never a length — a length is information about a secret and is excluded
   * deliberately.
   */
  test("no diagnostic anywhere carries a key, a fragment of one, or its length", async () => {
    test.skip(
      writable.global.length > 0,
      `this stack fixes ${writable.global.join(", ")} in the environment`,
    );

    await writeWholeApplication(admin, "global", "e2e-global-app");
    const applications = await readApplications(admin);
    const global = applications.find((a) => a.scope === "global")!;
    expect(global.hasPrivateKey, "there must be a key to fail to leak").toBe(
      true,
    );

    const wire = JSON.stringify(applications);
    expect(wire).not.toContain(KEY_PEM.trim());
    expect(wire).not.toContain(KEY_BODY);
    for (const fragment of [
      KEY_BODY.slice(0, 24),
      KEY_BODY.slice(40, 56),
      KEY_BODY.slice(-24),
    ]) {
      expect(wire).not.toContain(fragment);
    }
    for (const length of [
      String(KEY_PEM.length),
      String(KEY_PEM.trim().length),
      String(KEY_BODY.length),
    ]) {
      expect(wire).not.toContain(length);
    }

    // The screen, too — a masked preview is the tempting version of this
    // mistake and it renders rather than serialises.
    await admin.goto("/admin/configuration");
    await expect(admin.getByTestId("github-apps-panel")).toBeVisible({
      timeout: 20_000,
    });
    const shown = (
      await admin.getByTestId("github-apps-panel").innerText()
    ).replace(/\s+/g, " ");
    expect(shown).not.toContain(KEY_BODY.slice(0, 24));
    expect(shown).not.toContain("BEGIN PRIVATE KEY");
    // The client ID is *not* a secret — GitHub publishes it — and hiding it
    // would make the screen useless for the mistake operators actually make,
    // which the panel's own help text warns about: pasting the slug or the
    // numeric app ID in its place.
    //
    // Read as an input value rather than out of `innerText()`. The panel
    // renders it in a field, and `innerText()` does not include the value of
    // an `<input>` — so asserting on the panel's text found nothing and
    // looked exactly like the screen hiding it. The product was right and
    // this assertion was not.
    await expect(
      admin.getByTestId("github-app-global-client_id-input"),
    ).toHaveValue("Iv1.e2e-global");
  });

  /**
   * The read surface is administrators only. A client ID is not a secret, but
   * which applications an instance holds is not a stranger's business.
   */
  test("the surface is refused to somebody who is not an administrator", async ({
    page,
  }) => {
    // Two refusals at two layers, and they are not the same claim.
    //
    // A stranger is stopped by the transport: `/api/graphql` is wrapped in
    // `require_authenticated_user`, which answers 401 with an empty body — so
    // it is asserted with a raw request. Sending it through the `graphql`
    // helper throws on the unparseable body instead, which is how this test
    // first failed: it looked like a broken query rather than the refusal it
    // was actually getting.
    const stranger = await page.request.post("/api/graphql", {
      headers: { "Content-Type": "application/json" },
      data: { query: APPLICATIONS_QUERY, variables: {} },
    });
    expect(
      stranger.status(),
      "which applications an instance holds is not a stranger's business",
    ).toBe(401);

    // A signed-in ordinary account is stopped by the resolver's own
    // `admin_user` guard, which is the check this surface actually relies on
    // — the transport would let them through. Without this half, the test
    // above would pass on a resolver with no guard at all.
    await register(page, freshCredentials("e2eghuser"));
    const member = await graphql<
      GqlResult<{ githubApplications: GithubApplication[] | null }>
    >(page, APPLICATIONS_QUERY, {});
    expect(member.data?.githubApplications ?? null).toBeNull();
    expect(member.errors?.length ?? 0).toBeGreaterThan(0);
  });
});
