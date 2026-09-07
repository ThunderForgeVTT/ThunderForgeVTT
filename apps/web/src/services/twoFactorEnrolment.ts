import type {
  PendingTwoFactorEnrolment,
  TwoFactorConfirmation,
} from "@/types/twoFactor";

/**
 * Spec 041 US1 — the enrolment flow as a state machine, separate from the
 * screen that draws it.
 *
 * Two of this feature's requirements are properties of the *transitions*
 * rather than of any particular pixel, and both are the kind of thing a
 * refactor silently breaks:
 *
 *   - **FR-001c** — a mistyped code must not discard the enrolment in
 *     progress or require re-scanning. Here that is: `codeRejected` returns a
 *     state whose `enrolment` is the *same object* it already had. There is
 *     no path from a rejection back to `idle`.
 *   - **FR-009** — the recovery codes are shown once. Here that is:
 *     `confirmed` is the only state that holds them, `acknowledgeRecoveryCodes`
 *     leaves a state that does not, and no action produces codes from
 *     anything except a fresh `TwoFactorConfirmation` handed in by the
 *     server. Nothing can put them back, because nothing can fetch them
 *     again.
 *
 * Keeping this pure is also what makes the flow testable at all: this app has
 * neither jsdom nor testing-library (see `vite.config.ts` and
 * `packSurfaceBoundary.test.tsx`), so a test that drove the panel by clicking
 * is not available. A test that drives the transitions is.
 */

export type TwoFactorEnrolmentState =
  /** Nothing started. Reached again only by abandoning before confirmation. */
  | { step: "idle"; error: string | null }
  /** `setup/start` is in flight. */
  | { step: "starting" }
  /**
   * A pending secret exists on the server and is on screen. The account is
   * untouched until a code proves the secret (FR-003/FR-013).
   */
  | {
      step: "provisioning";
      enrolment: PendingTwoFactorEnrolment;
      isConfirming: boolean;
      /** Why the last code was refused, if one was. */
      error: string | null;
    }
  /** Confirmed. The only state that carries recovery-code plaintext. */
  | {
      step: "confirmed";
      confirmedAt: string | null;
      recoveryCodes: readonly string[];
      recoveryCodesNotice: string;
    }
  /** The person said they had saved the codes. The codes are gone. */
  | { step: "acknowledged"; confirmedAt: string | null };

export type TwoFactorEnrolmentAction =
  | { type: "start" }
  | { type: "started"; enrolment: PendingTwoFactorEnrolment }
  | { type: "startFailed"; message: string }
  | { type: "submitCode" }
  | { type: "codeRejected"; message: string }
  | { type: "confirmed"; confirmation: TwoFactorConfirmation }
  | { type: "acknowledgeRecoveryCodes" }
  | { type: "abandon" };

export const initialTwoFactorEnrolmentState: TwoFactorEnrolmentState = {
  step: "idle",
  error: null,
};

export function twoFactorEnrolmentReducer(
  state: TwoFactorEnrolmentState,
  action: TwoFactorEnrolmentAction,
): TwoFactorEnrolmentState {
  switch (action.type) {
    case "start":
      // Restarting from `provisioning` is allowed and is what "start over"
      // means: the server replaces the pending row and nothing else.
      return state.step === "confirmed" || state.step === "acknowledged"
        ? state
        : { step: "starting" };

    case "started":
      return {
        step: "provisioning",
        enrolment: action.enrolment,
        isConfirming: false,
        error: null,
      };

    case "startFailed":
      return { step: "idle", error: action.message };

    case "submitCode":
      return state.step === "provisioning"
        ? { ...state, isConfirming: true, error: null }
        : state;

    case "codeRejected":
      // FR-001c. The enrolment object is carried across unchanged — same
      // secret, same URI, same QR — so the screen does not re-render a new
      // code to scan and the server's pending row still matches what is
      // shown. Returning to `idle` here would be the bug.
      return state.step === "provisioning"
        ? { ...state, isConfirming: false, error: action.message }
        : state;

    case "confirmed":
      return {
        step: "confirmed",
        confirmedAt: action.confirmation.confirmedAt,
        recoveryCodes: action.confirmation.recoveryCodes,
        recoveryCodesNotice: action.confirmation.recoveryCodesNotice,
      };

    case "acknowledgeRecoveryCodes":
      // The codes leave the client here and there is no way back. `confirmedAt`
      // survives because FR-005 asks for it; the codes do not, because FR-009
      // says this was their only appearance.
      return state.step === "confirmed"
        ? { step: "acknowledged", confirmedAt: state.confirmedAt }
        : state;

    case "abandon":
      // FR-004. Abandoning is a client-side act only: the server was never
      // asked to change anything live, so there is nothing to undo and the
      // interface must not imply there is. After confirmation there is
      // nothing to abandon.
      return state.step === "confirmed" || state.step === "acknowledged"
        ? state
        : initialTwoFactorEnrolmentState;

    default:
      return state;
  }
}

/**
 * The six digits an authenticator shows. Whitespace is tolerated because
 * password managers paste it, and stripped before it reaches the server.
 */
export function isConfirmableCode(code: string): boolean {
  return /^\d{6}$/.test(code.replace(/\s+/g, ""));
}

/**
 * Whether this state still has a pending enrolment the person can go back to
 * after a wrong code — the readable form of FR-001c for the screen to use.
 */
export function isEnrolmentResumable(
  state: TwoFactorEnrolmentState,
): state is Extract<TwoFactorEnrolmentState, { step: "provisioning" }> {
  return state.step === "provisioning";
}
