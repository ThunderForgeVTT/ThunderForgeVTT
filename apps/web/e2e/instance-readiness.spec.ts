import { expect, test, type Page } from "@playwright/test";
import {
  openAdminPage,
  readReadiness,
  readSetting,
  writeSetting,
  writeSettingOrThrow,
} from "./fixtures/admin";

/**
 * Spec 040 US6, quickstart Scenario F steps 5 and 6: an instance says what it
 * is not ready for, in terms an operator can act on — and says so positively
 * when a capability is complete.
 *
 * # What is deliberately not here
 *
 * Steps 1 to 4 of Scenario F — the share refused, the world still fully
 * playable, the already-minted link still resolving — are
 * `publishing-gate.spec.ts`, which exists and covers them. Repeating them
 * would mean two files configuring and unconfiguring the same global settings
 * on one shard, which is a flake waiting for a busy machine.
 *
 * # The assertion that carries the most weight
 *
 * FR-027 / SC-007: no gap names a value, a fragment of one, or its length —
 * **including for settings that are set**, where a masked preview is the
 * tempting mistake. So this plants a real value in a real setting and then
 * requires it to be absent from the whole rendered page. A screen that showed
 * `Thu…ive` would pass every other assertion in this file.
 *
 * # Why it closes a capability rather than the whole instance
 *
 * Step 6 asks for "configure everything", which on this stack would mean
 * setting mail, both operator blocks and three GitHub applications — twenty
 * global settings, restored afterwards, with every other spec on the shard
 * exposed if the restore ever failed halfway. Closing one capability's gaps
 * and watching it turn available is the same claim about the same code, at a
 * fraction of the blast radius: the report is derived, so a capability that
 * flips is the derivation working.
 */

test.describe.configure({ mode: "serial" });

/** A value each declaration's own validators accept. */
const ACCEPTABLE: Record<string, string> = {
  "operator.name": "The Forgeton Collective",
  "operator.contact_email": "operator@thunderforge-e2e.example.org",
  "operator.jurisdiction": "The courts of Forgeton, in the Anvil Reach",
  "notice.contact_name": "The Forgeton Collective, Notices",
  "notice.contact_email": "notices@thunderforge-e2e.example.org",
  "notice.contact_postal_address": "1 Anvil Row, Forgeton",
  // Manifest-backed rather than a row, and included for exactly that reason:
  // `identify_operator` spans both backings, so closing it proves the write
  // path reaches `manifest.json` as well as `instance_settings`.
  support_email: "support@thunderforge-e2e.example.org",
};

/** The capability this file completes. Its gaps are all plain text or email. */
const CAPABILITY = "identify_operator";

let admin: Page;
const original: Record<string, string | null> = {};
/** The keys the capability test actually set, in order, so the third test can
 *  take one back without guessing which. */
const closedGaps: string[] = [];

async function remember(key: string): Promise<void> {
  if (!(key in original)) {
    original[key] = (await readSetting(admin, key)).value;
  }
}

test.beforeAll(async ({ browser }) => {
  admin = await openAdminPage(browser);
});

/**
 * Put back what was found — and cope with a value that cannot be put back.
 *
 * `config/realm-defaults.json` ships `support_email` as
 * `stewards@thunderforge.local`, and `Validator::NoReservedTld` refuses
 * `.local`. So an instance running on its shipped default holds a value its
 * own settings mutation will not accept, and a restore that insisted would
 * fail every run. Clearing instead returns the key to resolving from that same
 * default, which is where the value came from — the resolution an operator
 * sees is identical, and the failure is reported rather than swallowed.
 */
async function restore(key: string, value: string | null): Promise<void> {
  const result = await writeSetting(admin, key, value);
  if (!result.errors?.length) return;
  await writeSettingOrThrow(admin, key, null);
  console.warn(
    `[instance-readiness] \`${key}\` could not be restored to the value this ` +
      `instance was already using (${result.errors.map((e) => e.message).join("; ")}). ` +
      "Cleared instead, so it resolves from its default.",
  );
}

test.afterAll(async () => {
  if (!admin) return;
  for (const [key, value] of Object.entries(original)) {
    await restore(key, value);
  }
  await admin.context().close();
});

test.describe("Spec 040 Scenario F: an instance says what it is not ready for", () => {
  test("every gap names what it disables and what to set, and never a value", async () => {
    // Planted through the API rather than the screen: this test is about what
    // readiness renders, and a value that arrived by a different route is a
    // stronger test of that than one this page typed itself.
    const planted = "Planted Operator 4f2a9c";
    await remember("operator.name");
    await writeSettingOrThrow(admin, "operator.name", planted);

    await admin.goto("/admin/readiness");
    const panel = admin.getByTestId("readiness-panel");
    await expect(panel).toBeVisible({ timeout: 20_000 });

    const readiness = await readReadiness(admin);
    const unavailable = readiness.capabilities.filter((c) => !c.available);
    expect(
      unavailable.length,
      "this harness configures nothing, so something must be unavailable — " +
        "if not, this spec is asserting against a stack it was not written for",
    ).toBeGreaterThan(0);

    for (const capability of unavailable) {
      const card = admin.getByTestId(`readiness-capability-${capability.key}`);
      await expect(card).toHaveAttribute("data-available", "false");
      expect(
        capability.gaps.length,
        `\`${capability.key}\` is unavailable and names nothing to do about it`,
      ).toBeGreaterThan(0);

      for (const gap of capability.gaps) {
        // Rendered verbatim from the registry: what to set, and what is
        // limited until it is.
        await expect(card).toContainText(gap.whatToSet);
        await expect(card).toContainText(gap.whatIsLimited);
      }
    }

    // SC-007, over the whole page rather than one card.
    const page = await admin.locator("body").innerText();
    expect(page).not.toContain(planted);
    expect(page).not.toContain("4f2a9c");
    // Nor a masked or measured form of it.
    expect(page).not.toMatch(/Planted\s*[….]{1,3}/);
    expect(page).not.toMatch(/\b\d+\s*characters?\b/i);
  });

  test("closing a capability's gaps turns it available, and says so positively", async () => {
    const before = await readReadiness(admin);
    const capability = before.capabilities.find((c) => c.key === CAPABILITY);
    expect(capability, `this instance declares no \`${CAPABILITY}\``).toBeTruthy();

    for (const gap of capability?.gaps ?? []) {
      const value = ACCEPTABLE[gap.settingKey];
      expect(
        value,
        `no acceptable value is known for \`${gap.settingKey}\`; add one above ` +
          "rather than letting this test skip the gap it was written for",
      ).toBeTruthy();
      await remember(gap.settingKey);
      await writeSettingOrThrow(admin, gap.settingKey, value);
      closedGaps.push(gap.settingKey);
    }

    await admin.goto("/admin/readiness");
    const card = admin.getByTestId(`readiness-capability-${CAPABILITY}`);
    await expect(card).toBeVisible({ timeout: 20_000 });
    // The positive statement FR-025 asks for: available capabilities are
    // reported, not omitted. An empty list reads as "we did not check".
    await expect(card).toHaveAttribute("data-available", "true");
    await expect(card).toContainText("Available");

    const after = await readReadiness(admin);
    expect(after.capabilities.find((c) => c.key === CAPABILITY)?.gaps).toEqual([]);
  });

  test("clearing a setting takes the capability back, so the report is derived and not remembered", async () => {
    const key = closedGaps[0];
    expect(key, "the previous test closed no gap, so there is none to reopen").toBeTruthy();

    await writeSettingOrThrow(admin, key, null);

    await admin.goto("/admin/readiness");
    const card = admin.getByTestId(`readiness-capability-${CAPABILITY}`);
    await expect(card).toBeVisible({ timeout: 20_000 });
    // A stored readiness flag would still say "available" here. This is the
    // assertion that it is computed on every read.
    await expect(card).toHaveAttribute("data-available", "false");
    await expect(card).toContainText("Not configured");
  });
});
