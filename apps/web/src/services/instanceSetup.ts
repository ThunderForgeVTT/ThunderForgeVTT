import { withCsrf } from "@/api/auth";
import { postGraphQL } from "@/api/graphqlClient";
import type { SetupStatus } from "@/types/auth";

/**
 * Spec 040 US1 — first run, as data rather than as a sequence of screens.
 *
 * # Why this file exists at all
 *
 * `SetupPage.tsx` used to be one 780-line component with the whole of first
 * run inlined: three hard-coded "steps" that were really just captions, and
 * one form. Nothing about it could be driven by what the instance actually
 * needs, and nothing about it could be tested — this app has no jsdom and no
 * testing-library (see `services/__tests__/twoFactorEnrolment.test.ts`'s
 * header for the same problem and the same answer), so a test that drove the
 * wizard by clicking was never available. A test that drives the derivation is.
 *
 * So the derivation lives here, pure, and the components under
 * `pages/setup/steps/` draw whatever it returns.
 *
 * # The central rule (contracts/setup.md rule 1)
 *
 * **The wizard is driven by the registry, not by a list of steps.** The step
 * list is built from `required_settings` in `GET /authentication/setup/status`.
 * Adding a `RequiredAtSetup` declaration to `settings/registry.rs` must make a
 * field appear in setup with no change to any file in `pages/setup/`, and that
 * is only true if nothing here enumerates `operator.*`, `notice.*`, `mail.*`
 * or `support_email`. Nothing here does.
 *
 * Three steps are *not* derived, because they are not settings and no
 * declaration could ever produce them:
 *
 *   - **the administrator account** — a user row, not a setting;
 *   - **the second factor** — an enrolment, owned by spec 041 (FR-002a);
 *   - **the review** — the report about the other steps.
 *
 * Everything between the account and the second factor comes from the
 * registry, in the order the registry lists it.
 *
 * # Ambiguities in the contract, and the reading taken
 *
 * `contracts/setup.md` shows `required_settings` entries as
 * `{ key, kind, satisfied, source, fixed_by }` and calls the array "what setup
 * still needs". Two things it does not settle, and what was chosen:
 *
 *  1. **Does the array contain only `RequiredAtSetup` declarations?** It
 *     cannot: spec FR-002 has setup collect the notice contact and the mail
 *     settings, and in the registry those are `RequiredFor(capability)`, which
 *     FR-003 calls offered rather than required. So the array is read as
 *     *every declaration setup is willing to ask about*, and each entry may
 *     carry its own `requirement`. An entry with no `requirement` is treated
 *     as required, which is the reading that cannot silently let setup finish
 *     with something unanswered.
 *  2. **Does an entry describe a setting that is already satisfied?** Yes —
 *     it must, because rule 3 requires an environment-fixed setting to be
 *     *shown as fixed*, which is impossible if it has been filtered out. A
 *     satisfied entry is therefore expected and is not asked for again unless
 *     it is editable (`source: "INSTANCE"`), where it is shown pre-filled so a
 *     resumed pass can be corrected.
 *
 * Both readings are the ones that keep the wizard resumable, which is the
 * tie-breaker FR-006 asks for.
 *
 * Every optional field below is optional because the server half of this
 * feature is being written in parallel with it. A field the server does not
 * send yet degrades to something honest rather than to a crash.
 */

const API_BASE = "/api";

export type SettingKind =
  | "TEXT"
  | "EMAIL"
  | "URL"
  | "PORT"
  | "BOOL"
  | "ENUM"
  | "PROSE";

export type SettingSource = "ENVIRONMENT" | "INSTANCE" | "DEFAULT";

export type SettingRequirement =
  | "REQUIRED_AT_SETUP"
  | "REQUIRED_FOR"
  | "OPTIONAL";

/** One entry of `required_settings`, as `contracts/setup.md` renders it. */
export interface RequiredSetting {
  key: string;
  /** Widened deliberately: an unknown kind renders as text rather than throwing. */
  kind: SettingKind | (string & {});
  satisfied: boolean;
  source: SettingSource | null;
  fixed_by: string | null;
  /** Absent means "required" — see the header's ambiguity 1. */
  requirement?: SettingRequirement | (string & {}) | null;
  /** The stored value, when there is one and it is not a secret. */
  value?: string | null;
  /** The registry's `group`. Absent falls back to a group derived from the key. */
  group?: string | null;
  /** The registry's `what_to_set`, rendered verbatim as the field's hint. */
  what_to_set?: string | null;
  /** The capability a `REQUIRED_FOR` declaration serves. */
  capability?: string | null;
  /** The registry's `what_is_limited` — what an instance loses while this is unset. */
  what_is_limited?: string | null;
  /** For `ENUM`. */
  options?: string[] | null;
  secret?: boolean | null;
  /**
   * Whether the wizard gives this a step of its own.
   *
   * The registry states it per declaration (`SetupVisibility`). False means
   * "offered": something an operator might have meant to set, which belongs on
   * the last step with the other such things rather than on a screen of its own
   * that looks like a question. Absent from an older server, and absent reads
   * as essential — the pre-064 behaviour, one step per group.
   */
  essential?: boolean | null;
}

/** `GET /authentication/setup/status`, with what spec 040 adds to it. */
export interface SetupStatusWithSettings extends SetupStatus {
  required_settings?: RequiredSetting[] | null;
  second_factor_confirmed?: boolean | null;
}

export type SetupStepKind =
  | "account"
  | "settings"
  | "providers"
  | "second-factor"
  | "review";

export interface SetupStep {
  /** Stable across renders and reloads; the testid suffix. */
  id: string;
  kind: SetupStepKind;
  title: string;
  /** Every setting filed under this step, fixed ones included. */
  settings: RequiredSetting[];
  /** The ones this step actually asks for. */
  askable: RequiredSetting[];
  /** The ones the environment has fixed (FR-009): shown, never asked for. */
  fixed: RequiredSetting[];
  /**
   * Nothing left to ask. FR-009's consequence: an instance configured wholly
   * by environment has every settings step skipped and reaches the review
   * having been asked only for the account and the second factor.
   */
  skipped: boolean;
  /** True once this step's answers are stored, so a resumed pass lands past it. */
  complete: boolean;
  /**
   * Why this step is being asked about, when "why" is the operator's real
   * question. Undefined for a step whose field hints already say everything.
   */
  explainer?: string;
}

/**
 * Group labels for a server that does not send `group` yet.
 *
 * This is a *label* table, not a step table — it never decides which settings
 * exist or which step they land in, only what the heading over them reads as.
 * A key with no entry here still gets a step; it gets one titled after its own
 * prefix. That is the difference between a fallback and a hard-coded sequence.
 */
const GROUP_LABELS: Record<string, string> = {
  Operator: "Who runs this instance",
  "Copyright notices": "Where a copyright notice is served",
  Support: "Getting help",
  Mail: "Sending mail",
  "Legal prose": "Your own legal wording",
  "GitHub applications": "GitHub applications",
  Realm: "This realm",
  Access: "Who may join",
  Storage: "Where uploads are kept",
};

/**
 * The step title for everything the wizard merely offers.
 *
 * One step, not one per group, and the distinction is the point: storage
 * credentials, a realm name and a GitHub application are all things an operator
 * may want and none of them is a question this instance needs answered to run.
 * Four more screens that look mandatory is how a wizard stops being read.
 */
const LEFTOVERS_STEP_TITLE = "Optional extras";

export const LEFTOVERS_STEP_ID = "settings-anything-else";

/**
 * Why a group is being asked about, for the groups where "why" is the
 * operator's actual question.
 *
 * Spec 052 FR-032: the duties publishing creates are explained *before* the
 * operator is asked to discharge them. An operator who is asked for a
 * copyright agent's postal address with no reason given reasonably concludes
 * the software is being nosy; the reason is that serving other people's
 * uploads to the public is what creates the duty, and that is worth one
 * sentence.
 *
 * Keyed by registry group so a new declaration in one of these groups inherits
 * the explanation rather than needing a second one written for it.
 */
const GROUP_EXPLAINERS: Record<string, string> = {
  Access:
    "This decides who can get an account here. Invite-only is the usual answer for a private table: nobody signs up unless you hand them a link, and that link lets them either create an account or sign in with any provider you configure below.",
  "Copyright notices":
    "You said this instance publishes content beyond a world. Publishing other people's uploads to people who are not at the table is what creates a copyright-notice duty, so somebody has to be reachable about it. These answers are served publicly at /legal/dmca — which is the point of them, and the reason they cannot be a reserved example address.",
  "Legal prose":
    "Also because this instance publishes beyond a world: people who are not at your table need to be able to read what they are agreeing to before they join. Leave a field blank and that page simply is not served — nothing is invented on your behalf.",
  Mail: "Without a mail server this instance cannot send an invitation, a password reset or a verification link. You can leave it blank and hand out invite links yourself.",
  "GitHub applications":
    "Up to three GitHub Apps: one for syncing a world\u2019s lore to a repository, one for filing in-app feedback as issues, and a global one either of those falls back to. Leave all of it blank unless you have created the app it asks about.",
  Realm:
    "How this instance introduces itself: its name, its welcome message and the defaults a new world starts with.",
  Features: "Things that are off until you turn them on.",
  Storage:
    "Maps, tokens and portraits are kept in an S3-compatible object store. The shipped defaults point at a local RustFS, which is right for a self-hosted instance running the bundled stack and wrong for anything else.",
};

/**
 * The sentence above a step, or nothing.
 *
 * Nothing is the honest answer for a group whose own field hints already say
 * everything — a realm name does not need a paragraph.
 */
export function groupExplainer(group: string): string | undefined {
  return GROUP_EXPLAINERS[group];
}

const KEY_PREFIX_GROUPS: Record<string, string> = {
  storage: "Storage",
  operator: "Operator",
  notice: "Copyright notices",
  support_email: "Support",
  mail: "Mail",
  legal: "Legal prose",
  github_app: "GitHub applications",
  instance: "Access",
};

/** RFC 2606 / 6761. An address here can never receive a copyright notice. */
const RESERVED_TLDS = [".local", ".example", ".invalid", ".test"];

const EMAIL_PATTERN = /\S+@\S+\.\S+/;

export function slugify(value: string): string {
  return (
    value
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "") || "settings"
  );
}

/** The registry group a setting belongs to, from the server or from its key. */
export function groupOf(setting: RequiredSetting): string {
  if (setting.group && setting.group.trim()) {
    return setting.group.trim();
  }

  const prefix = setting.key.includes(".")
    ? setting.key.slice(0, setting.key.indexOf("."))
    : setting.key;

  return KEY_PREFIX_GROUPS[prefix] ?? titleCase(prefix);
}

export function groupTitle(group: string): string {
  return GROUP_LABELS[group] ?? group;
}

function titleCase(value: string): string {
  return value
    .split(/[._-]/)
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

/** The label over one field: the key's last segment, in prose. */
export function settingLabel(key: string): string {
  const tail = key.includes(".") ? key.slice(key.lastIndexOf(".") + 1) : key;
  return titleCase(tail);
}

/**
 * FR-009: the environment fixed this, so it is reported, not offered.
 *
 * `source === "ENVIRONMENT"` is the authority; `fixed_by` alone is accepted
 * too, because a server that names the variable has plainly fixed the value
 * and refusing to believe it would put an editable field in front of an
 * operator whose edit the server would then refuse with `409
 * fixed_by_environment`.
 */
export function isFixedByEnvironment(setting: RequiredSetting): boolean {
  return setting.source === "ENVIRONMENT" || Boolean(setting.fixed_by);
}

export function isOptional(setting: RequiredSetting): boolean {
  return setting.requirement === "OPTIONAL";
}

/**
 * What to call this setting's requirement, in three states rather than two.
 *
 * There are genuinely three, and collapsing them to "Optional or Required"
 * mislabels the middle one — which is what happened, and what the first-run
 * e2e caught. `operator.jurisdiction` is `RequiredFor(PublishTerms)`: the
 * server does not include it in `missing_required_settings` and `/complete`
 * succeeds without it, so calling it "Required" told the operator something
 * the product does not enforce, and worse, the step refused to advance on it.
 *
 * FR-003 is precisely this distinction — what setup requires, versus what it
 * merely offers — so losing it in the label loses the requirement.
 */
export function requirementLabel(setting: RequiredSetting): string {
  if (isOptional(setting)) {
    return "Optional";
  }
  if (setting.requirement === "REQUIRED_FOR") {
    const purpose = CAPABILITY_PURPOSES[setting.capability ?? ""];
    return purpose ? `Optional · ${purpose}` : "Optional";
  }
  return "Required";
}

/**
 * What a `REQUIRED_FOR` setting is for, in the operator's words.
 *
 * "Needed for a feature" said that something depended on the field and not
 * what, on every field it appeared beside. Setup finishes without any of
 * these, so the honest word is "Optional", followed by the one thing that
 * will not work until it is filled in.
 */
const CAPABILITY_PURPOSES: Record<string, string> = {
  send_mail: "needed to send mail",
  store_assets: "needed to keep uploads",
  publish_beyond_world: "needed once you share publicly",
  publish_terms: "needed once you share publicly",
  sync_lore: "needed to sync lore with GitHub",
  feedback: "needed for in-app feedback",
};

/**
 * Whether this setting gets a step of its own.
 *
 * Absent means yes, which keeps a server that does not send the field behaving
 * as it did before: one step per group, nothing merged.
 */
export function isEssential(setting: RequiredSetting): boolean {
  return setting.essential !== false;
}

/** Blocks completion while unset — the predicate `/complete` enforces. */
export function blocksCompletion(setting: RequiredSetting): boolean {
  return (
    !setting.satisfied &&
    (setting.requirement === undefined ||
      setting.requirement === null ||
      setting.requirement === "REQUIRED_AT_SETUP")
  );
}

export interface BuildStepsInput {
  requiredSettings: RequiredSetting[];
  accountCreated: boolean;
  secondFactorConfirmed: boolean;
}

/**
 * The whole wizard, as a list, derived from what the instance says it needs.
 *
 * Order is: the account, then one step per registry group **in the order the
 * server listed them**, then the second factor, then the review. The server's
 * order is the registry's order, which is documented there as "the order an
 * operator reads them in" — re-sorting here would be this file inventing a
 * sequence, which is the thing T069 is against.
 */
export function buildSetupSteps({
  requiredSettings,
  accountCreated,
  secondFactorConfirmed,
}: BuildStepsInput): SetupStep[] {
  const essential = requiredSettings.filter(isEssential);
  const leftovers = requiredSettings.filter((s) => !isEssential(s));

  const groups: string[] = [];
  const byGroup = new Map<string, RequiredSetting[]>();

  for (const setting of essential) {
    const group = groupOf(setting);
    if (!byGroup.has(group)) {
      byGroup.set(group, []);
      groups.push(group);
    }
    byGroup.get(group)?.push(setting);
  }

  const settingSteps: SetupStep[] = groups.map((group) => {
    const settings = byGroup.get(group) ?? [];
    const fixed = settings.filter(isFixedByEnvironment);
    const askable = settings.filter((s) => !isFixedByEnvironment(s));

    return {
      id: `settings-${slugify(group)}`,
      kind: "settings",
      title: groupTitle(group),
      settings,
      askable,
      fixed,
      skipped: askable.length === 0,
      // A step is answered when nothing in it still blocks completion. An
      // optional field left blank is answered; that is FR-003.
      complete: settings.every((s) => !blocksCompletion(s)),
      explainer: groupExplainer(group),
    };
  });

  // Everything the wizard merely offers, on one step, in the registry's order.
  //
  // `complete: true` is not an assumption that it was filled in — nothing here
  // can block completion by construction, so an operator who skips it has
  // answered it. What it buys is FR-006: a resumed pass lands on the second
  // factor rather than being sent back to a page of optional extras it already
  // walked past.
  const leftoversStep: SetupStep = {
    id: LEFTOVERS_STEP_ID,
    kind: "settings",
    title: LEFTOVERS_STEP_TITLE,
    settings: leftovers,
    askable: leftovers.filter((s) => !isFixedByEnvironment(s)),
    fixed: leftovers.filter(isFixedByEnvironment),
    skipped: leftovers.every(isFixedByEnvironment),
    complete: true,
    explainer:
      "None of this is needed to start playing, and all of it can be changed later in the admin area. Open only the sections you care about; the rest can stay closed.",
  };

  // Providers sit after the settings groups and before the extras: by this
  // point the operator has said who may join, so "an invite link also admits
  // someone by signing in with one of these" is a sentence that means
  // something. It is never required — an instance with local accounts only is
  // a supported instance — so it is always complete and always skippable.
  const providersStep: SetupStep = {
    id: "providers",
    kind: "providers",
    title: "Signing in with an existing account",
    settings: [],
    askable: [],
    fixed: [],
    skipped: false,
    complete: true,
    explainer:
      "Letting people sign in with an account they already have. Optional — this instance works with its own accounts alone — and worth doing if you handed out an invite link, because a link admits its holder either by creating an account or by signing in with any provider enabled here.",
  };

  return [
    {
      id: "account",
      kind: "account",
      title: "The first administrator",
      settings: [],
      askable: [],
      fixed: [],
      skipped: false,
      complete: accountCreated,
    },
    ...settingSteps,
    providersStep,
    // Omitted rather than rendered empty. A step titled "anything else you
    // meant to set" with nothing on it is a dead end, and a server that
    // offers nothing optional should produce no such screen at all.
    ...(leftovers.length > 0 ? [leftoversStep] : []),
    {
      id: "second-factor",
      kind: "second-factor",
      title: "Your second factor",
      settings: [],
      askable: [],
      fixed: [],
      skipped: false,
      complete: secondFactorConfirmed,
    },
    {
      id: "review",
      kind: "review",
      title: "What this instance can do",
      settings: [],
      askable: [],
      fixed: [],
      skipped: false,
      complete: false,
    },
  ];
}

/** The steps an operator is actually walked through. */
export function visibleSteps(steps: SetupStep[]): SetupStep[] {
  return steps.filter((step) => !step.skipped);
}

/**
 * Where a resumed pass lands (FR-006).
 *
 * The first step that is neither skipped nor already answered; the review if
 * every one of them is. Nothing is read from component state, a cookie or a
 * session to work this out — it is a function of what the server stored, which
 * is what makes closing the browser harmless.
 */
export function firstUnansweredStep(steps: SetupStep[]): number {
  const visible = visibleSteps(steps);
  const index = visible.findIndex((step) => !step.complete);
  return index === -1 ? Math.max(visible.length - 1, 0) : index;
}

/**
 * Client-side refusal for one value, or `undefined`.
 *
 * The server is the authority (FR-004, `settings/validate.rs`) and its refusal
 * is rendered wherever it disagrees with this. These exist so that the obvious
 * mistakes are named before a round trip, and they are deliberately the
 * *explainable* rules only — the reserved-TLD one, not a placeholder
 * heuristic, for the reason registry.rs § R15 gives: a cleverer detector
 * rejects somebody's real name.
 */
export function validateSettingValue(
  setting: RequiredSetting,
  raw: string,
): string | undefined {
  const value = raw.trim();

  if (!value) {
    // A blank field for a setting the instance has already stored is not a
    // missing answer, it is an unchanged one. The status endpoint reports
    // `satisfied` without necessarily reporting the value — it does not send
    // one today — so a resumed pass shows an empty box over a stored value,
    // and refusing that box would make FR-006's resumability unusable: the
    // operator would have to retype every answer they had already given.
    // The gate is `blocksCompletion`, not `!isOptional`. Those differ for a
    // `REQUIRED_FOR` declaration, and using the wrong one made the wizard
    // stricter than the server it fronts: it refused to advance past a field
    // `/complete` would have been perfectly happy to leave unset. A step that
    // cannot be passed is worse than a feature that is not configured.
    if (!blocksCompletion(setting)) {
      return undefined;
    }
    return "This is required before setup can finish.";
  }

  if (setting.kind === "EMAIL") {
    if (!EMAIL_PATTERN.test(value)) {
      return "Enter a valid email address.";
    }

    const lowered = value.toLowerCase();
    if (RESERVED_TLDS.some((tld) => lowered.endsWith(tld))) {
      return "An address at a reserved domain (.example, .invalid, .test, .local) can never receive mail. Use an address you monitor.";
    }
  }

  if (setting.kind === "PORT") {
    const port = Number(value);
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      return "Enter a port between 1 and 65535.";
    }
  }

  if (setting.kind === "URL" && !/^https?:\/\/\S+$/i.test(value)) {
    return "Enter a URL starting with http:// or https://.";
  }

  return undefined;
}

/** Every refusal in one step, keyed by setting. Empty means the step may be sent. */
export function validateStep(
  step: SetupStep,
  values: Record<string, string>,
): Record<string, string> {
  const errors: Record<string, string> = {};

  for (const setting of step.askable) {
    const problem = validateSettingValue(setting, values[setting.key] ?? "");
    if (problem) {
      errors[setting.key] = problem;
    }
  }

  return errors;
}

/**
 * What one step sends.
 *
 * Fixed settings are never included: the server answers `409
 * fixed_by_environment` for them, and sending a value it must refuse would
 * turn FR-009 into an error message. A blank optional field is omitted rather
 * than sent as `""`, so "I skipped this" and "I set this to nothing" stay
 * different answers.
 */
export function stepPayload(
  step: SetupStep,
  values: Record<string, string>,
): Record<string, string> {
  const payload: Record<string, string> = {};

  for (const setting of step.askable) {
    const value = (values[setting.key] ?? "").trim();
    if (value) {
      payload[setting.key] = value;
    }
  }

  return payload;
}

/** Values a resumed pass starts with: whatever the instance already stored. */
export function initialValues(
  requiredSettings: RequiredSetting[],
): Record<string, string> {
  const values: Record<string, string> = {};

  for (const setting of requiredSettings) {
    if (!isFixedByEnvironment(setting) && typeof setting.value === "string") {
      values[setting.key] = setting.value;
    }
  }

  return values;
}

/** A refusal from a setup route, with the machine-readable half kept. */
export class SetupRequestError extends Error {
  readonly code: string;
  readonly field: string | null;
  readonly fixedBy: string | null;
  readonly missing: string[];

  constructor(
    code: string,
    message: string,
    options: {
      field?: string | null;
      fixedBy?: string | null;
      missing?: string[] | null;
    } = {},
  ) {
    super(message);
    this.name = "SetupRequestError";
    this.code = code;
    this.field = options.field ?? null;
    this.fixedBy = options.fixedBy ?? null;
    this.missing = options.missing ?? [];
  }
}

interface SetupErrorPayload {
  status?: string;
  code?: string;
  message?: string;
  field?: string | null;
  fixed_by?: string | null;
  missing?: string[] | null;
}

async function readJson<T>(response: Response): Promise<T | null> {
  const contentType = response.headers.get("content-type") ?? "";
  if (!contentType.includes("application/json")) {
    return null;
  }

  return (await response.json()) as T;
}

function refusal(
  payload: SetupErrorPayload | null,
  fallback: string,
): SetupRequestError {
  return new SetupRequestError(
    payload?.code ?? payload?.status ?? "setup_error",
    payload?.message ?? fallback,
    {
      field: payload?.field ?? null,
      fixedBy: payload?.fixed_by ?? null,
      missing: payload?.missing ?? null,
    },
  );
}

export async function readSetupStatus(): Promise<SetupStatusWithSettings> {
  const response = await fetch(`${API_BASE}/authentication/setup/status`, {
    credentials: "same-origin",
  });

  if (!response.ok) {
    throw new Error("Failed to load setup status");
  }

  return (await response.json()) as SetupStatusWithSettings;
}

/**
 * `POST /authentication/setup/settings` — one step, written immediately.
 *
 * Contract rule 2: resumability is a property of the storage, not of a session
 * kept alive, so this is called as each step is left rather than once at the
 * end with everything accumulated in React state.
 */
export async function saveSetupSettings(
  adminCode: string,
  values: Record<string, string>,
): Promise<string[]> {
  const response = await fetch(`${API_BASE}/authentication/setup/settings`, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({ "Content-Type": "application/json" }),
    body: JSON.stringify({ admin_code: adminCode, values }),
  });

  const payload = await readJson<SetupErrorPayload & { remaining?: string[] }>(
    response,
  );

  if (!response.ok) {
    throw refusal(payload, "That step could not be saved.");
  }

  return payload?.remaining ?? [];
}

export interface ReadinessGap {
  settingKey: string;
  envVar: string | null;
  whatToSet: string;
  whatIsLimited: string;
}

export interface ReadinessCapability {
  key: string;
  label: string;
  available: boolean;
  gaps: ReadinessGap[];
}

export interface ReadinessReport {
  capabilities: ReadinessCapability[];
  fullyConfigured: boolean;
  unrecognisedSettings: string[];
}

interface SnakeGap {
  setting_key?: string;
  settingKey?: string;
  env_var?: string | null;
  envVar?: string | null;
  what_to_set?: string;
  whatToSet?: string;
  what_is_limited?: string;
  whatIsLimited?: string;
}

interface SnakeCapability {
  key?: string;
  label?: string;
  available?: boolean;
  gaps?: SnakeGap[];
}

interface SnakeReadiness {
  capabilities?: SnakeCapability[];
  fully_configured?: boolean;
  fullyConfigured?: boolean;
  unrecognised_settings?: string[];
  unrecognisedSettings?: string[];
}

/**
 * The readiness report in one shape, whichever shape it arrived in.
 *
 * There are two sources for it and they do not agree on case: the GraphQL
 * `instanceReadiness` query is camel (`fixtures/admin.ts` records its exact
 * shape) and the REST `/complete` response, which contract rule 5 says
 * carries the report, is snake like every other REST payload here. Rather than
 * pick one and have the other render as blanks, both are accepted.
 */
export function normaliseReadiness(
  raw: SnakeReadiness | null | undefined,
): ReadinessReport | null {
  if (!raw || !Array.isArray(raw.capabilities)) {
    return null;
  }

  const capabilities: ReadinessCapability[] = raw.capabilities.map(
    (capability) => ({
      key: capability.key ?? "",
      label: capability.label ?? capability.key ?? "",
      available: Boolean(capability.available),
      gaps: (capability.gaps ?? []).map((gap) => ({
        settingKey: gap.settingKey ?? gap.setting_key ?? "",
        envVar: gap.envVar ?? gap.env_var ?? null,
        whatToSet: gap.whatToSet ?? gap.what_to_set ?? "",
        whatIsLimited: gap.whatIsLimited ?? gap.what_is_limited ?? "",
      })),
    }),
  );

  return {
    capabilities,
    fullyConfigured: Boolean(raw.fullyConfigured ?? raw.fully_configured),
    unrecognisedSettings:
      raw.unrecognisedSettings ?? raw.unrecognised_settings ?? [],
  };
}

export interface CompleteSetupResult {
  message: string;
  readiness: ReadinessReport | null;
}

/** `POST /authentication/setup/complete`. */
export async function completeSetup(
  adminCode: string,
): Promise<CompleteSetupResult> {
  const response = await fetch(`${API_BASE}/authentication/setup/complete`, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({ "Content-Type": "application/json" }),
    body: JSON.stringify({ admin_code: adminCode }),
  });

  const payload = await readJson<
    SetupErrorPayload & { readiness?: SnakeReadiness | null }
  >(response);

  if (!response.ok) {
    throw refusal(payload, "Setup could not be completed.");
  }

  return {
    message: payload?.message ?? "Setup is complete.",
    readiness: normaliseReadiness(payload?.readiness),
  };
}

const INSTANCE_READINESS = `
  query SetupReadiness {
    instanceReadiness {
      capabilities {
        key
        label
        available
        gaps { settingKey envVar whatToSet whatIsLimited }
      }
      fullyConfigured
      unrecognisedSettings
    }
  }
`;

/**
 * The readiness report, read as the administrator who just enrolled.
 *
 * `instanceReadiness` is behind `graphql::admin_user`, which by the review
 * step is satisfied: `/setup/basic` issues a session cookie for the account it
 * creates, and that account is the first administrator. Before the account
 * step it is unreachable, and the review step falls back to what
 * `required_settings` already says rather than showing nothing.
 */
export async function fetchInstanceReadiness(): Promise<ReadinessReport | null> {
  try {
    const data = await postGraphQL<{ instanceReadiness?: SnakeReadiness }>(
      INSTANCE_READINESS,
    );
    return normaliseReadiness(data.instanceReadiness);
  } catch {
    // A refusal here is not a setup failure. The review step says what it
    // knows from `required_settings` instead.
    return null;
  }
}
