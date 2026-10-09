import type { TelemetryStatus } from "@/api/telemetry";
import { useTelemetryStatus } from "@/hooks/useTelemetryStatus";

/**
 * Spec 086 FR-035.4, Appendix A.4: where this instance's telemetry goes.
 *
 * Read-only on purpose. Telemetry is set by the environment and fixed at
 * startup, so a toggle here would be a control that lies; the panel names
 * the variables to set instead. The wording is Appendix A.4's, and a test
 * compares it with the spec.
 */

export const PROJECT_ENDPOINT = "https://telemetry.thunderforge.dev";

export const ANONYMOUS_TEXT =
  `This instance sends anonymous diagnostics to the ThunderForge project at ${PROJECT_ENDPOINT}: ` +
  "errors with personal details removed, timings, counts and versions. " +
  "Nothing anyone types, rolls, names or uploads, and no emails, ids, IP addresses or machine hostnames; " +
  "browsers' reports name your site's domain. Kept 14 days.";

export const redirectedText = (destination: string) =>
  `This instance sends its telemetry to ${destination}, your own collector, and nothing to the ThunderForge project.`;

export const OFF_TEXT = "Telemetry is off. Nothing is sent anywhere.";

export const CHANGE_TEXT =
  "To change this, set OTEL_EXPORTER_OTLP_ENDPOINT (server) or " +
  "THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT (browsers) to your own collector, " +
  "or TELEMETRY=false to turn it off, and restart.";

export const GUIDE_URL =
  "https://github.com/ThunderForgeVTT/ThunderForgeVTT/blob/main/docs/guides/telemetry.md";

type State = "anonymous" | "redirected" | "off";

/** Anonymous if either row reaches the project; otherwise redirected or off. */
export function stateOf(s: TelemetryStatus): State {
  if (!s.enabled || (s.serverTier === "off" && s.browserTier === "off")) {
    return "off";
  }
  if (s.serverTier === "anonymous" || s.browserTier === "anonymous") {
    return "anonymous";
  }
  return "redirected";
}

function summary(s: TelemetryStatus): string {
  switch (stateOf(s)) {
    case "off":
      return OFF_TEXT;
    case "anonymous":
      return ANONYMOUS_TEXT;
    case "redirected":
      return redirectedText(s.serverEndpoint ?? s.browserEndpoint ?? "");
  }
}

function Row(props: {
  label: string;
  on: boolean;
  tier: string;
  destination: string | null;
}) {
  return (
    <tr className="border-t border-border/60">
      <th scope="row" className="py-2 pr-4 text-left font-semibold">
        {props.label}
      </th>
      <td className="py-2 pr-4">{props.on ? "on" : "off"}</td>
      <td className="py-2 pr-4">{props.tier}</td>
      <td className="py-2 font-mono text-sm break-all">
        {props.on && props.destination ? props.destination : "nowhere"}
      </td>
    </tr>
  );
}

export function TelemetryPanelView({ status }: { status: TelemetryStatus }) {
  return (
    <div className="grid gap-4" data-testid="telemetry-panel">
      <p className="max-w-[70ch]" data-testid="telemetry-summary">
        {summary(status)}
      </p>
      <table className="w-full text-sm">
        <thead>
          <tr className="text-left text-xs tracking-widest text-muted-foreground uppercase">
            <th className="pb-2 pr-4 font-semibold">
              <span className="sr-only">Part</span>
            </th>
            <th className="pb-2 pr-4 font-semibold">State</th>
            <th className="pb-2 pr-4 font-semibold">Tier</th>
            <th className="pb-2 font-semibold">Destination</th>
          </tr>
        </thead>
        <tbody>
          <Row
            label="Server"
            on={status.enabled && status.serverExporting}
            tier={status.serverTier}
            destination={status.serverEndpoint}
          />
          <Row
            label="Browsers"
            on={status.enabled && status.browserEnabled}
            tier={status.browserTier}
            destination={status.browserEndpoint}
          />
        </tbody>
      </table>
      <p data-testid="telemetry-install-id">
        Install id: <code>{status.instanceId}</code>
      </p>
      <p className="max-w-[70ch] text-muted-foreground">
        {CHANGE_TEXT}{" "}
        <a
          className="underline"
          href={GUIDE_URL}
          rel="noreferrer"
          target="_blank"
        >
          What is sent
        </a>
      </p>
    </div>
  );
}

export function TelemetryPanel() {
  const { status, loading, error } = useTelemetryStatus();
  if (error) {
    return (
      <p className="text-destructive">
        Could not read the telemetry status: {error.message}
      </p>
    );
  }
  if (loading || !status) {
    return <p className="text-muted-foreground">Reading telemetry status…</p>;
  }
  return <TelemetryPanelView status={status} />;
}
