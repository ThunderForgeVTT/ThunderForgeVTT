/**
 * Spec 040 FR-009, FR-023 and T032, from the panel's side.
 *
 * # Why this renders to markup
 *
 * `apps/web` has neither jsdom nor testing-library, so components here are
 * server-rendered and read as markup — the precedent `feedbackReview` and
 * `packSurfaceBoundary` set. It suits what needs asserting: every claim below
 * is about what is *on the screen*, and two of them are about what is not.
 *
 * # What each case is guarding
 *
 * - **A fixed setting names the variable.** The panel must not render a
 *   disabled box with no explanation; FR-009's whole content is the sentence,
 *   not the disabling. A test that only checked "no input" would pass for the
 *   silently-greyed version this requirement exists to reject.
 * - **A secret is two words.** "Set" or "Not set" — no masked form, no
 *   truncation, no length. Asserted by planting a value the panel could only
 *   render if it had one, and requiring its absence.
 * - **A redacted change says a change happened.** The failure to catch is a
 *   history that renders an absent value as an empty string, which reads as
 *   "somebody set this to nothing" — the opposite of the truth.
 */

import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import {
  SettingChangeRow,
  SettingRow,
  asRequiredSetting,
} from "../InstanceSettingsPanel";
import type { ResolvedSetting, SettingChange } from "@/api/instanceSettings";

const PLANTED_SECRET = "hunter2-do-not-render-me";

function setting(overrides: Partial<ResolvedSetting> = {}): ResolvedSetting {
  return {
    key: "operator.name",
    kind: "TEXT",
    enumOptions: [],
    value: "Thunder Collective",
    secretState: null,
    source: "INSTANCE",
    fixedBy: null,
    editable: true,
    requirement: "REQUIRED_AT_SETUP",
    capability: null,
    whatToSet: "The name of the person or body that runs this instance.",
    group: "Operator",
    undecryptable: false,
    ...overrides,
  };
}

function change(overrides: Partial<SettingChange> = {}): SettingChange {
  return {
    key: "operator.name",
    previousValue: "Old Name",
    newValue: "Thunder Collective",
    redacted: false,
    changedBy: "01a0-admin",
    changedAt: "2026-09-08T09:00:00Z",
    source: "ADMIN",
    ...overrides,
  };
}

const render = (node: Parameters<typeof renderToStaticMarkup>[0]) =>
  renderToStaticMarkup(node);

describe("InstanceSettingsPanel — a setting's rendering", () => {
  it("offers a field for an editable setting", () => {
    const markup = render(
      <SettingRow setting={setting()} onSaved={() => {}} />,
    );

    expect(markup).toContain('data-testid="setup-setting-operator.name"');
    expect(markup).toContain('data-editable="true"');
    expect(markup).toContain("Set on this instance");
  });

  it("names the variable that fixed a setting, instead of a field with no explanation", () => {
    const markup = render(
      <SettingRow
        setting={setting({
          value: "Fixed Collective",
          source: "ENVIRONMENT",
          fixedBy: "THUNDERFORGE_OPERATOR_NAME",
          editable: false,
        })}
        onSaved={() => {}}
      />,
    );

    expect(markup).toContain("Fixed by THUNDERFORGE_OPERATOR_NAME");
    expect(markup).toContain("Unset that variable to edit it here");
    expect(markup).toContain('data-editable="false"');
    // No control at all — not a disabled one.
    expect(markup).not.toContain("<input");
    expect(markup).not.toContain('data-testid="instance-setting-save-');
  });

  it("says only whether a secret is set, and never any part of it", () => {
    const markup = render(
      <SettingRow
        setting={setting({
          key: "mail.password",
          group: "Mail",
          // A secret arrives with `value: null` from the server. Planting one
          // here proves the component would not render it even if it did.
          value: PLANTED_SECRET,
          secretState: "SET",
          requirement: "OPTIONAL",
        })}
        onSaved={() => {}}
      />,
    );

    expect(markup).not.toContain(PLANTED_SECRET);
    expect(markup).not.toContain("hunter2");
    expect(markup).toContain("Set");
  });

  it("says a stored value has become unreadable rather than showing it as unset", () => {
    const markup = render(
      <SettingRow
        setting={setting({ undecryptable: true, value: null })}
        onSaved={() => {}}
      />,
    );

    expect(markup).toContain("the instance secret changed since it was");
  });

  it("carries a secret's shape across to the field it renders", () => {
    const adapted = asRequiredSetting(
      setting({ value: null, secretState: "NOT_SET" }),
    );

    expect(adapted.secret).toBe(true);
    expect(adapted.satisfied).toBe(false);
  });
});

describe("InstanceSettingsPanel — a setting's history", () => {
  it("shows what a value was and what it became", () => {
    const markup = render(
      <SettingChangeRow settingKey="operator.name" change={change()} />,
    );

    expect(markup).toContain("Old Name");
    expect(markup).toContain("Thunder Collective");
    expect(markup).toContain('data-redacted="false"');
  });

  it("marks a redacted change as one, rather than rendering it as a change to nothing", () => {
    const markup = render(
      <SettingChangeRow
        settingKey="mail.password"
        change={change({
          key: "mail.password",
          previousValue: null,
          newValue: null,
          redacted: true,
        })}
      />,
    );

    expect(markup).toContain('data-redacted="true"');
    expect(markup).toContain("A secret was changed");
    expect(markup).toContain("not recorded");
    // The failure this exists to catch: an absent value drawn as "unset",
    // which reads as somebody having cleared the password.
    expect(markup).not.toContain("unset");
  });

  it("attributes a change the instance made itself, rather than to nobody", () => {
    const markup = render(
      <SettingChangeRow
        settingKey="operator.name"
        change={change({ changedBy: null, source: "SETUP" })}
      />,
    );

    expect(markup).toContain("the instance itself");
  });
});
