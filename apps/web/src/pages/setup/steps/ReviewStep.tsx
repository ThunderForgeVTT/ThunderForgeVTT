import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import {
  blocksCompletion,
  isFixedByEnvironment,
  requirementLabel,
  settingLabel,
  type ReadinessReport,
  type RequiredSetting,
} from "@/services/instanceSetup";

/**
 * The last screen, and the honest one (T072; FR-003, FR-005 scenario 5).
 *
 * Setup ends by saying what is **still unset**, not by congratulating an
 * operator on a finished instance. That is the difference between "setup
 * complete" and "this instance can do these things and cannot do those", and
 * FR-025's reasoning is why an available capability is listed too: an empty
 * list of complaints reads as "we did not check".
 *
 * # Two sources for the same report
 *
 * `instanceReadiness` (GraphQL, admin-gated) is the live one and is readable
 * here because the account step signed us in as the first administrator.
 * `contracts/setup.md` rule 5 also says the `/complete` response carries the
 * report; when it does, that one wins, because it describes the instance as it
 * was at the instant setup finished. If neither is available — an older server,
 * a refused query — the unset list below still answers the question from
 * `required_settings`, which is the wizard's own source of truth anyway.
 */
export interface ReviewStepProps {
  requiredSettings: RequiredSetting[];
  readiness: ReadinessReport | null;
  completion: string | null;
  failure: string | null;
  /** Names the declarations `/complete` refused for, from its `409 incomplete`. */
  missing: string[];
  isCompleting: boolean;
  onComplete: () => void;
  onRevisit: (settingKey: string) => void;
}

export function ReviewStep({
  requiredSettings,
  readiness,
  completion,
  failure,
  missing,
  isCompleting,
  onComplete,
  onRevisit,
}: ReviewStepProps) {
  const unset = requiredSettings.filter((setting) => !setting.satisfied);
  const blocking = unset.filter(blocksCompletion);
  const fixed = requiredSettings.filter(isFixedByEnvironment);

  return (
    <div className="grid gap-6">
      {completion ? (
        <div data-testid="setup-complete-message">
          <StatusBadge variant="success">{completion}</StatusBadge>
        </div>
      ) : null}

      {failure ? (
        <div data-testid="setup-complete-error">
          <StatusBadge variant="danger">{failure}</StatusBadge>
        </div>
      ) : null}

      {missing.length > 0 ? (
        <ul data-testid="setup-complete-missing" className="grid gap-1 text-sm">
          {missing.map((key) => (
            <li key={key} data-testid={`setup-complete-missing-${key}`}>
              Still needed: <strong>{settingLabel(key)}</strong> ({key})
            </li>
          ))}
        </ul>
      ) : null}

      <section data-testid="setup-review-unset" className="grid gap-3">
        <h3 className="text-sm font-semibold">What is still unset</h3>
        {unset.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            Every setting this instance declares has a value. Nothing here is
            waiting on you.
          </p>
        ) : (
          <ul className="grid gap-2">
            {unset.map((setting) => (
              <li
                key={setting.key}
                data-testid={`setup-review-unset-${setting.key}`}
                className="grid gap-1 rounded-lg border border-border p-3 text-sm"
              >
                <span className="flex flex-wrap items-center gap-2">
                  <strong>{settingLabel(setting.key)}</strong>
                  <code className="font-mono text-xs text-muted-foreground">
                    {setting.key}
                  </code>
                  <span className="text-xs text-muted-foreground">
                    {requirementLabel(setting)}
                  </span>
                </span>
                {setting.what_to_set ? (
                  <span className="text-muted-foreground">
                    {setting.what_to_set}
                  </span>
                ) : null}
                <span>
                  <Button
                    data-testid={`setup-review-revisit-${setting.key}`}
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => onRevisit(setting.key)}
                  >
                    Go back and set it
                  </Button>
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>

      {fixed.length > 0 ? (
        <section data-testid="setup-review-fixed" className="grid gap-2">
          <h3 className="text-sm font-semibold">
            Fixed by this deployment&rsquo;s environment
          </h3>
          <ul className="grid gap-1 text-sm text-muted-foreground">
            {fixed.map((setting) => (
              <li key={setting.key}>
                {settingLabel(setting.key)} — set by{" "}
                <code className="font-mono">
                  {setting.fixed_by ?? "the environment"}
                </code>
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      {readiness ? (
        <section data-testid="setup-review-readiness" className="grid gap-3">
          <h3 className="text-sm font-semibold">
            What this instance can and cannot do
          </h3>
          <ul className="grid gap-2">
            {readiness.capabilities.map((capability) => (
              <li
                key={capability.key}
                data-testid={`setup-review-capability-${capability.key}`}
                data-available={capability.available ? "true" : "false"}
                className="grid gap-2 rounded-lg border border-border p-3 text-sm"
              >
                <span className="flex items-center justify-between gap-3">
                  <strong>{capability.label}</strong>
                  <StatusBadge
                    variant={capability.available ? "success" : "warning"}
                  >
                    {capability.available ? "Available" : "Limited"}
                  </StatusBadge>
                </span>
                {capability.gaps.length > 0 ? (
                  <ul className="grid gap-1 text-muted-foreground">
                    {capability.gaps.map((gap) => (
                      <li
                        key={gap.settingKey}
                        data-testid="setup-review-gap"
                        data-setting-key={gap.settingKey}
                      >
                        {gap.whatToSet}
                        {gap.envVar ? (
                          <>
                            {" "}
                            Set <code className="font-mono">
                              {gap.envVar}
                            </code>{" "}
                            or fill it in from the administration screen.
                          </>
                        ) : null}{" "}
                        <em>{gap.whatIsLimited}</em>
                      </li>
                    ))}
                  </ul>
                ) : null}
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      {completion ? null : (
        <div className="grid gap-2">
          <Button
            data-testid="setup-complete"
            type="button"
            variant="primary"
            size="lg"
            icon="shield"
            disabled={isCompleting}
            onClick={onComplete}
          >
            {isCompleting ? "Finishing setup..." : "Finish setup"}
          </Button>
          <p className="text-sm text-muted-foreground">
            {blocking.length > 0
              ? "Something setup requires is still unset. The instance will refuse to finish and will name it."
              : "Anything left unset can be filled in afterwards from the administration screen, without a restart."}
          </p>
        </div>
      )}
    </div>
  );
}
