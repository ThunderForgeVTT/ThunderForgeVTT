import { postGraphQL } from "@/api/graphqlClient";

/**
 * Spec 040 US2/US3/US6: the operator's view of how this instance is
 * configured, what that configuration lets it do, and who changed what.
 *
 * Everything here is administrators-only, and every field is one the server
 * already computes. Nothing in this file re-derives a rule:
 *
 * - Whether a field may be edited is `editable`, from the server. It is not
 *   `source !== "ENVIRONMENT"` restated in TypeScript. Yesterday's setup-wizard
 *   defect was exactly that shape — a requirement re-expressed as a label, and
 *   the label was wrong for one of the three cases.
 * - A secret never arrives. `value` is null for one and `secretState` says set
 *   or not set; there is no masked form and no length anywhere in this file's
 *   types, so a component cannot render one by accident (FR-023).
 * - Which group a setting belongs in is `group`, from the declaration, so
 *   adding a setting files itself.
 */

export type SettingSource = "ENVIRONMENT" | "INSTANCE" | "DEFAULT";
export type SecretState = "SET" | "NOT_SET";
export type SettingRequirement =
  | "OPTIONAL"
  | "REQUIRED_AT_SETUP"
  | "REQUIRED_FOR_CAPABILITY";

export type SettingKind =
  | "TEXT"
  | "EMAIL"
  | "URL"
  | "PORT"
  | "BOOL"
  | "ENUM"
  | "PROSE";

export interface ResolvedSetting {
  key: string;
  /**
   * The shape of the value. Widened deliberately, the way the setup wizard's
   * own type is: a server newer than this bundle is an ordinary rolling
   * deploy, and an unknown kind should render as a text box rather than throw.
   */
  kind: SettingKind | (string & {});
  /** The permitted values, for `ENUM`. Empty for every other kind. */
  enumOptions: string[];
  /** Absent for a secret. Present and literal for everything else. */
  value: string | null;
  /** Set / not set, for a secret. Null for everything else. */
  secretState: SecretState | null;
  source: SettingSource;
  /** The variable that fixed this value, when the source is ENVIRONMENT. */
  fixedBy: string | null;
  editable: boolean;
  requirement: SettingRequirement;
  capability: string | null;
  whatToSet: string;
  group: string;
  /** The stored value could not be read back — the instance secret rotated. */
  undecryptable: boolean;
}

export interface SettingChange {
  key: string;
  previousValue: string | null;
  newValue: string | null;
  /** A secret's change record. The values are absent, not hidden by the UI. */
  redacted: boolean;
  changedBy: string | null;
  changedAt: string;
  source: string;
}

export interface ReadinessGap {
  settingKey: string;
  envVar: string | null;
  whatToSet: string;
  whatIsLimited: string;
}

export interface Capability {
  key: string;
  label: string;
  available: boolean;
  gaps: ReadinessGap[];
}

export interface SourceFlip {
  settingKey: string;
  was: SettingSource;
  now: SettingSource;
}

export interface InstanceReadiness {
  capabilities: Capability[];
  fullyConfigured: boolean;
  unrecognisedSettings: string[];
  sourceFlips: SourceFlip[];
}

const SETTING_FIELDS = `
  key
  kind
  enumOptions
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
`;

export async function fetchInstanceSettings(): Promise<ResolvedSetting[]> {
  const data = await postGraphQL<{ instanceSettings: ResolvedSetting[] }>(
    `query InstanceSettings { instanceSettings { ${SETTING_FIELDS} } }`,
  );
  return data.instanceSettings;
}

/**
 * One key, one write. There is deliberately no bulk form on the server, so
 * there is none here: a screen that saved thirty keys at once would have one
 * refusal reason for thirty possible refusals.
 *
 * `value: null` clears the stored value and the setting falls back to its
 * default; the change record shows that transition.
 */
export async function updateInstanceSetting(
  key: string,
  value: string | null,
): Promise<ResolvedSetting> {
  const data = await postGraphQL<{ updateInstanceSetting: ResolvedSetting }>(
    `mutation UpdateInstanceSetting($key: String!, $value: String) {
      updateInstanceSetting(key: $key, value: $value) { ${SETTING_FIELDS} }
    }`,
    { key, value },
  );
  return data.updateInstanceSetting;
}

export async function fetchSettingChanges(
  key: string,
  limit?: number,
): Promise<SettingChange[]> {
  const data = await postGraphQL<{ instanceSettingChanges: SettingChange[] }>(
    `query InstanceSettingChanges($key: String!, $limit: Int) {
      instanceSettingChanges(key: $key, limit: $limit) {
        key previousValue newValue redacted changedBy changedAt source
      }
    }`,
    { key, limit },
  );
  return data.instanceSettingChanges;
}

export async function fetchInstanceReadiness(): Promise<InstanceReadiness> {
  const data = await postGraphQL<{ instanceReadiness: InstanceReadiness }>(
    `query InstanceReadiness {
      instanceReadiness {
        fullyConfigured
        unrecognisedSettings
        sourceFlips { settingKey was now }
        capabilities {
          key
          label
          available
          gaps { settingKey envVar whatToSet whatIsLimited }
        }
      }
    }`,
  );
  return data.instanceReadiness;
}
