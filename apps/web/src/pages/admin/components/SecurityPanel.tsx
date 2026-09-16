import { useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { UserTwoFactorControl } from "@/pages/admin/components/UserTwoFactorControl";
import type {
  AdminBootstrapSettings,
  AuthSecuritySettings,
  TwoFactorCoverage,
} from "@/types/admin";

interface SecurityPanelProps {
  settings: AuthSecuritySettings;
  bootstrapSettings: AdminBootstrapSettings | null;
  /**
   * Spec 041 FR-021. Beside the switch, because the figure that matters is
   * how many people the switch is about to ask something of.
   */
  coverage: TwoFactorCoverage;
  onUpdate: (requiredForAllUsers: boolean) => Promise<AuthSecuritySettings>;
}

export function SecurityPanel({
  settings,
  bootstrapSettings,
  coverage,
  onUpdate,
}: SecurityPanelProps) {
  const [requiredForAllUsers, setRequiredForAllUsers] = useState(
    settings.twoFactorRequiredForAllUsers,
  );
  const [isSaving, setIsSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  const handleSave = async () => {
    setIsSaving(true);
    setStatus(null);

    try {
      await onUpdate(requiredForAllUsers);
      setStatus("2FA enforcement policy updated.");
    } catch (error) {
      setStatus(
        error instanceof Error
          ? error.message
          : "Failed to update security policy.",
      );
    } finally {
      setIsSaving(false);
    }
  };

  const pending = requiredForAllUsers !== settings.twoFactorRequiredForAllUsers;

  return (
    <div className="grid gap-6">
      {/*
       * What this screen is telling you, in one sentence, before any control.
       *
       * The owner's report was "I have no idea what this screen is really
       * telling me", which is fair: it opened on a switch whose two labels
       * were both descriptions of policy machinery ("where user-level or
       * admin policy already applies") and a three-line count block below it.
       * Nothing said who is affected right now, and nothing said what happens
       * to a person who has not enrolled when the switch goes on — which is
       * the only question an operator actually has before throwing it.
       */}
      <div className="grid gap-3">
        <div className="grid gap-1">
          <h3 className="text-lg font-semibold">Two-factor enforcement</h3>
          <p className="max-w-[70ch] text-muted-foreground">
            A second factor is an authenticator code asked for after the
            password. Administrators must always hold one, whatever this switch
            says. The switch decides whether everybody else must too — and
            nobody is ever locked out by it: a person without one is taken
            through enrolling at their next sign-in.
          </p>
        </div>

        {/*
         * FR-021 / SC-007. An operator about to require a second factor of
         * everybody is entitled to know how many people that is before they
         * throw the switch — and to know it as a number rather than as a list
         * of who is exposed. `requiredNotEnrolled` already counts every
         * administrator, because the role requires one whatever this switch
         * says.
         *
         * The counts are the headline now rather than a line of small text
         * under the control, because they are the only facts on this screen an
         * operator can act on.
         */}
        <div
          className="grid gap-3 rounded-lg border border-border bg-muted/30 p-4"
          data-testid="two-factor-coverage"
        >
          <dl className="grid grid-cols-2 gap-4 sm:grid-cols-3">
            <div data-testid="two-factor-coverage-enrolled">
              <dt className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                With a second factor
              </dt>
              <dd className="text-2xl font-semibold tabular-nums">
                {coverage.enrolled}
              </dd>
            </div>
            <div data-testid="two-factor-coverage-not-enrolled">
              <dt className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                Without
              </dt>
              <dd className="text-2xl font-semibold tabular-nums">
                {coverage.notEnrolled}
              </dd>
            </div>
            <div>
              <dt className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                Required and not yet enrolled
              </dt>
              <dd
                className={cn(
                  "text-2xl font-semibold tabular-nums",
                  coverage.requiredNotEnrolled > 0 &&
                    "text-amber-600 dark:text-amber-400",
                )}
              >
                {coverage.requiredNotEnrolled}
              </dd>
            </div>
          </dl>
          <p className="max-w-[70ch] text-sm text-muted-foreground">
            {!pending
              ? coverage.requiredNotEnrolled === 0
                ? "Everyone who has to hold one, does."
                : `${coverage.requiredNotEnrolled} ${
                    coverage.requiredNotEnrolled === 1
                      ? "account is"
                      : "accounts are"
                  } required to hold one and has not enrolled yet. They are asked at their next sign-in; until then they can still sign in.`
              : requiredForAllUsers
                ? `Turning this on asks ${coverage.notEnrolled} ${
                    coverage.notEnrolled === 1 ? "person" : "people"
                  } to enrol at their next sign-in. Nobody is locked out; they are taken through it.`
                : "Turning this off asks nothing of anybody. Every factor already confirmed stays in force."}
          </p>
          {/*
           * Room is left here, deliberately, for the thing this screen cannot
           * yet do: ask the people counted above to enrol. The owner chose a
           * skippable prompt at sign-in rather than a mail-out or a lockout,
           * and building it needs server work (a per-account "asked at" record,
           * so a person who skips is not asked again the same session). It is
           * a separate task; nothing here should grow into half of it.
           */}
        </div>

        <div className="grid gap-2 rounded-lg border border-border p-4">
          <label className="flex items-start gap-3 text-sm">
            <Switch
              checked={requiredForAllUsers}
              onCheckedChange={(checked) => setRequiredForAllUsers(checked)}
            />
            <span>
              <span className="block font-medium">
                Require a second factor of every account
              </span>
              <span className="block text-muted-foreground">
                {requiredForAllUsers
                  ? "On: everyone enrols. Off, only administrators and accounts an operator has individually required must."
                  : "Off: only administrators, and accounts an operator has individually required, must hold one."}
              </span>
            </span>
          </label>
          <div className="flex flex-wrap items-center gap-3">
            <Button
              type="button"
              variant="secondary"
              icon="shield"
              onClick={() => void handleSave()}
              disabled={isSaving || !pending}
            >
              {isSaving ? "Applying..." : "Update security policy"}
            </Button>
            {pending ? (
              <span className="text-sm text-muted-foreground">
                Not applied yet.
              </span>
            ) : null}
          </div>
        </div>
      </div>

      {/*
       * Spec 041 US6/US7. Beside the instance-wide switch because they are
       * the same decision at two scales, and because an operator who has just
       * read "3 accounts are required and have not enrolled" is one click
       * from the account they were told about.
       */}
      <UserTwoFactorControl />

      <div className="grid gap-3">
        <div className="grid gap-1">
          <h3 className="text-lg font-semibold">Bootstrap record</h3>
          <p className="max-w-[70ch] text-muted-foreground">
            When this instance was first set up, and whether it finished. It
            matters in one situation: if setup never completed, the first-run
            wizard is still reachable and the one-time admin code may still be
            live — so an instance that says “No” here is an instance somebody
            else could still claim.
          </p>
        </div>
        {bootstrapSettings ? (
          <div className="grid gap-1">
            <p className="text-muted-foreground">
              <strong className="text-foreground">Setup completed:</strong>{" "}
              {bootstrapSettings.setupCompleted ? "Yes" : "No"}
            </p>
            <p className="text-muted-foreground">
              <strong className="text-foreground">Admin code generated:</strong>{" "}
              {bootstrapSettings.adminCodeGeneratedAt
                ? new Date(
                    bootstrapSettings.adminCodeGeneratedAt,
                  ).toLocaleString()
                : "Not recorded"}
            </p>
            <p className="text-muted-foreground">
              <strong className="text-foreground">Setup completed at:</strong>{" "}
              {bootstrapSettings.setupCompletedAt
                ? new Date(bootstrapSettings.setupCompletedAt).toLocaleString()
                : "Pending"}
            </p>
          </div>
        ) : (
          <StatusBadge variant="warning">
            Bootstrap settings are unavailable.
          </StatusBadge>
        )}
      </div>

      {status ? <StatusBadge variant="info">{status}</StatusBadge> : null}
    </div>
  );
}
