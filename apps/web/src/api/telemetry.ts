import { postGraphQL } from "@/api/graphqlClient";

/**
 * Spec 086 FR-035.4: where this instance's telemetry goes, as the server
 * resolved it at startup. Administrators only. The tier is the wire name:
 * `full`, `anonymous` or `off`.
 */
export interface TelemetryStatus {
  enabled: boolean;
  serverExporting: boolean;
  serverTier: string;
  serverEndpoint: string | null;
  browserEnabled: boolean;
  browserTier: string;
  browserEndpoint: string | null;
  instanceId: string;
}

export async function fetchTelemetryStatus(): Promise<TelemetryStatus> {
  const data = await postGraphQL<{ telemetryStatus: TelemetryStatus }>(
    `query TelemetryStatus {
      telemetryStatus {
        enabled
        serverExporting
        serverTier
        serverEndpoint
        browserEnabled
        browserTier
        browserEndpoint
        instanceId
      }
    }`,
  );
  return data.telemetryStatus;
}
