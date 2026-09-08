import type { FormEvent } from "react";
import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import {
  beginTwoFactorEnrolment,
  confirmTwoFactorEnrolment,
  readTwoFactorStatus,
} from "@/api/twoFactor";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { TwoFactorEnrolmentSteps } from "@/components/security/TwoFactorEnrolmentSteps";
import { useAuth } from "@/hooks/useAuth";
import {
  initialTwoFactorEnrolmentState,
  isConfirmableCode,
  twoFactorEnrolmentReducer,
} from "@/services/twoFactorEnrolment";
import type { TwoFactorStatus } from "@/types/twoFactor";

/**
 * Spec 041 US1 (FR-001 … FR-005, FR-001a … FR-001c): adding a second factor
 * from your own account settings.
 *
 * Before this existed, enrolment was reachable only by posting JSON at
 * `/api/authentication/2fa/setup/start` by hand — which is why
 * `apps/web/e2e/two-factor.spec.ts` enrols over `page.request` and says so in
 * its header comment. The login challenge was the only two-factor UI in the
 * app, and it is the *second* half of a thing whose first half did not exist.
 *
 * # The steps, and why they are these steps
 *
 * 1. **Confirm your password.** Not a design choice so much as the server's
 *    current contract — see `TwoFactorEnrolmentCredentials` in
 *    `@/api/twoFactor` for what is supposed to replace it. It also reads as a
 *    re-authentication, which is normal on a security screen.
 * 2. **Scan or type the secret.** Both, always, never one or the other
 *    (FR-002): a desktop authenticator or a password manager has no camera,
 *    and a phone has no keyboard anybody enjoys. See the QR note below.
 * 3. **Enter a current code.** Nothing about the account has changed yet, and
 *    the panel says so in as many words (FR-003, FR-004). A wrong code
 *    re-renders this same step with the same secret — the enrolment is not
 *    discarded and nothing needs re-scanning (FR-001c).
 * 4. **Save the recovery codes.** Shown once, with a plain statement of what
 *    they are for. Dismissing them is deliberate and irreversible, because
 *    nothing on the server can produce them again (FR-009).
 *
 * # The QR code
 *
 * `research.md` § R10 decided the server encodes a module matrix and the
 * client draws rects — no QR dependency, no `dangerouslySetInnerHTML`. The
 * client half of that is `TwoFactorQrCode`, drawn by the shared
 * `TwoFactorEnrolmentSteps` this panel renders (FR-001a: one flow, three
 * entrances — this is the account-settings one, and the sign-in entrance is
 * `LoginView`). The
 * server half does not exist yet: `two_factor_setup_start` returns
 * `{status, message, otpauth_url}` and no `qr` field. Contract rule 6 already
 * says what to do about that — "a failure to build the QR is not a failure to
 * enrol" — so when `qr` is absent this panel shows the `otpauth://` URI as
 * selectable text next to the typeable secret, and enrolment completes
 * normally.
 *
 * TODO(spec-041, research.md § R10): remove the fallback prose below once
 * `two_factor_setup_start` returns `qr: { size, modules }`. No other change is
 * needed here.
 */
export function TwoFactorEnrolmentPanel() {
  const { user } = useAuth();
  const [state, dispatch] = useReducer(
    twoFactorEnrolmentReducer,
    initialTwoFactorEnrolmentState,
  );
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [status, setStatus] = useState<TwoFactorStatus | null>(null);
  const [statusLoaded, setStatusLoaded] = useState(false);
  const codeInputRef = useRef<HTMLInputElement | null>(null);

  /**
   * The password is held for the length of one enrolment because
   * `setup/confirm` needs it too, and asking for it twice would make a
   * mistyped code cost a password re-entry — the thing FR-001c exists to
   * prevent. It is dropped the moment the flow ends, in either direction.
   */
  const forgetPassword = useCallback(() => setPassword(""), []);

  useEffect(() => {
    let active = true;

    void readTwoFactorStatus().then((value) => {
      if (active) {
        setStatus(value);
        setStatusLoaded(true);
      }
    });

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (state.step === "provisioning" && !state.isConfirming) {
      const timer = window.setTimeout(() => codeInputRef.current?.focus(), 20);
      return () => window.clearTimeout(timer);
    }

    return undefined;
  }, [state]);

  const username = user?.username ?? "";

  const onBegin = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    if (!username || !password) {
      dispatch({
        type: "startFailed",
        message: "Enter your account password to continue.",
      });
      return;
    }

    dispatch({ type: "start" });
    setCode("");

    try {
      const enrolment = await beginTwoFactorEnrolment({ username, password });
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
      const confirmation = await confirmTwoFactorEnrolment(
        { username, password },
        code,
      );
      forgetPassword();
      setCode("");
      dispatch({ type: "confirmed", confirmation });
      // What the server just told us is now the truth about the account, and
      // it is more current than whatever the status read said (or failed to
      // say) when this panel mounted.
      setStatus({
        enabled: true,
        confirmedAt: confirmation.confirmedAt,
        recoveryCodesRemaining: confirmation.recoveryCodes.length,
        recoveryCodesLow: false,
        enrolmentPending: false,
      });
    } catch (error) {
      // Deliberately *not* a restart. The pending secret is untouched on the
      // server, so the same one on screen is still the right one (FR-001c).
      dispatch({
        type: "codeRejected",
        message:
          error instanceof Error
            ? error.message
            : "That code was not accepted. Try the next one.",
      });
    }
  };

  const onAbandon = () => {
    forgetPassword();
    setCode("");
    dispatch({ type: "abandon" });
  };

  return (
    <Card
      surface="parchment"
      className="grid gap-5 p-6"
      data-testid="two-factor-enrolment-panel"
    >
      <header className="grid gap-1">
        <h2 className="text-lg font-semibold">Two-factor authentication</h2>
        <p className="text-sm text-muted-foreground">
          A code from an authenticator app, asked for after your password when
          you sign in.
        </p>
      </header>

      <TwoFactorStatusSummary
        status={status}
        loaded={statusLoaded}
        justConfirmedAt={
          state.step === "confirmed" || state.step === "acknowledged"
            ? state.confirmedAt
            : null
        }
      />

      {state.step === "idle" || state.step === "starting" ? (
        <form onSubmit={onBegin} className="grid gap-4">
          <p className="text-sm text-muted-foreground">
            You will confirm your password, add ThunderForge to an authenticator
            app, and then prove it works with one code. Nothing about your
            account changes until that code is accepted.
          </p>
          <Field
            label="Account password"
            htmlFor="two-factor-password"
            accent="Required"
            error={
              state.step === "idle" ? (state.error ?? undefined) : undefined
            }
            hint="Confirms it is you before a new second factor is issued."
          >
            <Input
              data-testid="two-factor-password"
              id="two-factor-password"
              name="password"
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              disabled={state.step === "starting"}
            />
          </Field>
          <div>
            <Button
              data-testid="two-factor-begin"
              type="submit"
              variant="primary"
              icon="shield"
              disabled={state.step === "starting"}
            >
              {state.step === "starting"
                ? "Starting..."
                : status?.enabled
                  ? "Replace my second factor"
                  : "Set up two-factor authentication"}
            </Button>
          </div>
          {status?.enabled ? (
            <p className="text-sm text-muted-foreground">
              You already have a second factor. The one you are using now keeps
              working until you confirm the new one.
            </p>
          ) : null}
        </form>
      ) : null}

      <TwoFactorEnrolmentSteps
        state={state}
        code={code}
        onCodeChange={setCode}
        onConfirm={onConfirm}
        onAbandon={state.step === "provisioning" ? onAbandon : undefined}
        onAcknowledge={() => dispatch({ type: "acknowledgeRecoveryCodes" })}
        codeInputRef={codeInputRef}
      />
    </Card>
  );
}

/**
 * FR-005 — whether a second factor is in force, and when it was confirmed.
 *
 * This can genuinely fail to be answerable. `contracts/enrolment.md` specifies
 * `GET /api/authentication/2fa/status`, and `src/server/src/auth/mod.rs` does
 * not route it; no GraphQL field carries an account's own two-factor state
 * either. So `readTwoFactorStatus` resolves to `null` and this says it cannot
 * tell, which is the honest thing to say and better than a badge that means
 * nothing. A confirmation completed in this session is reported regardless,
 * because that one we witnessed.
 */
function TwoFactorStatusSummary({
  status,
  loaded,
  justConfirmedAt,
}: {
  status: TwoFactorStatus | null;
  loaded: boolean;
  justConfirmedAt: string | null;
}) {
  if (!loaded) {
    return (
      <StatusBadge variant="info">
        Checking your account&apos;s state…
      </StatusBadge>
    );
  }

  if (!status) {
    return (
      <div className="grid gap-1">
        <StatusBadge variant="warning">
          This instance cannot report whether two-factor is already on.
        </StatusBadge>
        {justConfirmedAt ? (
          <p className="text-sm text-muted-foreground">
            You confirmed a second factor at{" "}
            {new Date(justConfirmedAt).toLocaleString()}.
          </p>
        ) : null}
      </div>
    );
  }

  return (
    <div className="grid gap-1">
      <StatusBadge variant={status.enabled ? "success" : "info"}>
        {status.enabled
          ? "Two-factor authentication is on for your account."
          : "Two-factor authentication is off for your account."}
      </StatusBadge>
      {status.enabled ? (
        <p className="text-sm text-muted-foreground">
          Confirmed{" "}
          {status.confirmedAt
            ? new Date(status.confirmedAt).toLocaleString()
            : "at an unrecorded time"}
          .
          {status.recoveryCodesRemaining === null
            ? ""
            : ` ${status.recoveryCodesRemaining} recovery ${
                status.recoveryCodesRemaining === 1 ? "code" : "codes"
              } left.`}
        </p>
      ) : null}
    </div>
  );
}
