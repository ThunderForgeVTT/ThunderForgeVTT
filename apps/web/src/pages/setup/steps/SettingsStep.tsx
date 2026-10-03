import { useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { AccessConsequences } from "@/pages/setup/steps/AccessConsequences";
import { DesignatedAgentNotice } from "@/pages/setup/steps/DesignatedAgentNotice";
import { SettingField } from "@/pages/setup/steps/SettingField";
import { settingLabel, type SetupStep } from "@/services/instanceSetup";
import type { StorageConnectionReport } from "@/types/admin";

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
  /**
   * Save this step and ask the server whether the object store answers.
   *
   * Only called from the step that collects storage, and only when the
   * operator presses for it. Passed in rather than imported here because the
   * answers have to be written before they can be tested, and only the page
   * knows how to write a step.
   */
  onTestStorage?: () => Promise<StorageConnectionReport>;
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

/**
 * Does this step collect where uploads are kept?
 *
 * Same reasoning as `mentionsNoticeContact`: keyed off the declaration
 * prefix, so the connection test follows the settings if the registry moves
 * them to another group — including onto the merged final step, which is
 * where they are today.
 */
function mentionsStorage(step: SetupStep): boolean {
  return step.settings.some((setting) => setting.key.startsWith("storage."));
}

/** Does this step decide who may join? */
function mentionsAccessPolicy(step: SetupStep): boolean {
  return step.settings.some(
    (setting) => setting.key === "instance.access_policy",
  );
}

/** As the server reads a boolean setting's value. */
function isTruthy(value: string | undefined): boolean {
  return ["true", "1", "yes", "on"].includes(
    (value ?? "").trim().toLowerCase(),
  );
}

export function SettingsStep({
  step,
  values,
  errors,
  onChange,
  onTestStorage,
}: SettingsStepProps) {
  const [probe, setProbe] = useState<StorageConnectionReport | null>(null);
  const [probeFailure, setProbeFailure] = useState<string | null>(null);
  const [isProbing, setIsProbing] = useState(false);

  const runProbe = async () => {
    if (!onTestStorage) {
      return;
    }
    setIsProbing(true);
    setProbe(null);
    setProbeFailure(null);
    try {
      setProbe(await onTestStorage());
    } catch (error) {
      setProbeFailure(
        error instanceof Error ? error.message : "The test could not be run.",
      );
    } finally {
      setIsProbing(false);
    }
  };

  return (
    <div className="grid gap-6">
      {/*
       * Why before what. Spec 052 FR-032 asks for the reason an answer is
       * wanted to be in front of the operator *before* they are asked for it,
       * and the step is the only place that is true of — a hint under a field
       * is read after the question has already landed as an imposition.
       */}
      {step.explainer ? (
        <p
          data-testid="setup-group-explainer"
          data-group={step.id}
          className="rounded-lg border border-border bg-muted/40 p-3 text-sm text-muted-foreground"
        >
          {step.explainer}
        </p>
      ) : null}

      {mentionsAccessPolicy(step) ? (
        <AccessConsequences
          policy={values["instance.access_policy"]}
          publishes={isTruthy(values["instance.publishes_beyond_world"])}
        />
      ) : null}

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

      {mentionsStorage(step) && onTestStorage ? (
        <section data-testid="setup-storage-probe" className="grid gap-3">
          <p className="text-sm text-muted-foreground">
            Saving these and testing them is the difference between finding out
            now and finding out the first time somebody uploads a map.
          </p>
          <div className="flex flex-wrap items-center gap-3">
            <Button
              type="button"
              variant="secondary"
              icon="wand"
              data-testid="setup-storage-probe-run"
              disabled={isProbing}
              onClick={() => void runProbe()}
            >
              {isProbing ? "Testing..." : "Save and test storage"}
            </Button>
            {probe ? (
              <StatusBadge
                variant={probe.reachable ? "success" : "danger"}
                data-testid="setup-storage-probe-result"
              >
                {probe.reachable
                  ? `Reached ${probe.bucket} at ${probe.endpoint}.`
                  : `Could not use ${probe.bucket} at ${probe.endpoint}: ${probe.detail ?? "no detail given"}`}
              </StatusBadge>
            ) : null}
            {probeFailure ? (
              <StatusBadge
                variant="danger"
                data-testid="setup-storage-probe-failure"
              >
                {probeFailure}
              </StatusBadge>
            ) : null}
          </div>
        </section>
      ) : null}

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
