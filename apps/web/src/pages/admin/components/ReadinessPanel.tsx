import { useCallback, useEffect, useState } from "react";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import {
  fetchInstanceReadiness,
  type InstanceReadiness,
} from "@/api/instanceSettings";

/**
 * Spec 040 US6: what this instance can and cannot do, given how it is
 * configured — and for each thing it cannot, the one sentence that fixes it.
 *
 * # Not a health check
 *
 * Nothing here is a probe. The report is derived from the resolved settings on
 * every read and never stored, so it cannot go stale and it cannot disagree
 * with the settings panel — they are two renderings of one resolution.
 *
 * # An unavailable capability is not an error
 *
 * FR-028 / SC-009: an unset setting is a readiness gap and a refusal at the
 * point of use. It is never a reason the server failed to start, and this
 * screen says so in that register — "mail is not configured, and here is what
 * that costs you", not a red failure. A fresh instance would otherwise open on
 * a wall of alarms describing nothing wrong.
 *
 * # Source flips and unrecognised keys
 *
 * Both are the upgrade story (FR-028). A value that used to come from the
 * environment and now comes from the instance changed hands without anyone
 * editing it, and an operator who does not know that will edit the wrong
 * place. An unrecognised stored key is usually a rename — it is reported, and
 * deliberately not deleted.
 */
export function ReadinessPanel() {
  const [readiness, setReadiness] = useState<InstanceReadiness | null>(null);
  const [error, setError] = useState<string | null>(null);

  // The promise-chain shape `GitHubAppsPanel` uses, so the effect body itself
  // sets no state: `react-hooks/set-state-in-effect` rejects the async-IIFE
  // form, and one loading idiom per admin panel is worth more than the
  // marginally shorter one.
  const load = useCallback(() => {
    fetchInstanceReadiness()
      .then((next) => {
        setReadiness(next);
        setError(null);
      })
      .catch((cause: unknown) => {
        setError(
          cause instanceof Error
            ? cause.message
            : "This instance's readiness could not be read.",
        );
      });
  }, []);

  useEffect(load, [load]);

  if (error) {
    return <StatusBadge variant="danger">{error}</StatusBadge>;
  }

  if (!readiness) {
    return <p className="text-muted-foreground">Reading this instance...</p>;
  }

  return (
    <div className="grid gap-4" data-testid="readiness-panel">
      <div data-testid="readiness-summary">
        <StatusBadge variant={readiness.fullyConfigured ? "success" : "info"}>
          {readiness.fullyConfigured
            ? "Everything this instance declares is configured."
            : "This instance runs. Some capabilities are not configured yet."}
        </StatusBadge>
      </div>

      <div className="grid gap-3">
        {readiness.capabilities.map((capability) => (
          <div
            key={capability.key}
            className="grid gap-2 rounded-lg border border-border bg-secondary/40 p-4"
            data-testid={`readiness-capability-${capability.key}`}
            data-available={capability.available ? "true" : "false"}
          >
            <div className="flex flex-wrap items-center justify-between gap-2">
              <p className="font-semibold">{capability.label}</p>
              <StatusBadge
                variant={capability.available ? "success" : "warning"}
              >
                {capability.available ? "Available" : "Not configured"}
              </StatusBadge>
            </div>
            {capability.gaps.length ? (
              <ul className="grid gap-2">
                {capability.gaps.map((gap) => (
                  <li
                    key={gap.settingKey}
                    className="grid gap-0.5 text-sm"
                    data-testid={`readiness-gap-${gap.settingKey}`}
                  >
                    {/* `whatToSet` and `whatIsLimited` are rendered verbatim.
                        They are written in the registry to name a variable and
                        never a value, and rephrasing them here would be a
                        second place for that rule to be got wrong. */}
                    <p>{gap.whatToSet}</p>
                    <p className="text-muted-foreground">
                      {gap.whatIsLimited}
                      {gap.envVar
                        ? ` Environment variable: ${gap.envVar}.`
                        : ""}
                    </p>
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        ))}
      </div>

      {readiness.sourceFlips.length ? (
        <div
          className="grid gap-2 rounded-lg border border-primary/20 bg-primary/5 p-4"
          data-testid="readiness-source-flips"
        >
          <h4 className="font-semibold">Values that changed hands</h4>
          <p className="text-muted-foreground">
            These resolve from somewhere different than they used to. Nobody
            edited them; the environment did.
          </p>
          <ul className="grid gap-1 text-sm">
            {readiness.sourceFlips.map((flip) => (
              <li key={flip.settingKey}>
                <strong>{flip.settingKey}</strong>: {flip.was} → {flip.now}
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      {readiness.unrecognisedSettings.length ? (
        <div
          className="grid gap-2 rounded-lg border border-border p-4"
          data-testid="readiness-unrecognised"
        >
          <h4 className="font-semibold">
            Stored keys this version does not know
          </h4>
          <p className="text-muted-foreground">
            Kept, not deleted — usually a setting that was renamed. Nothing
            reads them.
          </p>
          <ul className="grid gap-1 text-sm">
            {readiness.unrecognisedSettings.map((key) => (
              <li key={key}>{key}</li>
            ))}
          </ul>
        </div>
      ) : null}
    </div>
  );
}
