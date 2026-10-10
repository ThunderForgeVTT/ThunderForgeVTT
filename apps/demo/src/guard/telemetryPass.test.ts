/**
 * Spec 086 FR-021: the guard passes a telemetry post to the configured
 * origin, and nothing else that leaves the page.
 */
import { afterEach, describe, expect, it } from "vitest";
import { allowTelemetryTo, isTelemetryPost } from "./telemetryPass";

const on = (endpoint: string) => ({ enabled: true, endpoint });
const post = (url: string) => isTelemetryPost("POST", new URL(url));

afterEach(() => allowTelemetryTo(null));

describe("isTelemetryPost", () => {
  it("passes nothing until the config has named an origin", () => {
    expect(post("https://telemetry.thunderforge.dev/v1/logs")).toBe(false);
  });

  it("passes a POST to the origin's logs and traces", () => {
    allowTelemetryTo(on("https://telemetry.thunderforge.dev"));
    expect(post("https://telemetry.thunderforge.dev/v1/logs")).toBe(true);
    expect(post("https://telemetry.thunderforge.dev/v1/traces")).toBe(true);
  });

  it("refuses another method, another path, a query, another origin", () => {
    allowTelemetryTo(on("https://telemetry.thunderforge.dev"));
    const logs = new URL("https://telemetry.thunderforge.dev/v1/logs");
    expect(isTelemetryPost("GET", logs)).toBe(false);
    expect(isTelemetryPost("PUT", logs)).toBe(false);
    expect(post("https://telemetry.thunderforge.dev/v1/metrics")).toBe(false);
    expect(post("https://telemetry.thunderforge.dev/")).toBe(false);
    expect(post("https://telemetry.thunderforge.dev/v1/logs?x=1")).toBe(false);
    expect(post("http://telemetry.thunderforge.dev/v1/logs")).toBe(false);
    expect(post("https://evil.example/v1/logs")).toBe(false);
    expect(
      post("https://telemetry.thunderforge.dev.evil.example/v1/logs"),
    ).toBe(false);
  });

  it("keeps an endpoint's path prefix, and ignores a trailing slash", () => {
    allowTelemetryTo(on("https://otel.example.org/otlp/"));
    expect(post("https://otel.example.org/otlp/v1/logs")).toBe(true);
    expect(post("https://otel.example.org/v1/logs")).toBe(false);
  });

  it("closes again when the config is off or not a URL", () => {
    allowTelemetryTo(on("https://telemetry.thunderforge.dev"));
    allowTelemetryTo({ enabled: false });
    expect(post("https://telemetry.thunderforge.dev/v1/logs")).toBe(false);
    allowTelemetryTo(on("not a url"));
    expect(post("https://telemetry.thunderforge.dev/v1/logs")).toBe(false);
    allowTelemetryTo(on("ftp://telemetry.thunderforge.dev"));
    expect(post("ftp://telemetry.thunderforge.dev/v1/logs")).toBe(false);
  });
});
