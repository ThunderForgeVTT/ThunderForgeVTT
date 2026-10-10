/**
 * Spec 086 FR-024: the notice's telemetry line follows the served config.
 */
import { describe, expect, it } from "vitest";
import { createNoticeStore, telemetryLine } from "./telemetryNotice";

describe("the notice's telemetry line", () => {
  it("names ThunderForge on the anonymous tier, linking to What we measure", () => {
    expect(
      telemetryLine({
        enabled: true,
        endpoint: "https://t.example",
        tier: "anonymous",
      }),
    ).toEqual({
      text: "Anonymous usage counts go to ThunderForge; what you type does not.",
      href: "/#telemetry",
    });
  });

  it("names the server's operator on the operator tier", () => {
    expect(
      telemetryLine({
        enabled: true,
        endpoint: "https://t.example",
        tier: "operator",
      }),
    ).toEqual({
      text: "Anonymous usage counts go to this server's operator; what you type does not.",
      href: "/#telemetry",
    });
  });

  it("says nothing when telemetry is off", () => {
    expect(telemetryLine(null)).toBeNull();
  });

  it("is told the config once it is read", () => {
    const store = createNoticeStore();
    const seen: unknown[] = [];
    const stop = store.subscribe(() => seen.push(store.get()));
    expect(store.get()).toBeNull();
    store.set({
      enabled: true,
      endpoint: "https://t.example",
      tier: "anonymous",
    });
    expect(seen).toHaveLength(1);
    expect(store.get()?.text).toContain("ThunderForge");
    stop();
    store.set(null);
    expect(seen).toHaveLength(1);
  });
});
