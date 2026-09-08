import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { DesignatedAgentNotice } from "@/pages/setup/steps/DesignatedAgentNotice";
import { SettingField } from "@/pages/setup/steps/SettingField";
import { settingLabel, type SetupStep } from "@/services/instanceSetup";

/**
 * One step of the wizard, for one registry group.
 *
 * There is exactly one of these components and there are as many steps as the
 * registry produces groups. That is T069's whole point: adding a declaration
 * adds a field (or a step, if it opens a new group) with no edit here.
 */
export interface SettingsStepProps {
  step: SetupStep;
  values: Record<string, string>;
  errors: Record<string, string>;
  onChange: (key: string, value: string) => void;
}

/**
 * Does this step collect the address a copyright notice is served on?
 *
 * Keyed off the declaration prefix rather than off a step called "notices",
 * because the wizard has no named steps to key off. If the registry renames
 * the group, the obligation notice follows the settings; if it moves
 * `notice.contact_email` to a different group, the notice moves with it.
 */
function mentionsNoticeContact(step: SetupStep): boolean {
  return step.settings.some((setting) => setting.key.startsWith("notice."));
}

export function SettingsStep({
  step,
  values,
  errors,
  onChange,
}: SettingsStepProps) {
  return (
    <div className="grid gap-6">
      {mentionsNoticeContact(step) ? <DesignatedAgentNotice /> : null}

      {step.askable.length > 0 ? (
        <div className="grid gap-4">
          {step.askable.map((setting) => (
            <SettingField
              key={setting.key}
              setting={setting}
              value={values[setting.key] ?? ""}
              error={errors[setting.key]}
              onChange={onChange}
            />
          ))}
        </div>
      ) : (
        <StatusBadge variant="info">
          Everything on this step is already set by the environment. There is
          nothing to answer here.
        </StatusBadge>
      )}

      {step.fixed.length > 0 ? (
        /*
         * FR-009: told it is fixed, and where it comes from — never offered as
         * an editable field. An edit here would be refused by the server with
         * `409 fixed_by_environment` anyway, so an input would be a lie with a
         * round trip attached.
         */
        <section data-testid="setup-fixed-settings" className="grid gap-3">
          <h3 className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Fixed by this deployment&rsquo;s environment
          </h3>
          <ul className="grid gap-2">
            {step.fixed.map((setting) => (
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
                  . Change it where that variable is set, then restart this
                  instance. It cannot be edited here.
                </span>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
    </div>
  );
}
