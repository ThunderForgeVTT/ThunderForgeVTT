import { useState } from "react";
import { SettingField } from "@/pages/setup/steps/SettingField";
import {
  groupExplainer,
  groupOf,
  groupTitle,
  isFixedByEnvironment,
  settingLabel,
  slugify,
  type RequiredSetting,
  type SetupStep,
} from "@/services/instanceSetup";

/**
 * The optional extras, one closed section per registry group.
 *
 * They are still one step: none of them is a question this instance needs
 * answered, and a screen each would make four optional things look like four
 * tasks. But as one flat form they were twenty unrelated fields under a single
 * heading, three of them called "Client Id". So each group is its own section
 * with its own sentence of why, closed until the operator opens it, and a
 * group whose keys carry a scope (`github_app.sync.slug`) is split again by
 * that scope.
 *
 * A section holding a refused field is open whether or not it was opened, so
 * an error is never behind a closed door.
 */
export interface ExtrasSectionsProps {
  step: SetupStep;
  values: Record<string, string>;
  errors: Record<string, string>;
  onChange: (key: string, value: string) => void;
}

/** The middle segment of a three-part key, or nothing. */
function scopeOf(key: string): string {
  const parts = key.split(".");
  return parts.length >= 3 ? parts[1] : "";
}

function titleCase(value: string): string {
  return value
    .split(/[._-]/)
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

function summary(settings: RequiredSetting[]): string {
  const set = settings.filter((setting) => setting.satisfied).length;
  const count = `${settings.length} setting${settings.length === 1 ? "" : "s"}`;
  return set === 0 ? `${count}, none set` : `${count}, ${set} already set`;
}

export function ExtrasSections({
  step,
  values,
  errors,
  onChange,
}: ExtrasSectionsProps) {
  const [opened, setOpened] = useState<Record<string, boolean>>({});

  const groups: string[] = [];
  const byGroup = new Map<string, RequiredSetting[]>();
  for (const setting of step.settings) {
    const group = groupOf(setting);
    if (!byGroup.has(group)) {
      byGroup.set(group, []);
      groups.push(group);
    }
    byGroup.get(group)?.push(setting);
  }

  return (
    <div className="grid gap-3" data-testid="setup-extras">
      {groups.map((group) => {
        const settings = byGroup.get(group) ?? [];
        const slug = slugify(group);
        const askable = settings.filter((s) => !isFixedByEnvironment(s));
        const fixed = settings.filter(isFixedByEnvironment);
        const refused = askable.some((setting) => errors[setting.key]);
        const open = refused || Boolean(opened[group]);
        const explainer = groupExplainer(group);

        const scopes: string[] = [];
        for (const setting of askable) {
          const scope = scopeOf(setting.key);
          if (!scopes.includes(scope)) {
            scopes.push(scope);
          }
        }

        return (
          <section
            key={group}
            data-testid={`setup-extras-section-${slug}`}
            className="rounded-lg border border-border"
          >
            <button
              type="button"
              aria-expanded={open}
              aria-controls={`setup-extras-body-${slug}`}
              data-testid={`setup-extras-toggle-${slug}`}
              className="flex w-full items-center justify-between gap-3 rounded-lg p-3 text-left outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
              onClick={() =>
                setOpened((current) => ({ ...current, [group]: !open }))
              }
            >
              <span className="grid gap-0.5">
                <span className="font-semibold">{groupTitle(group)}</span>
                <span className="text-sm text-muted-foreground">
                  {fixed.length === settings.length
                    ? "Set by this deployment’s environment"
                    : summary(settings)}
                </span>
              </span>
              <span className="text-sm text-primary">
                {open ? "Close" : "Open"}
              </span>
            </button>

            {open ? (
              <div
                id={`setup-extras-body-${slug}`}
                className="grid gap-4 border-t border-border p-3"
              >
                {explainer ? (
                  <p className="text-sm text-muted-foreground">{explainer}</p>
                ) : null}

                {scopes.map((scope) => (
                  <div key={scope || "all"} className="grid gap-4">
                    {scope && scopes.length > 1 ? (
                      <h4 className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                        {titleCase(scope)}
                      </h4>
                    ) : null}
                    {askable
                      .filter((setting) => scopeOf(setting.key) === scope)
                      .map((setting) => (
                        <SettingField
                          key={setting.key}
                          setting={setting}
                          value={values[setting.key] ?? ""}
                          error={errors[setting.key]}
                          onChange={onChange}
                        />
                      ))}
                  </div>
                ))}

                {fixed.length > 0 ? (
                  <ul className="grid gap-2">
                    {fixed.map((setting) => (
                      <li
                        key={setting.key}
                        data-testid={`setup-setting-fixed-${setting.key}`}
                        className="grid gap-1 rounded-lg border border-border bg-muted/40 p-3 text-sm"
                      >
                        <span className="font-semibold">
                          {settingLabel(setting.key)}
                        </span>
                        <span className="text-muted-foreground">
                          Set by{" "}
                          <code className="font-mono">
                            {setting.fixed_by ?? "the environment"}
                          </code>
                          . Change it where that variable is set, then restart
                          this instance. It cannot be edited here.
                        </span>
                      </li>
                    ))}
                  </ul>
                ) : null}
              </div>
            ) : null}
          </section>
        );
      })}
    </div>
  );
}
