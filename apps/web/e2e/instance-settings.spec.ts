import { expect, test, type Page } from "@playwright/test";
import {
  INSTANCE_READINESS_QUERY,
  INSTANCE_SETTINGS_QUERY,
  openAdminPage,
  readReadiness,
  readSetting,
  readSettings,
  writeSetting,
  writeSettingOrThrow,
  type GqlResult,
  type ResolvedSetting,
} from "./fixtures/admin";
import {
  freshCredentials,
  graphql,
  register,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Spec 040 US2, US3 and US6: an administrator reads every setting with its
 * source, changes one, is refused one the environment has fixed, and is told
 * what this instance cannot yet do — while nobody else can read or write any
 * of it.
 *
 * # What only a running instance can show
 *
 * `settings/graphql.rs` proves the redaction rule against every declaration,
 * and `readiness.rs` proves the report's shape. Both do it with a resolver in
 * process. What they cannot show is that a write made through the real
 * mutation is the value the *next* read resolves — FR-007's "no restart" —
 * and that the surface is reachable under the names a client uses, by an
 * administrator and by nobody else.
 *
 * # Why these tests change settings and put them back
 *
 * There is one instance per shard and settings are global to it. Everything
 * here records what it found and restores it, and the file is serial so two
 * tests never hold different opinions about the same key at once.
 */

/**
 * The setting the write tests use.
 *
 * `operator.jurisdiction` is `RequiredFor(PublishTerms)` and it is the *only*
 * declaration that capability requires — so clearing it makes exactly one
 * capability unavailable with exactly one gap, which is what lets the
 * readiness test below assert on a gap it created rather than on whatever the
 * harness happened to leave unset.
 */
const JURISDICTION = "operator.jurisdiction";
const A_JURISDICTION = "The courts of Forgeton, in the Anvil Reach";

/** A secret declaration, for the rule that a secret is only ever SET/NOT_SET. */
const MAIL_PASSWORD = "mail.password";

let admin: Page;
const original: Record<string, string | null> = {};

/** Keys this file writes to, and whether the environment has fixed them. */
async function unwritableKeys(page: Page): Promise<string[]> {
  const settings = await readSettings(page);
  return [JURISDICTION, MAIL_PASSWORD]
    .map((key) => settings.find((s) => s.key === key))
    .filter((s): s is ResolvedSetting => Boolean(s))
    .filter((s) => s.source === "ENVIRONMENT")
    .map((s) => `${s.key} (fixed by ${s.fixedBy})`);
}

let fixedByEnvironment: string[] = [];

test.describe.configure({ mode: "serial" });

test.beforeAll(async ({ browser }) => {
  admin = await openAdminPage(browser);
  fixedByEnvironment = await unwritableKeys(admin);
  for (const key of [JURISDICTION, MAIL_PASSWORD]) {
    original[key] = (await readSetting(admin, key)).value;
  }
});

test.afterAll(async () => {
  if (!admin) {
    return;
  }
  for (const key of [JURISDICTION, MAIL_PASSWORD]) {
    // A secret's `value` reads back as null by design, so the restore for
    // `mail.password` is "clear it" whenever it was not set — which is the
    // only state this file ever finds it in on a stack that configures no
    // mail. Deliberately not `writeSettingOrThrow`: a key the environment
    // fixes refuses every write, and that is not a failed restore.
    await writeSetting(admin, key, original[key]);
  }
  await admin.context().close();
});

test.describe("Spec 040 US2/US3: an administrator's view of the settings", () => {
  test("every declared setting is listed with a source and something to act on", async () => {
    const settings = await readSettings(admin);

    // The list is the declaration list, so a surface returning a handful of
    // keys is a surface that has quietly stopped enumerating.
    expect(settings.length).toBeGreaterThanOrEqual(30);

    for (const setting of settings) {
      expect(["ENVIRONMENT", "INSTANCE", "DEFAULT"]).toContain(setting.source);
      // FR-012: a declaration cannot omit what to set or where it is filed,
      // and this is where an operator reads both.
      expect(setting.whatToSet.trim(), setting.key).not.toBe("");
      expect(setting.group.trim(), setting.key).not.toBe("");
      // FR-009: the environment fixing a value is the *only* reason this
      // surface names a variable, and a fixed value is never offered as
      // editable.
      if (setting.source === "ENVIRONMENT") {
        expect(setting.fixedBy, setting.key).toBeTruthy();
        expect(setting.editable, setting.key).toBe(false);
      }
    }

    const known = settings.map((s) => s.key);
    for (const key of [
      "operator.name",
      "notice.contact_email",
      "mail.host",
      MAIL_PASSWORD,
      JURISDICTION,
    ]) {
      expect(known).toContain(key);
    }
  });

  test("a secret is only ever set or not set, whatever it is worth", async () => {
    test.skip(
      fixedByEnvironment.length > 0,
      `this stack fixes ${fixedByEnvironment.join(", ")} in the environment`,
    );

    const before = await readSetting(admin, MAIL_PASSWORD);
    expect(before.value, "a secret never renders its value").toBeNull();
    expect(before.secretState).toBe("NOT_SET");

    const credential = `correct-horse-battery-staple-${uniqueSuffix()}`;
    await writeSettingOrThrow(admin, MAIL_PASSWORD, credential);

    const after = await readSetting(admin, MAIL_PASSWORD);
    // The write took — this is the half that says the assertions below are
    // about a secret that is actually stored, not about an empty one.
    expect(after.secretState).toBe("SET");
    expect(after.source).toBe("INSTANCE");
    expect(after.value).toBeNull();

    // SC-007, against the wire rather than the struct: not the value, not a
    // fragment of it, not its length. A masked preview is the tempting
    // version of this mistake, so the length is asserted too.
    const rendered = JSON.stringify(after);
    expect(rendered).not.toContain(credential);
    for (const fragment of ["correct", "horse", "battery", "staple"]) {
      expect(rendered).not.toContain(fragment);
    }
    expect(rendered).not.toContain(String(credential.length));

    await writeSettingOrThrow(admin, MAIL_PASSWORD, null);
    expect((await readSetting(admin, MAIL_PASSWORD)).secretState).toBe(
      "NOT_SET",
    );
  });

  test("a change made here is the value the next read resolves", async () => {
    test.skip(
      fixedByEnvironment.length > 0,
      `this stack fixes ${fixedByEnvironment.join(", ")} in the environment`,
    );

    const before = await readSetting(admin, JURISDICTION);
    // Whatever it was, it was not this — so a stale read cannot pass.
    expect(before.value).not.toBe(A_JURISDICTION);

    await writeSettingOrThrow(admin, JURISDICTION, A_JURISDICTION);

    const after = await readSetting(admin, JURISDICTION);
    expect(after.value).toBe(A_JURISDICTION);
    // FR-007: no restart. The source says where the answer came from, and a
    // value written through this surface reads back as the instance's own.
    expect(after.source).toBe("INSTANCE");
    expect(after.editable).toBe(true);

    // Clearing is a change like any other: the setting falls back to its
    // default rather than keeping the last value written.
    await writeSettingOrThrow(admin, JURISDICTION, null);
    const cleared = await readSetting(admin, JURISDICTION);
    expect(cleared.value).toBeNull();
    expect(cleared.source).toBe("DEFAULT");

    // And the trail FR-008 asks for exists for both of them.
    const history = await graphql<
      GqlResult<{
        instanceSettingChanges: {
          key: string;
          previousValue: string | null;
          newValue: string | null;
          source: string;
        }[];
      }>
    >(
      admin,
      `
        query Changes($key: String!) {
          instanceSettingChanges(key: $key, limit: 5) {
            key
            previousValue
            newValue
            source
          }
        }
      `,
      { key: JURISDICTION },
    );
    const changes = history.data?.instanceSettingChanges ?? [];
    expect(changes.length).toBeGreaterThanOrEqual(2);
    expect(changes.map((c) => c.newValue)).toContain(A_JURISDICTION);
    expect(changes.every((c) => c.source === "admin")).toBe(true);
  });

  /**
   * FR-009 / ADR-041: a value the environment fixed is refused by name, not
   * accepted and quietly dropped.
   *
   * Whether this stack has such a setting is a property of the operator's
   * environment rather than of the harness — `scripts/e2e-parallel.mjs` fixes
   * none of the declared settings itself, and a developer's `.env` may fix
   * the `SYNC_GITHUB_APP_*` ones. So the test finds one rather than assuming
   * one, and says plainly when there is none to find.
   */
  test("a value the environment has fixed says so and refuses a write", async () => {
    const settings = await readSettings(admin);
    const fixed = settings.find((s) => s.source === "ENVIRONMENT");
    // Not a skip any more. `scripts/e2e-parallel.mjs` fixes `realm_name` in
    // the environment deliberately so this test always has a target: it used
    // to look for one and skip when there was none, and on this harness there
    // never was — so the refusal it exists to pin was pinned by nothing, and
    // the test passed by not running. A missing target is now a failure,
    // because it means the harness stopped providing one.
    expect(
      fixed,
      "the harness must fix one declared setting in the environment (THUNDERFORGE_REALM_NAME) for this to test anything",
    ).toBeTruthy();

    const target = fixed!;
    expect(target.editable).toBe(false);
    expect(target.fixedBy).toBeTruthy();

    const refused = await writeSetting(admin, target.key, "something else");
    expect(
      refused.data?.updateInstanceSetting,
      "an environment-fixed setting must not report a successful write",
    ).toBeFalsy();
    const message = refused.errors?.[0]?.message ?? "";
    // It names the variable, which is the one thing an operator has to change
    // to make it editable again.
    expect(message).toContain(target.fixedBy!);

    // And nothing moved. A refusal that still wrote is the failure ADR-041 is
    // about, only louder.
    const after = await readSetting(admin, target.key);
    expect(after.value).toBe(target.value);
    expect(after.source).toBe("ENVIRONMENT");
  });
});

test.describe("Spec 040 US6: what this instance cannot yet do", () => {
  test("readiness names the gap it was given, and loses it when it is filled", async () => {
    test.skip(
      fixedByEnvironment.length > 0,
      `this stack fixes ${fixedByEnvironment.join(", ")} in the environment`,
    );

    await writeSettingOrThrow(admin, JURISDICTION, null);

    const withGap = await readReadiness(admin);
    // Every capability is reported every time, available ones included:
    // FR-025 scenario 3 wants a positive statement, and an empty list of
    // complaints reads as "we did not check".
    expect(withGap.capabilities.map((c) => c.key)).toEqual(
      expect.arrayContaining([
        "identify_operator",
        "send_mail",
        "publish_beyond_world",
        "publish_terms",
        "sync_lore",
        "feedback",
      ]),
    );

    const terms = withGap.capabilities.find((c) => c.key === "publish_terms")!;
    expect(terms.available).toBe(false);
    const gap = terms.gaps.find((g) => g.settingKey === JURISDICTION);
    expect(
      gap,
      "the capability whose only requirement was just cleared must name it",
    ).toBeTruthy();
    expect(gap!.whatToSet.trim()).not.toBe("");
    expect(gap!.whatIsLimited.trim()).not.toBe("");
    expect(gap!.envVar).toBe("THUNDERFORGE_OPERATOR_JURISDICTION");
    // `fullyConfigured` is a conjunction of the capabilities and not a
    // separate opinion about them.
    expect(withGap.fullyConfigured).toBe(
      withGap.capabilities.every((c) => c.available),
    );
    // The instance's own bookkeeping row (`system.source_snapshot`) is not an
    // operator's leftover configuration.
    expect(
      withGap.unrecognisedSettings.filter((key) => key.startsWith("system.")),
    ).toEqual([]);

    await writeSettingOrThrow(admin, JURISDICTION, A_JURISDICTION);

    const filled = await readReadiness(admin);
    const termsNow = filled.capabilities.find(
      (c) => c.key === "publish_terms",
    )!;
    expect(
      termsNow.gaps.map((g) => g.settingKey),
      "a gap must disappear when the setting behind it is set — readiness is derived per read, never stored",
    ).not.toContain(JURISDICTION);
    expect(termsNow.available).toBe(true);

    await writeSettingOrThrow(admin, JURISDICTION, null);
  });

  test("a gap never names a value, a fragment of one, or its length", async () => {
    test.skip(
      fixedByEnvironment.length > 0,
      `this stack fixes ${fixedByEnvironment.join(", ")} in the environment`,
    );

    // Asserted with a secret that **is** set, because the tempting mistake is
    // a masked preview of a configured credential rather than a leak from an
    // empty one.
    const credential = `correct-horse-battery-staple-${uniqueSuffix()}`;
    await writeSettingOrThrow(admin, MAIL_PASSWORD, credential);

    try {
      const readiness = await readReadiness(admin);
      const rendered = JSON.stringify(readiness);
      expect(rendered).not.toContain(credential);
      for (const fragment of ["correct", "horse", "battery", "staple"]) {
        expect(rendered).not.toContain(fragment);
      }

      // The length, checked against the gaps for the subsystem the secret
      // belongs to rather than the whole document — a port number elsewhere
      // in the report is a number an operator asked for, not a hint.
      const mailGaps = JSON.stringify(
        readiness.capabilities
          .flatMap((c) => c.gaps)
          .filter((g) => g.settingKey.startsWith("mail.")),
      );
      expect(mailGaps).not.toContain(String(credential.length));
    } finally {
      await writeSettingOrThrow(admin, MAIL_PASSWORD, null);
    }
  });
});

test.describe("Spec 040: none of this is anybody else's", () => {
  test("a signed-in non-administrator is refused every part of it", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2esetting"));

    for (const [label, query] of [
      ["instanceSettings", INSTANCE_SETTINGS_QUERY],
      ["instanceReadiness", INSTANCE_READINESS_QUERY],
    ] as const) {
      const result = await graphql<GqlResult<Record<string, unknown>>>(
        page,
        query,
        {},
      );
      expect(result.errors?.[0]?.message, label).toContain(
        "Admin privileges required",
      );
      // Not merely an error alongside a payload: there must be nothing to read.
      expect(result.data?.[label], label).toBeFalsy();
    }

    const marker = `Somewhere they should not be able to name ${uniqueSuffix()}`;
    const write = await writeSetting(page, JURISDICTION, marker);
    expect(write.errors?.[0]?.message).toContain("Admin privileges required");

    // And the refusal actually refused. A test that only asserts the error
    // would pass against a server that returned one *after* writing.
    expect((await readSetting(admin, JURISDICTION)).value).not.toBe(marker);
  });
});
