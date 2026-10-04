import type { FormEvent } from "react";
import { useReducer, useState } from "react";
import {
  beginTwoFactorEnrolment,
  confirmTwoFactorEnrolment,
  requestSetupEnrolmentTicket,
  TwoFactorRequestError,
  type TwoFactorEnrolmentCredentials,
} from "@/api/twoFactor";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";
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
 * this entrance has, and `@/api/twoFactor`'s `TwoFactorEnrolmentCredentials`
 * already had both shapes this step presents.
 *
 * # The password, and the administrator who has none
 *
 * `setup/start` and `setup/confirm` authorise with the account password, which
 * the account step hands over for the length of one pass, or with an enrolment
 * ticket. An administrator bootstrapped through a sign-in provider has no
 * password to hand over, and for a long time had no ticket either: this step
 * told them to "sign in once" to enrol, and `/login` redirects to `/setup`
 * while setup is open. Setup needed the factor and the factor needed setup to
 * be over.
 *
 * So when there is no password in hand, this step asks the server for a
 * ticket (`requestSetupEnrolmentTicket`). The server gives one for the
 * bootstrap code and this browser's session together, and only while setup is
 * open — it is the ticket a sign-in mints, from a second place, and not a
 * third way to authorise enrolment. The ticket is kept for the pass: `start`
 * does not spend it, `confirm` does.
 *
 * The password form stays as the other way through. A local administrator who
 * reopened setup in a browser that holds no session — a different machine, or
 * cookies cleared — is refused a ticket, and still has the password they
 * chose.
 */
export interface SecondFactorStepProps {
  /** The account the account step created, when it created one locally. */
  credentials: { username: string; password: string } | null;
  /** The bootstrap code, which a ticket is asked for with. */
  adminCode: string;
  /** Already confirmed — a resumed pass, or an enrolment done elsewhere. */
  confirmed: boolean;
  onConfirmed: () => void;
}

export function SecondFactorStep({
  credentials,
  adminCode,
  confirmed,
  onConfirmed,
}: SecondFactorStepProps) {
  const [state, dispatch] = useReducer(
    twoFactorEnrolmentReducer,
    initialTwoFactorEnrolmentState,
  );
  const [code, setCode] = useState("");
  const [reentered, setReentered] = useState({ username: "", password: "" });
  // What `start` was authorised with, kept so `confirm` presents the same
  // proof. A ticket is minted once per pass and spent by the confirmation.
  const [authorisedWith, setAuthorisedWith] =
    useState<TwoFactorEnrolmentCredentials | null>(null);

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

  // The account step hands its credentials over in memory, and a reload or a
  // resumed setup loses them. A password typed again here is used if there is
  // one; otherwise the server is asked for a ticket, which is the only way
  // through for an administrator who never had a password.
  const withPassword =
    credentials ??
    (reentered.username.trim() && reentered.password
      ? { username: reentered.username.trim(), password: reentered.password }
      : null);

  const onBegin = async () => {
    dispatch({ type: "start" });
    setCode("");
    setAuthorisedWith(null);

    try {
      const using =
        withPassword ?? (await requestSetupEnrolmentTicket(adminCode.trim()));
      const enrolment = await beginTwoFactorEnrolment(using);
      setAuthorisedWith(using);
      dispatch({ type: "started", enrolment });
    } catch (error) {
      dispatch({
        type: "startFailed",
        message:
          // No session in this browser, so no ticket. Say what to do about
          // it rather than repeating the server's description of the cause.
          error instanceof TwoFactorRequestError &&
          error.status === "unauthenticated"
            ? "This browser is not signed in as the administrator. Enter the administrator's username and password below, or start setup again from the browser you created the administrator in."
            : error instanceof Error
              ? error.message
              : "Could not start two-factor setup.",
      });
    }
  };

  const onConfirm = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    const using = authorisedWith;
    if (state.step !== "provisioning" || !using) {
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
      const confirmation = await confirmTwoFactorEnrolment(using, code);
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
          {credentials ? null : (
            <div
              data-testid="setup-second-factor-reenter"
              className="grid max-w-sm gap-3"
            >
              <p className="text-sm text-muted-foreground">
                Setup was reopened. If this browser is still signed in as the
                administrator you created, you can go straight on. If it is not,
                confirm that administrator here: enrolment is then authorised
                with the account&rsquo;s password.
              </p>
              <label className="grid gap-1 text-sm font-medium">
                Administrator username
                <Input
                  data-testid="setup-second-factor-username"
                  autoComplete="username"
                  value={reentered.username}
                  onChange={(event) =>
                    setReentered({ ...reentered, username: event.target.value })
                  }
                />
              </label>
              <label className="grid gap-1 text-sm font-medium">
                Password
                <Input
                  data-testid="setup-second-factor-password"
                  type="password"
                  autoComplete="current-password"
                  value={reentered.password}
                  onChange={(event) =>
                    setReentered({ ...reentered, password: event.target.value })
                  }
                />
              </label>
              <p className="text-sm text-muted-foreground">
                An administrator created through a sign-in provider has no
                password here. Leave both fields empty and continue: this
                browser is already signed in as that administrator, and that
                together with the setup code is what authorises enrolment.
              </p>
            </div>
          )}
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
