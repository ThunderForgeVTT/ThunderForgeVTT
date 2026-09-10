import { useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import {
  findAdminAccount,
  resetAccountSecondFactor,
  setAccountTwoFactorRequired,
  type AdminAccountView,
} from "@/api/twoFactor";

/**
 * Spec 041 US6/US7: the two things an operator may do to somebody else's
 * second factor.
 *
 * # Why this replaces a database edit
 *
 * FR-024 asks for "a defined path, without editing the database", and until
 * this control the only way to help somebody who had lost both their
 * authenticator and their recovery codes was `psql` against a live table:
 * unaudited by construction, unnotified, and done from memory at the worst
 * possible moment. Everything here is recorded with the operator's own id
 * against it, and the account holder is told.
 *
 * # Why it is a lookup and not a list
 *
 * An operator with a real reason to act on somebody's second factor already
 * knows which account it is, because that person has just asked them for help.
 * A browsable list of accounts without a second factor is a target list for
 * whoever takes over this session — the same reason `twoFactorCoverage` is
 * three integers rather than a roster.
 *
 * # What it deliberately cannot do
 *
 * Enrol a factor, or issue recovery codes. An operator who could hand out a
 * working second factor could sign in as the account holder, and no amount of
 * audit trail makes that acceptable. A reset leaves the account with no factor;
 * the person enrols again from their own screen, with a secret only they see.
 */
export function UserTwoFactorControl() {
  const [identifier, setIdentifier] = useState("");
  const [account, setAccount] = useState<AdminAccountView | null>(null);
  const [searched, setSearched] = useState(false);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  /**
   * A reset is the one action here that weakens an account, so it is asked
   * twice. Not a modal: the second click is on the same button, which keeps
   * the account it is about on the screen while the operator decides.
   */
  const [resetArmed, setResetArmed] = useState(false);

  const run = async (work: () => Promise<string>) => {
    setBusy(true);
    setNotice(null);
    setError(null);
    try {
      const message = await work();
      setNotice(message);
      // Re-read rather than patch: the server is the authority on what the
      // account now looks like, and a patched local copy is how a panel comes
      // to disagree with the thing it is describing.
      setAccount(await findAdminAccount(identifier));
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : "That could not be done.",
      );
    } finally {
      setBusy(false);
      setResetArmed(false);
    }
  };

  const lookup = async () => {
    setSearched(true);
    setResetArmed(false);
    await run(async () => {
      const found = await findAdminAccount(identifier);
      setAccount(found);
      return found
        ? `Found ${found.username}.`
        : "No account has that username or email address.";
    });
  };

  return (
    <div className="grid gap-3">
      <div>
        <h3 className="font-semibold">One account&rsquo;s second factor</h3>
        <p className="text-muted-foreground">
          Require a second factor of one person, or reset one for somebody who
          has lost both their authenticator and their recovery codes. Every
          change here is recorded against your account, and they are told.
        </p>
      </div>

      <form
        className="flex flex-wrap items-end gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          void lookup();
        }}
      >
        <label className="grid gap-1 text-sm">
          <span>Username or email address</span>
          <input
            className="h-9 rounded-md border bg-background px-2"
            value={identifier}
            onChange={(event) => setIdentifier(event.target.value)}
            data-testid="admin-account-identifier"
            autoComplete="off"
          />
        </label>
        <Button
          type="submit"
          variant="secondary"
          icon="compass"
          disabled={busy || identifier.trim() === ""}
          data-testid="admin-account-find"
        >
          Find account
        </Button>
      </form>

      {error ? (
        <StatusBadge variant="danger" data-testid="admin-account-error">
          {error}
        </StatusBadge>
      ) : null}
      {notice ? (
        <StatusBadge variant="info" data-testid="admin-account-notice">
          {notice}
        </StatusBadge>
      ) : null}

      {searched && !account && !busy ? (
        <p
          className="text-sm text-muted-foreground"
          data-testid="admin-account-missing"
        >
          No account has that username or email address. It is matched exactly,
          not searched.
        </p>
      ) : null}

      {account ? (
        <div
          className="grid gap-3 rounded-md border bg-muted/30 p-3 text-sm"
          data-testid="admin-account"
        >
          <div className="grid gap-0.5">
            <span className="font-medium" data-testid="admin-account-username">
              {account.username}
            </span>
            <span className="text-muted-foreground">{account.email}</span>
            <span data-testid="admin-account-factor-state">
              {account.twoFactorEnabled
                ? "Holds a confirmed second factor."
                : "Holds no second factor."}
            </span>
          </div>

          {/*
           * FR-027 / ADR-094. An administrator must hold one because of the
           * role, so this control has nothing to offer for one — and saying
           * why is better than a disabled switch nobody can explain.
           */}
          {account.isAdmin ? (
            <p
              className="text-muted-foreground"
              data-testid="admin-account-is-admin"
            >
              This is an administrator account, so a second factor is already
              required by the role. There is no switch for that.
            </p>
          ) : (
            <Button
              type="button"
              variant="secondary"
              icon="shield"
              disabled={busy}
              data-testid="admin-account-toggle-required"
              onClick={() =>
                void run(() =>
                  setAccountTwoFactorRequired(
                    account.id,
                    !account.twoFactorAdminRequired,
                  ),
                )
              }
            >
              {account.twoFactorAdminRequired
                ? "Stop requiring a second factor"
                : "Require a second factor of this account"}
            </Button>
          )}

          <div className="grid gap-1">
            <Button
              type="button"
              variant={resetArmed ? "primary" : "ghost"}
              icon="spark"
              disabled={busy || !account.twoFactorEnabled}
              data-testid="admin-account-reset"
              onClick={() => {
                if (!resetArmed) {
                  setResetArmed(true);
                  return;
                }
                void run(() => resetAccountSecondFactor(account.id));
              }}
            >
              {resetArmed
                ? `Yes — reset ${account.username}'s second factor`
                : "Reset this account's second factor"}
            </Button>
            <p className="text-xs text-muted-foreground">
              {account.twoFactorEnabled
                ? "The account will sign in on its password alone until it enrols again. No code, no secret and no session is handed to you."
                : "This account holds no second factor, so there is nothing to reset."}
            </p>
          </div>
        </div>
      ) : null}
    </div>
  );
}
