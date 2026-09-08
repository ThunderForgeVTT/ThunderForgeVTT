import type { FormEvent, RefObject } from "react";
import { Button } from "@/components/ui/button/Button";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { TwoFactorQrCode } from "@/components/security/TwoFactorQrCode";
import type { TwoFactorEnrolmentState } from "@/services/twoFactorEnrolment";

/**
 * Spec 041 FR-001a: the enrolment flow's steps, drawn once.
 *
 * "The enrolment flow MUST be one flow, reachable from account settings, from
 * first-run setup, and from a sign-in that requires enrolment — identical in
 * its steps and wording, differing only in where the person arrives
 * afterwards." Three entrances that each drew their own version of "scan this,
 * type a code, save these codes" would satisfy that sentence by accident at
 * best and would drift apart on the first wording change, so the steps live
 * here and the entrances are thin.
 *
 * What each entrance keeps for itself is exactly what FR-001a says differs:
 *
 *   - how it was authorised — a password on the settings screen, the login
 *     challenge at a sign-in that requires enrolment (see
 *     `enrolmentAuthorisationBody` in `@/api/twoFactor`);
 *   - what happens after the recovery codes are acknowledged — a settings page
 *     stays put, a sign-in goes where the person was going (FR-020).
 *
 * This component is presentational. The state machine is
 * `services/twoFactorEnrolment`, shared by both entrances, and every
 * requirement that lives in a transition (FR-001c's "a wrong code does not
 * discard the enrolment", FR-009's "the codes are shown once") is asserted
 * against the reducer rather than against this markup.
 */
export interface TwoFactorEnrolmentStepsProps {
  state: TwoFactorEnrolmentState;
  /** The six digits, as typed. */
  code: string;
  onCodeChange: (value: string) => void;
  onConfirm: (event: FormEvent<HTMLFormElement>) => void;
  /** Leaving before confirmation. Omitted where there is nowhere to leave to. */
  onAbandon?: () => void;
  abandonLabel?: string;
  /** "I have saved these codes" — the only way past the recovery codes. */
  onAcknowledge: () => void;
  acknowledgeLabel?: string;
  /** What the entrance says once the codes are gone. */
  acknowledgedNotice?: string;
  codeInputRef?: RefObject<HTMLInputElement | null>;
  /** Disables the confirm button while the entrance is doing something after it. */
  isBusy?: boolean;
}

export function TwoFactorEnrolmentSteps({
  state,
  code,
  onCodeChange,
  onConfirm,
  onAbandon,
  abandonLabel = "Cancel",
  onAcknowledge,
  acknowledgeLabel = "I have saved these codes",
  acknowledgedNotice = "Two-factor authentication is on. Your recovery codes have been hidden and cannot be shown again.",
  codeInputRef,
  isBusy = false,
}: TwoFactorEnrolmentStepsProps) {
  if (state.step === "provisioning") {
    return (
      <div className="grid gap-5">
        <div className="grid gap-4 sm:grid-cols-[auto_1fr] sm:items-start">
          {state.enrolment.qr ? (
            <TwoFactorQrCode matrix={state.enrolment.qr} />
          ) : null}
          <div className="grid gap-3">
            <div className="grid gap-1">
              <h3 className="text-sm font-semibold">
                {state.enrolment.qr
                  ? "Scan this, or type the key"
                  : "Add this key to your authenticator"}
              </h3>
              <p className="text-sm text-muted-foreground">
                {state.enrolment.qr
                  ? "Scan the code with your authenticator app, or type the key below if you are setting this up on the same device."
                  : "This instance does not draw a scannable code yet. Type the key below into your authenticator, or paste the setup link if your app or password manager accepts one."}
              </p>
            </div>

            <Field
              label="Setup key"
              htmlFor="two-factor-secret"
              hint="Spaces are only there to make it readable — type it with or without them."
            >
              <code
                data-testid="two-factor-setup-key"
                id="two-factor-secret"
                className="block rounded-md border border-border bg-muted px-3 py-2 font-mono text-sm break-all select-all"
              >
                {state.enrolment.secret || state.enrolment.otpauthUrl}
              </code>
            </Field>

            <Field
              label="Setup link"
              htmlFor="two-factor-otpauth"
              hint="Some desktop authenticators and password managers take this whole link."
            >
              <code
                id="two-factor-otpauth"
                className="block rounded-md border border-border bg-muted px-3 py-2 font-mono text-xs break-all select-all"
              >
                {state.enrolment.otpauthUrl}
              </code>
            </Field>
          </div>
        </div>

        <form onSubmit={onConfirm} className="grid gap-4">
          <Field
            label="Code from your authenticator"
            htmlFor="two-factor-code"
            accent="Required"
            error={state.error ?? undefined}
            hint="Six digits. If it is refused, wait for the next one and try again — you will not need to scan anything twice."
          >
            <Input
              data-testid="two-factor-code"
              ref={codeInputRef}
              id="two-factor-code"
              name="twoFactorCode"
              inputMode="numeric"
              autoComplete="one-time-code"
              maxLength={7}
              value={code}
              onChange={(event) => onCodeChange(event.target.value)}
              disabled={state.isConfirming}
            />
          </Field>
          <div className="flex flex-wrap gap-3">
            <Button
              data-testid="two-factor-confirm"
              type="submit"
              variant="primary"
              icon="shield"
              disabled={state.isConfirming || isBusy}
            >
              {state.isConfirming ? "Checking..." : "Turn on two-factor"}
            </Button>
            {onAbandon ? (
              <Button
                type="button"
                variant="ghost"
                onClick={onAbandon}
                disabled={state.isConfirming}
              >
                {abandonLabel}
              </Button>
            ) : null}
          </div>
          <p className="text-sm text-muted-foreground">
            Nothing has changed on your account yet. If you stop here, you sign
            in exactly the way you do now.
          </p>
        </form>
      </div>
    );
  }

  if (state.step === "confirmed") {
    return (
      <div className="grid gap-4">
        <StatusBadge variant="success">
          Two-factor authentication is on.
        </StatusBadge>
        <div className="grid gap-1">
          <h3 className="text-sm font-semibold">
            Save these recovery codes now
          </h3>
          <p className="text-sm text-muted-foreground">
            {state.recoveryCodesNotice}
          </p>
          <p className="text-sm text-muted-foreground">
            They are what gets you back into your account if you lose the device
            with your authenticator on it. This is the only time they will be
            shown — there is no page that can display them again.
          </p>
        </div>
        <ul className="grid grid-cols-2 gap-2 rounded-md border border-border bg-muted p-3 font-mono text-sm select-all">
          {state.recoveryCodes.map((recoveryCode) => (
            <li key={recoveryCode} data-testid="two-factor-recovery-code">
              {recoveryCode}
            </li>
          ))}
        </ul>
        <div>
          <Button
            data-testid="two-factor-acknowledge-codes"
            type="button"
            variant="secondary"
            icon="shield"
            onClick={onAcknowledge}
            disabled={isBusy}
          >
            {acknowledgeLabel}
          </Button>
        </div>
      </div>
    );
  }

  if (state.step === "acknowledged") {
    return <StatusBadge variant="success">{acknowledgedNotice}</StatusBadge>;
  }

  return null;
}
