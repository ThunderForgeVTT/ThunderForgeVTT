import { describe, expect, it } from "vitest";
import {
  blocksCompletion,
  buildSetupSteps,
  firstUnansweredStep,
  initialValues,
  isFixedByEnvironment,
  normaliseReadiness,
  settingLabel,
  stepPayload,
  validateSettingValue,
  validateStep,
  visibleSteps,
  type RequiredSetting,
} from "@/services/instanceSetup";

/**
 * Spec 040 US1 — the properties T069 and T071 are actually about.
 *
 * This app has no jsdom and no testing-library, so the wizard cannot be driven
 * by clicking here; T074's Playwright spec does that. What is asserted here is
 * the derivation the wizard is built out of, which is where "driven by the
 * registry" and "an environment-fixed setting is never asked for" either hold
 * or quietly stop holding.
 */

function setting(
  key: string,
  overrides: Partial<RequiredSetting> = {},
): RequiredSetting {
  return {
    key,
    kind: "TEXT",
    satisfied: false,
    source: null,
    fixed_by: null,
    ...overrides,
  };
}

describe("buildSetupSteps", () => {
  it("derives a step per registry group, in the server's order", () => {
    const steps = buildSetupSteps({
      requiredSettings: [
        setting("operator.name"),
        setting("operator.contact_email", { kind: "EMAIL" }),
        setting("notice.contact_email", { kind: "EMAIL" }),
        setting("support_email", { kind: "EMAIL" }),
      ],
      accountCreated: false,
      secondFactorConfirmed: false,
    });

    expect(steps.map((step) => step.id)).toEqual([
      "account",
      "settings-operator",
      "settings-copyright-notices",
      "settings-support",
      "second-factor",
      "review",
    ]);
  });

  it("gives a declaration nothing in this app has ever heard of a step of its own", () => {
    // The property T069 is for: a new declaration reaches the wizard without
    // an edit to any file under pages/setup/.
    const steps = buildSetupSteps({
      requiredSettings: [setting("telemetry.endpoint", { kind: "URL" })],
      accountCreated: false,
      secondFactorConfirmed: false,
    });

    const derived = steps.filter((step) => step.kind === "settings");
    expect(derived).toHaveLength(1);
    expect(derived[0].settings[0].key).toBe("telemetry.endpoint");
  });

  it("uses the server's group when it sends one", () => {
    const steps = buildSetupSteps({
      requiredSettings: [
        setting("mail.host", { group: "Mail" }),
        setting("operator.name", { group: "Mail" }),
      ],
      accountCreated: false,
      secondFactorConfirmed: false,
    });

    expect(steps.filter((step) => step.kind === "settings")).toHaveLength(1);
  });
});

describe("FR-009: a setting the environment fixed", () => {
  const fixed = setting("support_email", {
    kind: "EMAIL",
    satisfied: true,
    source: "ENVIRONMENT",
    fixed_by: "THUNDERFORGE_SUPPORT_EMAIL",
  });

  it("is recognised as fixed", () => {
    expect(isFixedByEnvironment(fixed)).toBe(true);
    expect(isFixedByEnvironment(setting("operator.name"))).toBe(false);
  });

  it("is never asked for, but is still shown", () => {
    const [, step] = buildSetupSteps({
      requiredSettings: [fixed],
      accountCreated: false,
      secondFactorConfirmed: false,
    });

    expect(step.askable).toHaveLength(0);
    expect(step.fixed).toHaveLength(1);
    expect(step.skipped).toBe(true);
  });

  it("is never sent back to the server", () => {
    const [, step] = buildSetupSteps({
      requiredSettings: [fixed, setting("operator.name")],
      accountCreated: false,
      secondFactorConfirmed: false,
    });

    expect(
      stepPayload(step, {
        support_email: "someone@else.test",
        "operator.name": "The Guild",
      }),
    ).toEqual({});
  });

  it("leaves an instance configured wholly by environment two screens", () => {
    // FR-009's consequence, and the thing that makes the wizard bearable for a
    // container deployment: the account, the second factor, then the review.
    const steps = buildSetupSteps({
      requiredSettings: [
        setting("operator.name", {
          satisfied: true,
          source: "ENVIRONMENT",
          fixed_by: "THUNDERFORGE_OPERATOR_NAME",
        }),
        setting("operator.contact_email", {
          kind: "EMAIL",
          satisfied: true,
          source: "ENVIRONMENT",
          fixed_by: "THUNDERFORGE_OPERATOR_CONTACT_EMAIL",
        }),
        fixed,
      ],
      accountCreated: false,
      secondFactorConfirmed: false,
    });

    expect(visibleSteps(steps).map((step) => step.id)).toEqual([
      "account",
      "second-factor",
      "review",
    ]);
  });

  it("still offers a value the instance itself stored", () => {
    const stored = setting("operator.name", {
      satisfied: true,
      source: "INSTANCE",
      value: "The Guild",
    });

    const [, step] = buildSetupSteps({
      requiredSettings: [stored],
      accountCreated: false,
      secondFactorConfirmed: false,
    });

    expect(step.askable).toHaveLength(1);
    expect(initialValues([stored])).toEqual({ "operator.name": "The Guild" });
  });
});

describe("FR-006: where a resumed pass lands", () => {
  it("skips what is already answered", () => {
    const steps = buildSetupSteps({
      requiredSettings: [
        setting("operator.name", { satisfied: true, source: "INSTANCE" }),
        setting("notice.contact_email", { kind: "EMAIL" }),
      ],
      accountCreated: true,
      secondFactorConfirmed: false,
    });

    expect(visibleSteps(steps)[firstUnansweredStep(steps)].id).toBe(
      "settings-copyright-notices",
    );
  });

  it("lands on the review when everything is answered", () => {
    const steps = buildSetupSteps({
      requiredSettings: [
        setting("operator.name", { satisfied: true, source: "INSTANCE" }),
      ],
      accountCreated: true,
      secondFactorConfirmed: true,
    });

    expect(visibleSteps(steps)[firstUnansweredStep(steps)].kind).toBe("review");
  });
});

describe("FR-003: required against merely offered", () => {
  it("treats an entry with no requirement as required", () => {
    expect(blocksCompletion(setting("operator.name"))).toBe(true);
  });

  it("does not let an optional entry hold up completion", () => {
    expect(
      blocksCompletion(setting("mail.host", { requirement: "OPTIONAL" })),
    ).toBe(false);
  });

  it("lets a resumed pass leave an already-stored field blank", () => {
    // The status endpoint says `satisfied` without sending the value, so the
    // box is empty over a stored answer. Refusing it would mean retyping every
    // answer already given, which is FR-006 failing.
    const stored = setting("operator.name", {
      satisfied: true,
      source: "INSTANCE",
    });
    expect(validateSettingValue(stored, "")).toBeUndefined();
  });

  it("lets an optional field be left blank", () => {
    const optional = setting("mail.from_name", { requirement: "OPTIONAL" });
    expect(validateSettingValue(optional, "  ")).toBeUndefined();
    expect(validateSettingValue(setting("operator.name"), "  ")).toMatch(
      /required/i,
    );
  });
});

describe("FR-004: the refusals worth making before a round trip", () => {
  it("refuses an address at a reserved domain", () => {
    const email = setting("notice.contact_email", { kind: "EMAIL" });
    expect(validateSettingValue(email, "dmca@thunderforge.example")).toMatch(
      /reserved domain/i,
    );
    expect(
      validateSettingValue(email, "dmca@thunderforge.dev"),
    ).toBeUndefined();
  });

  it("refuses a port outside the range", () => {
    const port = setting("mail.port", { kind: "PORT" });
    expect(validateSettingValue(port, "0")).toMatch(/between 1 and 65535/);
    expect(validateSettingValue(port, "587")).toBeUndefined();
  });

  it("reports every refusal on a step, not the first", () => {
    const [, step] = buildSetupSteps({
      requiredSettings: [
        setting("operator.name"),
        setting("operator.contact_email", { kind: "EMAIL" }),
      ],
      accountCreated: false,
      secondFactorConfirmed: false,
    });

    expect(Object.keys(validateStep(step, {}))).toEqual([
      "operator.name",
      "operator.contact_email",
    ]);
  });
});

describe("stepPayload", () => {
  it("omits a blank optional value rather than sending an empty string", () => {
    const [, step] = buildSetupSteps({
      requiredSettings: [
        setting("mail.from_name", { requirement: "OPTIONAL" }),
        setting("mail.host", { requirement: "OPTIONAL" }),
      ],
      accountCreated: false,
      secondFactorConfirmed: false,
    });

    expect(stepPayload(step, { "mail.host": " smtp.example.org " })).toEqual({
      "mail.host": "smtp.example.org",
    });
  });
});

describe("settingLabel", () => {
  it("reads the key's last segment as prose", () => {
    expect(settingLabel("notice.contact_postal_address")).toBe(
      "Contact Postal Address",
    );
    expect(settingLabel("support_email")).toBe("Support Email");
  });
});

describe("normaliseReadiness", () => {
  it("accepts the GraphQL camel shape", () => {
    const report = normaliseReadiness({
      capabilities: [
        {
          key: "send_mail",
          label: "Send mail",
          available: false,
          gaps: [
            {
              settingKey: "mail.host",
              envVar: "THUNDERFORGE_MAIL_HOST",
              whatToSet: "The SMTP host.",
              whatIsLimited: "Nothing can be emailed.",
            },
          ],
        },
      ],
      fullyConfigured: false,
      unrecognisedSettings: [],
    });

    expect(report?.capabilities[0].gaps[0].envVar).toBe(
      "THUNDERFORGE_MAIL_HOST",
    );
  });

  it("accepts the REST snake shape the completion response carries", () => {
    const report = normaliseReadiness({
      capabilities: [
        {
          key: "send_mail",
          label: "Send mail",
          available: false,
          gaps: [
            {
              setting_key: "mail.host",
              env_var: "THUNDERFORGE_MAIL_HOST",
              what_to_set: "The SMTP host.",
              what_is_limited: "Nothing can be emailed.",
            },
          ],
        },
      ],
      fully_configured: false,
      unrecognised_settings: ["legacy.key"],
    });

    expect(report?.capabilities[0].gaps[0].settingKey).toBe("mail.host");
    expect(report?.unrecognisedSettings).toEqual(["legacy.key"]);
  });

  it("is null when there is no report", () => {
    expect(normaliseReadiness(null)).toBeNull();
    expect(normaliseReadiness({})).toBeNull();
  });
});
