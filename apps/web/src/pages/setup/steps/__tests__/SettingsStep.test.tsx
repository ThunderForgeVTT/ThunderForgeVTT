import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SettingsStep } from "@/pages/setup/steps/SettingsStep";
import {
  buildSetupSteps,
  requirementLabel,
  type RequiredSetting,
} from "@/services/instanceSetup";

function setting(
  key: string,
  group: string,
  extra: Partial<RequiredSetting> = {},
): RequiredSetting {
  return {
    key,
    kind: "TEXT",
    satisfied: false,
    source: null,
    fixed_by: null,
    requirement: "OPTIONAL",
    group,
    essential: true,
    ...extra,
  };
}

const SETTINGS: RequiredSetting[] = [
  setting("instance.access_policy", "Access", {
    kind: "ENUM",
    options: ["invite_only", "open", "closed"],
  }),
  setting("instance.publishes_beyond_world", "Access", { kind: "BOOL" }),
  setting("operator.name", "Operator", { requirement: "REQUIRED_AT_SETUP" }),
  setting("notice.contact_email", "Copyright notices", {
    kind: "EMAIL",
    requirement: "REQUIRED_FOR",
    capability: "publish_beyond_world",
  }),
  setting("mail.host", "Mail", {
    requirement: "REQUIRED_FOR",
    capability: "send_mail",
  }),
  setting("github_app.sync.slug", "GitHub applications", { essential: false }),
  setting("github_app.feedback.slug", "GitHub applications", {
    essential: false,
  }),
  setting("realm_name", "Realm", { essential: false }),
];

function render(id: string, values: Record<string, string> = {}): string {
  const step = buildSetupSteps({
    requiredSettings: SETTINGS,
    accountCreated: true,
    secondFactorConfirmed: false,
  }).find((candidate) => candidate.id === id);
  if (!step) {
    throw new Error(`no step ${id}`);
  }
  return renderToStaticMarkup(
    <SettingsStep
      step={step}
      values={values}
      errors={{}}
      onChange={() => {}}
    />,
  );
}

describe("who may join", () => {
  it("offers each policy as a card to press, not as a dropdown", () => {
    const html = render("settings-access", {
      "instance.access_policy": "invite_only",
    });

    expect(html).not.toContain("<select");
    for (const policy of ["invite_only", "open", "closed"]) {
      expect(html).toContain(`setup-access-consequence-${policy}`);
    }
    expect(html).toMatch(
      /aria-checked="true"[^>]*data-testid="setup-access-consequence-invite_only"/,
    );
    expect(html).toContain("1. Who should be able to get an account here?");
    expect(html).toContain("2. Will anything here be shared");
  });

  it("says what sharing means while sign-ups are not open", () => {
    const html = render("settings-access", {
      "instance.access_policy": "invite_only",
      "instance.publishes_beyond_world": "true",
    });

    expect(html).toContain("setup-invite-publishing-note");
    expect(html).not.toContain("setup-open-publishing-warning");
  });

  it("warns when an open instance also shares", () => {
    const html = render("settings-access", {
      "instance.access_policy": "open",
      "instance.publishes_beyond_world": "true",
    });

    expect(html).toContain("setup-open-publishing-warning");
    expect(html).toContain("DMCA");
  });
});

describe("who runs this instance", () => {
  it("says a private instance does not show it to the signed-out", () => {
    expect(render("settings-operator")).toContain(
      "none of this is shown to anyone who is not signed in",
    );
  });

  it("says an open instance publishes it", () => {
    expect(
      render("settings-operator", { "instance.access_policy": "open" }),
    ).toContain("public legal pages");
  });
});

describe("a setting a feature needs", () => {
  it("is called optional, and says what for", () => {
    expect(requirementLabel(SETTINGS[4])).toBe(
      "Optional · needed to send mail",
    );
    expect(render("settings-mail")).not.toContain("Needed for a feature");
  });

  it("offers Resend on the mail step", () => {
    expect(render("settings-mail")).toContain("setup-mail-resend-apply");
  });

  it("calls the copyright contact optional while invite-only", () => {
    const html = render("settings-copyright-notices");
    expect(html).toContain('data-policy="invite_only"');
    expect(html).toContain("Optional while only people you invited can join");
  });
});

describe("the optional extras", () => {
  it("are one closed section per group", () => {
    const html = render("settings-anything-else");

    expect(html).toContain("setup-extras-toggle-github-applications");
    expect(html).toContain("setup-extras-toggle-realm");
    expect(html).not.toContain("setup-setting-realm_name");
  });
});
