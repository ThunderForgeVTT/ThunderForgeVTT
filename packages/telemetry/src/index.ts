/** The core's public surface (contracts/browser-events.md). No DOM, no OTLP. */

export * from "./allowList.ts";
export { parseConfig, readConfig, endpointOrigin } from "./config.ts";
export type { Tier, TelemetryConfig } from "./config.ts";
export { privacyOf, errorsOnly } from "./privacy.ts";
export type { Privacy } from "./privacy.ts";
export {
  errorAttributes,
  reduceStack,
  MESSAGE_CAP,
  STACK_CAP,
} from "./errors.ts";
export type { ErrorSource, Redactor } from "./errors.ts";
export { SESSION_KEY, memoryStorage } from "./session.ts";
export { deviceAttributes } from "./ua.ts";
export type { DeviceFacts } from "./ua.ts";
export {
  createTelemetry,
  noopTelemetry,
  MAX_EVENTS,
  MAX_ERRORS,
} from "./telemetry.ts";
export type {
  CreateTelemetryOptions,
  SpanRef,
  OpenSpan,
  Telemetry,
  TelemetrySink,
} from "./telemetry.ts";
export type { Batch, TelemetryRecord } from "./records.ts";
export { bootTelemetry, telemetry, sessionStore } from "./boot.ts";
