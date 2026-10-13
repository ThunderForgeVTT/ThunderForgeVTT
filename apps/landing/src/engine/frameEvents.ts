// Spec 090 FR-018: what the engine frame tells the page when it holds back or
// fails. Its own module so `telemetry.ts`, loaded with the page, can listen
// without pulling the frame into the first download.

export const FRAME_FAILED = "thunderforge:engine-frame-failed";

/** Why the frame held the engine back, a closed set. */
export type HeldBack = "no_webgl2" | "save_data" | "low_memory" | "small_texture";

export interface FrameFailed {
  /** `probe` when held back; else the load stage, or `engine` for a crash. */
  stage: string;
  reason: HeldBack | "error";
}
