import { useState } from "react";
import { disableTwoFactor } from "@/api/twoFactor";
import { Button } from "@/components/ui/button/Button";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";

/**
 * Spec 041 US4 (FR-012, FR-014): the deliberate way off.
 *
 * # There was not one
 *
 * Until this, the only route out of two-factor was a side effect: beginning a
 * new enrolment cleared the old factor. So the way to turn it off was to use
 * the button that turns it on and then walk away — and an account that
 * abandoned an enrolment lost the factor it already had, without being asked
 * for anything.
 *
 * # Why it asks for both
 *
 * The password and a code. That is exactly what adding a factor cost, which is
 * the point: removal is the one action that makes every future sign-in
 * cheaper, and it should not be reachable from a session somebody left open.
 *
 * A recovery code is accepted in place of the authenticator code, because
 * "my phone is gone" is the most likely reason somebody wants this and
 * refusing them would leave the deliberate route open only to people who do
 * not need it.
 *
 * # Why it is behind a disclosure rather than on the page
 *
 * Nothing here should be one misclick from happening, and a form that is
 * always open invites being filled in. It is not hidden — the account holder
 * came to this page to manage exactly this — but it asks to be opened first.
 */
export function TwoFactorRemovalPanel({
  onRemoved,
}: {
  onRemoved?: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [useRecoveryCode, setUseRecoveryCode] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      const message = await disableTwoFactor({
        password,
        code: useRecoveryCode ? undefined : code,
        recoveryCode: useRecoveryCode ? code : undefined,
      });
      setDone(message);
      setPassword("");
      setCode("");
      onRemoved?.();
    } catch (caught) {
      // Kept on screen with the fields intact. A refusal here is usually
      // "your administrator requires this", which is not something retyping a
      // password fixes.
      setError(
        caught instanceof Error
          ? caught.message
          : "The second factor could not be turned off.",
      );
    } finally {
      setBusy(false);
    }
  };

  if (done) {
    return (
      <div data-testid="two-factor-removed">
        <StatusBadge variant="success">{done}</StatusBadge>
      </div>
    );
  }

  return (
    <section
      className="grid gap-3 rounded-lg border border-border p-4"
      data-testid="two-factor-removal"
    >
      <div className="grid gap-1">
        <h3 className="font-semibold">Turn off two-factor authentication</h3>
        <p className="text-sm text-muted-foreground">
          This asks for your password and one code — the same things that
          turning it on cost.
        </p>
      </div>

      {open ? (
        <div className="grid gap-3">
          <Field label="Your password" htmlFor="two-factor-removal-password">
            <Input
              id="two-factor-removal-password"
              data-testid="two-factor-removal-password"
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
            />
          </Field>

          <Field
            label={
              useRecoveryCode
                ? "A recovery code"
                : "A code from your authenticator"
            }
            htmlFor="two-factor-removal-code"
          >
            <Input
              id="two-factor-removal-code"
              data-testid="two-factor-removal-code"
              autoComplete="one-time-code"
              inputMode={useRecoveryCode ? "text" : "numeric"}
              value={code}
              onChange={(event) => setCode(event.target.value)}
            />
          </Field>

          <label className="flex items-center gap-2 text-sm text-muted-foreground">
            <input
              type="checkbox"
              className="size-4"
              data-testid="two-factor-removal-use-recovery"
              checked={useRecoveryCode}
              onChange={(event) => {
                setUseRecoveryCode(event.target.checked);
                setCode("");
              }}
            />
            Use a recovery code instead — for when the authenticator is gone
          </label>

          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              variant="danger"
              disabled={busy || !password || !code}
              onClick={() => void submit()}
              data-testid="two-factor-removal-submit"
            >
              {busy ? "Turning it off..." : "Turn it off"}
            </Button>
            <Button
              type="button"
              variant="ghost"
              disabled={busy}
              onClick={() => {
                setOpen(false);
                setError(null);
                setPassword("");
                setCode("");
              }}
            >
              Cancel
            </Button>
          </div>

          {error ? (
            <div data-testid="two-factor-removal-error">
              <StatusBadge variant="danger">{error}</StatusBadge>
            </div>
          ) : null}
        </div>
      ) : (
        <div>
          <Button
            type="button"
            variant="ghost"
            onClick={() => setOpen(true)}
            data-testid="two-factor-removal-open"
          >
            Turn it off
          </Button>
        </div>
      )}
    </section>
  );
}
