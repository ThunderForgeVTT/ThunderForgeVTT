import { useState } from "react";
import { changePassword } from "@/api/password";
import { Button } from "@/components/ui/button/Button";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";

/**
 * Spec 036 FR-008: change the password, and end every other session with it.
 *
 * # There was no way to do this
 *
 * Not a hidden one, not an awkward one — none. `password_hash` was written at
 * registration, at the admin bootstrap and by OAuth auto-provisioning, and
 * never again. Somebody who believed their password had been seen had one
 * remedy, which was to stop using the account.
 *
 * # Why it says how many sessions went
 *
 * Because that is the part somebody needs to see. The reason to change a
 * password is usually that somebody else has it, and somebody else who has it
 * is signed in; "your password has been changed" alone leaves the question
 * that actually matters unanswered.
 *
 * # Why it asks for the current password and not a code
 *
 * Unlike turning a second factor *off*, this makes no future sign-in cheaper.
 * Demanding possession here would mean somebody who has lost their
 * authenticator cannot rotate a password they think is compromised, which
 * trades the urgent risk for the smaller one.
 */
export function PasswordChangePanel({ onChanged }: { onChanged?: () => void }) {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);

  // Checked here and not on the server: the server has no idea what the
  // person meant to type twice, and a mismatch is a typo rather than a
  // refusal. Everything the server *can* decide — length, sameness, the
  // current password — it decides, and this does not second-guess it.
  const mismatched = confirm.length > 0 && next !== confirm;
  const ready = Boolean(current) && Boolean(next) && next === confirm;

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      const result = await changePassword({
        currentPassword: current,
        newPassword: next,
      });
      setDone(result.message);
      setCurrent("");
      setNext("");
      setConfirm("");
      onChanged?.();
    } catch (caught) {
      setError(
        caught instanceof Error
          ? caught.message
          : "The password could not be changed.",
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <section
      className="grid gap-3 rounded-lg border border-border p-4"
      data-testid="password-change"
    >
      <div className="grid gap-1">
        <h3 className="font-semibold">Change your password</h3>
        <p className="text-sm text-muted-foreground">
          Every other session signs out when you do this. The one you are on
          stays.
        </p>
      </div>

      {done ? (
        <StatusBadge variant="success" data-testid="password-change-done">
          {done}
        </StatusBadge>
      ) : null}

      <Field label="Your current password" htmlFor="password-change-current">
        <Input
          id="password-change-current"
          data-testid="password-change-current"
          type="password"
          autoComplete="current-password"
          value={current}
          onChange={(event) => setCurrent(event.target.value)}
        />
      </Field>

      <Field label="A new password" htmlFor="password-change-new">
        <Input
          id="password-change-new"
          data-testid="password-change-new"
          type="password"
          autoComplete="new-password"
          value={next}
          onChange={(event) => setNext(event.target.value)}
        />
      </Field>

      <Field label="The new password again" htmlFor="password-change-confirm">
        <Input
          id="password-change-confirm"
          data-testid="password-change-confirm"
          type="password"
          autoComplete="new-password"
          value={confirm}
          onChange={(event) => setConfirm(event.target.value)}
        />
      </Field>

      {mismatched ? (
        <StatusBadge variant="warning" data-testid="password-change-mismatch">
          Those two do not match.
        </StatusBadge>
      ) : null}

      {error ? (
        <StatusBadge variant="danger" data-testid="password-change-error">
          {error}
        </StatusBadge>
      ) : null}

      <div>
        <Button
          type="button"
          variant="secondary"
          disabled={busy || !ready}
          onClick={() => void submit()}
          data-testid="password-change-submit"
        >
          {busy ? "Changing..." : "Change password"}
        </Button>
      </div>
    </section>
  );
}
