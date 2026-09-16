import { useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import {
  fetchInstanceReadiness,
  fetchInstanceSettings,
  type Capability,
  type InstanceReadiness,
  type ReadinessGap,
  type ResolvedSetting,
} from "@/api/instanceSettings";
import { groupOf } from "@/services/instanceSetup";
import { asRequiredSetting, instanceGroupPath } from "./InstanceSettingsPanel";

/**
 * Spec 040 US6: what this instance can and cannot do, given how it is
 * configured — and for each thing it cannot, the one sentence that fixes it,
 * and the link that takes you to where you fix it.
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
 * # Why it now reads the settings too
 *
 * The owner's complaint was that the screen names what is missing and then
 * leaves you to find it: "it needs like a call to action, like a link on how
 * to set those things up". A link is only honest if it leads to a control
 * that exists, and whether a control exists is `editable` — from the server,
 * on the settings query, and nowhere else (FR-009). So the panel reads both
 * and links only where there is something to click. A gap the environment has
 * fixed names its variable instead, because that is where the change has to
 * be made and no page of this app can make it.
 *
 * # Two lists, not one
 *
 * What is missing comes first and what already works comes second. The
 * registry's own order is the order an operator should *read* them in, which
 * is the right order for a report and the wrong one for a to-do list — it
 * scatters the three things needing attention among the three that don't.
 * Both lists are always rendered: FR-025 scenario 3 wants the positive
 * assertion, and an omitted capability reads as "we did not check".
 *
 * # Source flips and unrecognised keys
 *
 * Both are the upgrade story (FR-028). A value that used to come from the
 * environment and now comes from the instance changed hands without anyone
 * editing it, and an operator who does not know that will edit the wrong
 * place. An unrecognised stored key is usually a rename — it is reported, and
 * deliberately not deleted.
 */

/** Where the control for this setting actually lives, or nothing. */
interface GapDestination {
  href: string;
  label: string;
}

function destinationFor(
  gap: ReadinessGap,
  setting: ResolvedSetting | undefined,
): GapDestination | null {
  // FR-009. No link to a control that is not there — a dead end dressed as a
  // call to action is worse than no call to action.
  if (setting && !setting.editable) {
    return null;
  }
  const group = setting ? groupOf(asRequiredSetting(setting)) : null;
  // Mail's settings are set up on the mail page now, beside the tester that
  // proves them, so that is where "set this" has to point.
  if (group === "Mail" || gap.settingKey.startsWith("mail.")) {
    return {
      href: `/admin/mail#setting-${gap.settingKey}`,
      label: "Set it up in Mail",
    };
  }
  // The nine GitHub keys are deliberately absent from the instance panel —
  // an application is resolved whole, and offering its keys one at a time is
  // the field-at-a-time write FR-021 forbids. So a link to their group there
  // would land on a group that is not rendered; Configuration is where they
  // actually are.
  if (
    group === "GitHub applications" ||
    gap.settingKey.startsWith("github_app")
  ) {
    return {
      href: "/admin/configuration#github-applications",
      label: "Set it up in Configuration",
    };
  }
  if (!group) {
    return { href: "/admin/instance", label: "Set it in Instance settings" };
  }
  return {
    href: `${instanceGroupPath(group)}#setting-${gap.settingKey}`,
    label: `Set it in ${group}`,
  };
}

function CapabilityCard({
  capability,
  settingsByKey,
}: {
  capability: Capability;
  settingsByKey: Map<string, ResolvedSetting>;
}) {
  return (
    <div
      className="grid gap-3 rounded-lg border border-border bg-secondary/40 p-4"
      data-testid={`readiness-capability-${capability.key}`}
      data-available={capability.available ? "true" : "false"}
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="font-semibold">{capability.label}</p>
        <StatusBadge variant={capability.available ? "success" : "warning"}>
          {capability.available ? "Available" : "Not configured"}
        </StatusBadge>
      </div>
      {capability.gaps.length ? (
        <ul className="grid gap-3">
          {capability.gaps.map((gap) => {
            const setting = settingsByKey.get(gap.settingKey);
            const destination = destinationFor(gap, setting);
            return (
              <li
                key={gap.settingKey}
                className="grid gap-1 border-t border-border/60 pt-3 text-sm first:border-t-0 first:pt-0"
                data-testid={`readiness-gap-${gap.settingKey}`}
              >
                {/* `whatToSet` and `whatIsLimited` are rendered verbatim.
                    They are written in the registry to name a variable and
                    never a value, and rephrasing them here would be a
                    second place for that rule to be got wrong. */}
                <p>{gap.whatToSet}</p>
                <p className="text-muted-foreground">
                  {gap.whatIsLimited}
                  {gap.envVar ? ` Environment variable: ${gap.envVar}.` : ""}
                </p>
                {destination ? (
                  <p>
                    <Link
                      to={destination.href}
                      data-testid={`readiness-gap-link-${gap.settingKey}`}
                      className="inline-flex items-center gap-1.5 font-medium text-primary underline-offset-4 hover:underline"
                    >
                      <FantasyIcon name="compass" size={14} />
                      {destination.label}
                    </Link>
                  </p>
                ) : (
                  <p
                    className="text-muted-foreground"
                    data-testid={`readiness-gap-fixed-${gap.settingKey}`}
                  >
                    {setting?.fixedBy
                      ? `Fixed by ${setting.fixedBy}. Change it where that variable is set — this instance cannot.`
                      : "Not set from this screen. It is written through another surface."}
                  </p>
                )}
              </li>
            );
          })}
        </ul>
      ) : (
        <p className="text-sm text-muted-foreground">
          Everything this capability needs is set.
        </p>
      )}
    </div>
  );
}

export function ReadinessPanel() {
  const [readiness, setReadiness] = useState<InstanceReadiness | null>(null);
  const [settings, setSettings] = useState<ResolvedSetting[]>([]);
  const [error, setError] = useState<string | null>(null);

  // The promise-chain shape `GitHubAppsPanel` uses, so the effect body itself
  // sets no state: `react-hooks/set-state-in-effect` rejects the async-IIFE
  // form, and one loading idiom per admin panel is worth more than the
  // marginally shorter one.
  const load = useCallback(() => {
    Promise.all([fetchInstanceReadiness(), fetchInstanceSettings()])
      .then(([nextReadiness, nextSettings]) => {
        setReadiness(nextReadiness);
        setSettings(nextSettings);
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

  const settingsByKey = useMemo(
    () => new Map(settings.map((setting) => [setting.key, setting])),
    [settings],
  );

  const missing = readiness?.capabilities.filter((c) => !c.available) ?? [];
  const working = readiness?.capabilities.filter((c) => c.available) ?? [];

  if (error) {
    return <StatusBadge variant="danger">{error}</StatusBadge>;
  }

  if (!readiness) {
    return <p className="text-muted-foreground">Reading this instance...</p>;
  }

  return (
    <div className="grid gap-5" data-testid="readiness-panel">
      <div className="grid gap-2" data-testid="readiness-summary">
        <StatusBadge variant={readiness.fullyConfigured ? "success" : "info"}>
          {readiness.fullyConfigured
            ? "Everything this instance declares is configured."
            : "This instance runs. Some capabilities are not configured yet."}
        </StatusBadge>
        <p className="max-w-[70ch] text-sm text-muted-foreground">
          <strong className="tabular-nums text-foreground">
            {working.length} of {readiness.capabilities.length}
          </strong>{" "}
          capabilities are available. Nothing below is a failure: an
          unconfigured capability is refused where it is used and never stops
          the instance. Each gap says what to set and links to where you set it.
        </p>
      </div>

      {missing.length ? (
        <section className="grid gap-3" data-testid="readiness-missing">
          <h4 className="text-sm font-semibold tracking-wider text-muted-foreground uppercase">
            Not configured yet ({missing.length})
          </h4>
          {missing.map((capability) => (
            <CapabilityCard
              key={capability.key}
              capability={capability}
              settingsByKey={settingsByKey}
            />
          ))}
        </section>
      ) : null}

      {working.length ? (
        <section className="grid gap-3" data-testid="readiness-working">
          <h4 className="text-sm font-semibold tracking-wider text-muted-foreground uppercase">
            Working ({working.length})
          </h4>
          {working.map((capability) => (
            <CapabilityCard
              key={capability.key}
              capability={capability}
              settingsByKey={settingsByKey}
            />
          ))}
        </section>
      ) : null}

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
