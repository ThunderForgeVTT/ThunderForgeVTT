import {
  ALLOWED_ATTRIBUTES,
  EVENT_NAMES,
  telemetry,
} from "@thunderforge/telemetry";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  REASONS,
  STEPS,
  recordStep,
  setStepSink,
  stepAttributes,
  type StepAttributes,
} from "../telemetry";

/** Spec 048 T088: `sheet_import.step` carries only the closed sets. */

afterEach(() => {
  setStepSink(undefined);
  vi.restoreAllMocks();
});

describe("sheet_import.step", () => {
  it("carries a reason on failed only", () => {
    for (const step of STEPS.filter((s) => s !== "failed")) {
      expect(stepAttributes(step, "SHEET_ENCRYPTED")).toEqual({ step });
    }
    expect(stepAttributes("failed", "SHEET_ENCRYPTED")).toEqual({
      step: "failed",
      reason: "encrypted",
    });
  });

  it("maps every refusal code into the closed set, and nothing else", () => {
    const codes = [
      "SHEET_ENCRYPTED",
      "SHEET_TOO_LARGE",
      "SHEET_TOO_MANY_PAGES",
      "SHEET_NOT_RECOGNISED",
      "SHEET_UNREADABLE",
      "PLAN_CHANGED",
    ];
    const reasons = codes.map((code) => stepAttributes("failed", code).reason);
    expect(reasons).toEqual([...REASONS]);
    expect(stepAttributes("failed", "FORBIDDEN")).toEqual({ step: "failed" });
    expect(stepAttributes("failed", "an arbitrary sentence")).toEqual({
      step: "failed",
    });
    expect(stepAttributes("failed")).toEqual({ step: "failed" });
  });

  it("hands the event to the sink by its name", () => {
    const seen: [string, StepAttributes][] = [];
    setStepSink((name, attributes) => seen.push([name, attributes]));
    recordStep("opened");
    recordStep("failed", "PLAN_CHANGED");
    expect(seen).toEqual([
      ["sheet_import.step", { step: "opened" }],
      ["sheet_import.step", { step: "failed", reason: "plan_changed" }],
    ]);
  });

  it("records through @thunderforge/telemetry by default", () => {
    const event = vi.spyOn(telemetry, "event").mockImplementation(() => {});
    recordStep("failed", "SHEET_TOO_LARGE");
    expect(event).toHaveBeenCalledWith("sheet_import.step", {
      step: "failed",
      reason: "too_large",
    });
  });

  it("is a name and attributes the package allows", () => {
    expect(EVENT_NAMES).toContain("sheet_import.step");
    expect(ALLOWED_ATTRIBUTES).toContain("step");
    expect(ALLOWED_ATTRIBUTES).toContain("reason");
  });
});
