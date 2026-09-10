import { type Browser, type Page } from "@playwright/test";
import { graphql, loginAsAdmin } from "./helpers";

/**
 * Reaching spec 040's administrator-only surfaces from a test.
 *
 * `instanceSettings`, `instanceReadiness`, `updateInstanceSetting`,
 * `mailAvailability` and `sendTestMail` all go through `graphql::admin_user`,
 * so every spec about them needs the seeded platform administrator rather than
 * a registered account. That is one login and one set of typed shapes, kept
 * here rather than copied into four files.
 */

/** What a GraphQL call answers with when the answer might be a refusal. */
export interface GqlResult<T> {
  data?: T | null;
  errors?: { message: string }[];
}

/** `ResolvedSetting`, as `settings/graphql.rs` renders it. */
export interface ResolvedSetting {
  key: string;
  value: string | null;
  secretState: "SET" | "NOT_SET" | null;
  source: "ENVIRONMENT" | "INSTANCE" | "DEFAULT";
  fixedBy: string | null;
  editable: boolean;
  requirement: string;
  capability: string | null;
  whatToSet: string;
  group: string;
  undecryptable: boolean;
}

export interface ReadinessGap {
  settingKey: string;
  envVar: string | null;
  whatToSet: string;
  whatIsLimited: string;
}

export interface InstanceReadiness {
  capabilities: {
    key: string;
    label: string;
    available: boolean;
    gaps: ReadinessGap[];
  }[];
  fullyConfigured: boolean;
  unrecognisedSettings: string[];
  sourceFlips: { settingKey: string; was: string; now: string }[];
}

export const INSTANCE_SETTINGS_QUERY = `
  query InstanceSettings {
    instanceSettings {
      key
      value
      secretState
      source
      fixedBy
      editable
      requirement
      capability
      whatToSet
      group
      undecryptable
    }
  }
`;

export const INSTANCE_READINESS_QUERY = `
  query InstanceReadiness {
    instanceReadiness {
      capabilities {
        key
        label
        available
        gaps { settingKey envVar whatToSet whatIsLimited }
      }
      fullyConfigured
      unrecognisedSettings
      sourceFlips { settingKey was now }
    }
  }
`;

export const UPDATE_INSTANCE_SETTING = `
  mutation UpdateInstanceSetting($key: String!, $value: String) {
    updateInstanceSetting(key: $key, value: $value) {
      key
      value
      source
      editable
      secretState
    }
  }
`;

/**
 * A page signed in as the seeded platform administrator, in a context of its
 * own.
 *
 * Waits for `/admin` **or** `/welcome` for the reason
 * `instance-access-gate.spec.ts` records: `redirectAfterLogin` sends an
 * administrator to the admin area, and waiting only for `/welcome` times out
 * on a login that entirely succeeded.
 */
export async function openAdminPage(browser: Browser): Promise<Page> {
  const context = await browser.newContext();
  const page = await context.newPage();
  // Spec 041 FR-027: an administrator holds a second factor, so this is two
  // steps. `loginAsAdmin` does both, using the secret `global-setup` enrolled
  // for this run — a plain `login` leaves the page on the challenge step, and
  // every test in the file is then skipped by a failing `beforeAll` whose
  // message is about a locator rather than about a sign-in.
  await loginAsAdmin(page);
  return page;
}

/** Every declared setting as this instance resolves it right now. */
export async function readSettings(page: Page): Promise<ResolvedSetting[]> {
  const result = await graphql<
    GqlResult<{ instanceSettings: ResolvedSetting[] }>
  >(page, INSTANCE_SETTINGS_QUERY, {});
  if (!result.data?.instanceSettings) {
    throw new Error(
      `instanceSettings did not answer: ${JSON.stringify(result.errors ?? result)}`,
    );
  }
  return result.data.instanceSettings;
}

export async function readSetting(
  page: Page,
  key: string,
): Promise<ResolvedSetting> {
  const found = (await readSettings(page)).find(
    (setting) => setting.key === key,
  );
  if (!found) {
    throw new Error(`\`${key}\` is not a setting this instance declares`);
  }
  return found;
}

/**
 * Write one setting, returning the raw answer.
 *
 * Raw rather than unwrapped because half the callers are asserting a
 * *refusal*, and a helper that threw on one would make those unwritable.
 */
export async function writeSetting(
  page: Page,
  key: string,
  value: string | null,
): Promise<GqlResult<{ updateInstanceSetting: ResolvedSetting }>> {
  return graphql<GqlResult<{ updateInstanceSetting: ResolvedSetting }>>(
    page,
    UPDATE_INSTANCE_SETTING,
    { key, value },
  );
}

/** Write one setting and fail loudly if the instance refused. */
export async function writeSettingOrThrow(
  page: Page,
  key: string,
  value: string | null,
): Promise<void> {
  const result = await writeSetting(page, key, value);
  if (result.errors?.length) {
    throw new Error(
      `writing \`${key}\` was refused: ${result.errors.map((e) => e.message).join("; ")}`,
    );
  }
}

export async function readReadiness(page: Page): Promise<InstanceReadiness> {
  const result = await graphql<
    GqlResult<{ instanceReadiness: InstanceReadiness }>
  >(page, INSTANCE_READINESS_QUERY, {});
  if (!result.data?.instanceReadiness) {
    throw new Error(
      `instanceReadiness did not answer: ${JSON.stringify(result.errors ?? result)}`,
    );
  }
  return result.data.instanceReadiness;
}
