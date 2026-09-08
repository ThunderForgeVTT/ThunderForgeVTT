import { describe, expect, it } from "vitest";

import {
  initialTwoFactorEnrolmentState,
  isConfirmableCode,
  isEnrolmentResumable,
  twoFactorEnrolmentReducer,
  type TwoFactorEnrolmentAction,
  type TwoFactorEnrolmentState,
} from "../twoFactorEnrolment";
import type { PendingTwoFactorEnrolment } from "@/types/twoFactor";

/**
 * Spec 041 US1 — the two properties of the enrolment flow that are not about
 * how it looks.
 *
 * FR-001c ("a mistyped code must not discard the enrolment in progress or
 * require re-scanning") and FR-009 ("the codes are returned once") are both
 * claims about which states follow which, and both are the kind of thing that
 * a plausible-looking refactor breaks without failing anything else. So they
 * are asserted against the reducer directly rather than through a rendered
 * panel — which this app could not do anyway: there is no jsdom and no
 * testing-library here (see `vite.config.ts`).
 */

const ENROLMENT: PendingTwoFactorEnrolment = {
  otpauthUrl:
    "otpauth://totp/ThunderForge:wizard?secret=GEZDGNBVGY3TQOJQ&issuer=ThunderForge",
  secret: "GEZD GNBV GY3T QOJQ",
  qr: null,
};

function run(
  actions: readonly TwoFactorEnrolmentAction[],
  from: TwoFactorEnrolmentState = initialTwoFactorEnrolmentState,
): TwoFactorEnrolmentState {
  return actions.reduce(twoFactorEnrolmentReducer, from);
}

const provisioning = () =>
  run([{ type: "start" }, { type: "started", enrolment: ENROLMENT }]);

describe("two-factor enrolment: a wrong code (FR-001c)", () => {
  it("leaves the enrolment in progress, on the same secret", () => {
    const rejected = run(
      [
        { type: "submitCode" },
        { type: "codeRejected", message: "Invalid 2FA code" },
      ],
      provisioning(),
    );

    expect(rejected.step).toBe("provisioning");
    expect(isEnrolmentResumable(rejected)).toBe(true);
    if (!isEnrolmentResumable(rejected)) {
      throw new Error("unreachable");
    }

    // The same object, not merely an equal one: whatever is on screen is what
    // the server's pending row still holds, so nothing needs re-scanning.
    expect(rejected.enrolment).toBe(ENROLMENT);
    expect(rejected.isConfirming).toBe(false);
    expect(rejected.error).toBe("Invalid 2FA code");
  });

  it("survives being got wrong repeatedly and then accepts the right code", () => {
    const afterThreeMisses = run(
      [
        { type: "submitCode" },
        { type: "codeRejected", message: "Invalid 2FA code" },
        { type: "submitCode" },
        { type: "codeRejected", message: "Invalid 2FA code" },
        { type: "submitCode" },
        { type: "codeRejected", message: "Invalid 2FA code" },
      ],
      provisioning(),
    );

    expect(isEnrolmentResumable(afterThreeMisses)).toBe(true);

    const confirmed = twoFactorEnrolmentReducer(afterThreeMisses, {
      type: "confirmed",
      confirmation: {
        signedIn: false,
        confirmedAt: "2026-09-07T12:04:11Z",
        recoveryCodes: ["4KJH-92MX-QW3T"],
        recoveryCodesNotice: "Keep these somewhere else.",
      },
    });

    expect(confirmed.step).toBe("confirmed");
  });

  it("never turns a rejection into a fresh start", () => {
    const rejected = run(
      [{ type: "codeRejected", message: "no" }],
      provisioning(),
    );

    expect(rejected.step).not.toBe("idle");
    expect(rejected.step).not.toBe("starting");
  });
});

describe("two-factor enrolment: recovery codes are shown once (FR-009)", () => {
  const CODES = ["4KJH-92MX-QW3T", "8PLM-31QA-ZX9V", "1TYU-77BN-CD4R"] as const;

  const confirmed = () =>
    twoFactorEnrolmentReducer(provisioning(), {
      type: "confirmed",
      confirmation: {
        signedIn: false,
        confirmedAt: "2026-09-07T12:04:11Z",
        recoveryCodes: CODES,
        recoveryCodesNotice: "Each works once.",
      },
    });

  it("carries the codes exactly once, in the confirmed state", () => {
    const state = confirmed();
    expect(state.step).toBe("confirmed");
    expect(state).toMatchObject({ recoveryCodes: CODES });
  });

  it("drops them for good once they have been acknowledged", () => {
    const acknowledged = twoFactorEnrolmentReducer(confirmed(), {
      type: "acknowledgeRecoveryCodes",
    });

    expect(acknowledged.step).toBe("acknowledged");
    expect(JSON.stringify(acknowledged)).not.toContain("4KJH");
    // What FR-005 does keep: that it is on, and when it was confirmed.
    expect(acknowledged).toMatchObject({
      confirmedAt: "2026-09-07T12:04:11Z",
    });
  });

  it("cannot be talked back into showing them", () => {
    const acknowledged = twoFactorEnrolmentReducer(confirmed(), {
      type: "acknowledgeRecoveryCodes",
    });

    // Every action the flow has, replayed against the acknowledged state.
    // None of them may produce plaintext again — the only action that can is
    // `confirmed`, and that needs a fresh response from a server that has no
    // route capable of returning these codes a second time.
    const replayed = run(
      [
        { type: "start" },
        { type: "submitCode" },
        { type: "codeRejected", message: "no" },
        { type: "acknowledgeRecoveryCodes" },
        { type: "abandon" },
        { type: "startFailed", message: "no" },
      ],
      acknowledged,
    );

    expect(JSON.stringify(replayed)).not.toContain("4KJH");
  });

  it("refuses to start a new enrolment over an unacknowledged set of codes", () => {
    const state = twoFactorEnrolmentReducer(confirmed(), { type: "start" });
    expect(state.step).toBe("confirmed");
  });
});

describe("two-factor enrolment: abandoning (FR-004)", () => {
  it("returns to exactly the state it began in", () => {
    const abandoned = run([{ type: "abandon" }], provisioning());
    expect(abandoned).toEqual(initialTwoFactorEnrolmentState);
  });

  it("keeps no trace of the pending secret", () => {
    const abandoned = run(
      [
        { type: "submitCode" },
        { type: "codeRejected", message: "Invalid 2FA code" },
        { type: "abandon" },
      ],
      provisioning(),
    );

    expect(JSON.stringify(abandoned)).not.toContain("GEZD");
  });

  it("does nothing once the factor is actually on", () => {
    const acknowledged = run(
      [
        {
          type: "confirmed",
          confirmation: {
            signedIn: false,
            confirmedAt: "2026-09-07T12:04:11Z",
            recoveryCodes: [],
            recoveryCodesNotice: "",
          },
        },
        { type: "acknowledgeRecoveryCodes" },
        { type: "abandon" },
      ],
      provisioning(),
    );

    expect(acknowledged.step).toBe("acknowledged");
  });
});

describe("two-factor enrolment: restarting", () => {
  it("replaces the pending enrolment and clears the previous complaint", () => {
    const second: PendingTwoFactorEnrolment = { ...ENROLMENT, secret: "AAAA" };
    const restarted = run(
      [
        { type: "codeRejected", message: "Invalid 2FA code" },
        { type: "start" },
        { type: "started", enrolment: second },
      ],
      provisioning(),
    );

    expect(restarted).toMatchObject({
      step: "provisioning",
      enrolment: second,
      error: null,
    });
  });
});

describe("isConfirmableCode", () => {
  it("accepts six digits, with or without the spaces a manager pastes", () => {
    expect(isConfirmableCode("492013")).toBe(true);
    expect(isConfirmableCode(" 492 013 ")).toBe(true);
  });

  it("rejects anything else", () => {
    expect(isConfirmableCode("49201")).toBe(false);
    expect(isConfirmableCode("4920134")).toBe(false);
    expect(isConfirmableCode("49201a")).toBe(false);
    expect(isConfirmableCode("")).toBe(false);
  });
});
