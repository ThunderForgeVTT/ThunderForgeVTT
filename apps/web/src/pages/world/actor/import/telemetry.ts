/**
 * Spec 048 research R16: the browser event `sheet_import.step`, against
 * spec 086's events table (`contracts/browser-events.md`).
 *
 * `packages/telemetry` is not on this branch yet, so `recordStep` hands the
 * event to a sink that does nothing. When the branches meet, the sink is
 * the package's `event("sheet_import.step", attributes)`, the name joins
 * its `EventName` type and the attributes its `ALLOWED_ATTRIBUTES`. The
 * call sites stay where they are.
 */

export const STEPS = [
  "opened",
  "read",
  "reviewed",
  "applied",
  "declined",
  "failed",
] as const;
export type SheetImportStep = (typeof STEPS)[number];

export const REASONS = [
  "encrypted",
  "too_large",
  "too_many_pages",
  "unrecognised",
  "unreadable",
  "plan_changed",
] as const;
export type SheetImportFailure = (typeof REASONS)[number];

export interface StepAttributes {
  step: SheetImportStep;
  reason?: SheetImportFailure;
}

const BY_CODE: Record<string, SheetImportFailure> = {
  SHEET_ENCRYPTED: "encrypted",
  SHEET_TOO_LARGE: "too_large",
  SHEET_TOO_MANY_PAGES: "too_many_pages",
  SHEET_NOT_RECOGNISED: "unrecognised",
  SHEET_UNREADABLE: "unreadable",
  PLAN_CHANGED: "plan_changed",
};

/**
 * The event's attributes. `reason` rides on `failed` only, and only when
 * the refusal's code is one of the closed set: a refusal for any other
 * cause (a permission, the flag) is a failure without a reason.
 */
export function stepAttributes(
  step: SheetImportStep,
  code?: string,
): StepAttributes {
  if (step !== "failed") return { step };
  const reason = code === undefined ? undefined : BY_CODE[code];
  return reason ? { step, reason } : { step };
}

type Sink = (name: "sheet_import.step", attributes: StepAttributes) => void;

let sink: Sink = () => {};

/** For the tests, and for 086's package once it is here. */
export function setStepSink(next: Sink): void {
  sink = next;
}

export function recordStep(step: SheetImportStep, code?: string): void {
  sink("sheet_import.step", stepAttributes(step, code));
}
