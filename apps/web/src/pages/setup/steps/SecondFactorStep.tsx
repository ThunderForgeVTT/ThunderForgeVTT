import type { FormEvent } from "react";
import { useReducer, useState } from "react";
import {
  beginTwoFactorEnrolment,
  confirmTwoFactorEnrolment,
} from "@/api/twoFactor";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { TwoFactorEnrolmentSteps } from "@/components/security/TwoFactorEnrolmentSteps";
import {
  initialTwoFactorEnrolmentState,
  isConfirmableCode,
  twoFactorEnrolmentReducer,
} from "@/services/twoFactorEnrolment";

/**
 * Spec 040 FR-002a — setup does not finish without a confirmed second factor.
 *
 * # This reuses spec 041's flow rather than drawing a second one
 *
 * `TwoFactorEnrolmentSteps` is already shared between the account-settings
 * entrance and the sign-in-that-requires-enrolment entrance, and its header
 * states the rule it exists for (041 FR-001a: *one* flow, identical steps and
 * wording, differing only in where the person arrives afterwards). First-run
 * setup is the third entrance that sentence names, so it is wired to the same
 * component and the same reducer, and it differs only in what happens after
 * the recovery codes are acknowledged — here, the review step.
 *
 * **It needed no new prop.** The props it already exposes (`onAbandon`,
 * `acknowledgeLabel`, `acknowledgedNotice`, `isBusy`) covered every difference
 * this entrance has. `@/api/twoFactor`'s `TwoFactorEnrolmentCredentials`
 * likewise already names first-run setup as a user of its
 * `{ username, password }` shape, so no API change was needed either.
 *
 * # The password, and the OAuth hole
 *
 * `setup/start` and `setup/confirm` authorise with the account password, which
 * the account step hands over for the length of one pass. An administrator
 * bootstrapped through OAuth has no password to hand over and no login
 * challenge either — there is no third authorisation shape, and inventing one
 * here would be designing spec 041's contract from inside 040, which
 * `contracts/setup.md` rule 4 explicitly forbids.
 *
 * What this step does about that is say so: it explains that the second factor
 * is enrolled at the first sign-in instead (041 FR-019's entrance), rather
 * than pretending to offer a flow it cannot start. Tried and rejected:
 * re-prompting for a password on this step, which for an OAuth account is a
 * password that does not exist.
 */
export interface SecondFactorStepProps {
  /** The account the account step created, when it created one locally. */
  credentials: { username: string; password: string } | null;
  /** Already confirmed — a resumed pass, or an enrolment done elsewhere. */
  confirmed: boolean;
  onConfirmed: () => void;
}

export function SecondFactorStep({
  credentials,
  confirmed,
  onConfirmed,
}: SecondFactorStepProps) {
  const [state, dispatch] = useReducer(
    twoFactorEnrolmentReducer,
    initialTwoFactorEnrolmentState,
  );
  const [code, setCode] = useState("");

  if (confirmed) {
    return (
      <div data-testid="setup-second-factor-confirmed" className="grid gap-3">
        <StatusBadge variant="success">
          Two-factor authentication is on for this administrator.
        </StatusBadge>
        <p className="text-sm text-muted-foreground">
          Setup can finish. Every administrator on this instance holds a second
          factor, and that is checked again when you complete setup.
        </p>
      </div>
    );
  }

  if (!credentials) {
    return (
      <div data-testid="setup-second-factor-deferred" className="grid gap-3">
        <StatusBadge variant="warning">
          This administrator has no password for this instance to check.
        </StatusBadge>
        <p className="text-sm text-muted-foreground">
          Enrolment here is authorised with the account password, and an
          administrator created through an OAuth provider does not have one.
          Sign in once and the instance will take you through enrolling a second
          factor then. Setup cannot be completed until that has happened.
        </p>
      </div>
    );
  }

  const onBegin = async () => {
    dispatch({ type: "start" });
    setCode("");

    try {
      const enrolment = await beginTwoFactorEnrolment(credentials);
      dispatch({ type: "started", enrolment });
    } catch (error) {
      dispatch({
        type: "startFailed",
        message:
          error instanceof Error
            ? error.message
            : "Could not start two-factor setup.",
      });
    }
  };

  const onConfirm = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    if (state.step !== "provisioning") {
      return;
    }

    if (!isConfirmableCode(code)) {
      dispatch({
        type: "codeRejected",
        message: "Enter the six digits your authenticator is showing.",
      });
      return;
    }

    dispatch({ type: "submitCode" });

    try {
      const confirmation = await confirmTwoFactorEnrolment(credentials, code);
      setCode("");
      dispatch({ type: "confirmed", confirmation });
    } catch (error) {
      // Not a restart: the pending secret is untouched on the server, so the
      // one on screen is still the right one (041 FR-001c).
      dispatch({
        type: "codeRejected",
        message:
          error instanceof Error
            ? error.message
            : "That code was not accepted. Try the next one.",
      });
    }
  };

  return (
    <div className="grid gap-5">
      <p className="text-sm text-muted-foreground">
        The account that owns this instance holds a second factor. Setup will
        not complete without one, and there is no administrator on this instance
        that does not have one.
      </p>

      {state.step === "idle" || state.step === "starting" ? (
        <div className="grid gap-3">
          {state.step === "idle" && state.error ? (
            <div data-testid="setup-second-factor-error">
              <StatusBadge variant="danger">{state.error}</StatusBadge>
            </div>
          ) : null}
          <div>
            <Button
              data-testid="setup-second-factor-start"
              type="button"
              variant="primary"
              icon="shield"
              disabled={state.step === "starting"}
              onClick={() => void onBegin()}
            >
              {state.step === "starting"
                ? "Preparing..."
                : "Set up two-factor authentication"}
            </Button>
          </div>
        </div>
      ) : (
        <TwoFactorEnrolmentSteps
          state={state}
          code={code}
          onCodeChange={setCode}
          onConfirm={(event) => void onConfirm(event)}
          onAbandon={() => {
            setCode("");
            dispatch({ type: "abandon" });
          }}
          abandonLabel="Start over"
          onAcknowledge={() => {
            dispatch({ type: "acknowledgeRecoveryCodes" });
            onConfirmed();
          }}
          acknowledgeLabel="I have saved these codes"
          acknowledgedNotice="Two-factor authentication is on. Setup can finish."
        />
      )}
    </div>
  );
}
