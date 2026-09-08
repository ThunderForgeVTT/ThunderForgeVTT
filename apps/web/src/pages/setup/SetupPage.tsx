import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { getCurrentSession } from "@/services/auth";
import { readTwoFactorStatus } from "@/api/twoFactor";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { AuthLayout } from "@/layouts/auth-layout/AuthLayout";
import { AccountStep } from "@/pages/setup/steps/AccountStep";
import { ReviewStep } from "@/pages/setup/steps/ReviewStep";
import { SecondFactorStep } from "@/pages/setup/steps/SecondFactorStep";
import { SettingsStep } from "@/pages/setup/steps/SettingsStep";
import {
  buildSetupSteps,
  completeSetup,
  fetchInstanceReadiness,
  firstUnansweredStep,
  initialValues,
  readSetupStatus,
  saveSetupSettings,
  SetupRequestError,
  stepPayload,
  validateStep,
  visibleSteps,
  type ReadinessReport,
  type RequiredSetting,
  type SetupStatusWithSettings,
  type SetupStep,
} from "@/services/instanceSetup";
import type { SetupStatus } from "@/types/auth";
import type { SeoConfig } from "@/types/seo";
import { cn } from "@/lib/utils";

/**
 * Spec 040 US1 — first run, as a wizard the registry drives (T069 … T073).
 *
 * # What this page is not, any more
 *
 * It used to be one screen with the account form on it and three decorative
 * captions calling themselves "setup steps". Nothing on it collected the
 * operator identity, the notice contact, the support address or the mail
 * settings that FR-002 says setup collects, and nothing on it could, because
 * the screen was a list of fields somebody typed out by hand.
 *
 * # How the step list is built
 *
 * `GET /authentication/setup/status` returns `required_settings` — the
 * registry's declarations, each saying whether it is satisfied and where its
 * value came from. `buildSetupSteps` groups them by the registry's own group
 * and returns one step per group, in the server's order, with the account, the
 * second factor and the review around them. **No file under `pages/setup/`
 * names a setting key**, except `SettingsStep`'s `notice.` test for where the
 * designated-agent obligation belongs (T073) — and that keys off the
 * declaration, not off a step. Adding a declaration to `settings/registry.rs`
 * puts a field in this wizard with nothing edited here (FR-012).
 *
 * # Resumability (FR-006) is storage, not state
 *
 * Every step writes as it is left, through `POST
 * /authentication/setup/settings`. On load the page asks the server what is
 * answered and starts at the first thing that is not. Nothing is kept in
 * `sessionStorage`, nothing is accumulated across steps and posted at the end.
 * Close the browser, restart the container, come back: the wizard picks up
 * where it stopped because the *instance* did.
 *
 * The one thing that cannot be recovered from storage is the password of a
 * locally created administrator, which the second-factor step needs to
 * authorise enrolment. A reload between creating the account and enrolling
 * therefore means signing in and enrolling there instead — which is spec 041
 * FR-019's entrance, and is the same place an OAuth-bootstrapped administrator
 * ends up. Storing the password to avoid that was considered and rejected.
 *
 * # FR-009 in one sentence
 *
 * A setting whose `source` is `ENVIRONMENT` is never rendered as an input; a
 * step where *every* setting is like that has nothing to ask and is dropped
 * from the walk entirely. An instance configured wholly by environment is
 * therefore asked for the account and the second factor, and then shown the
 * review.
 */

export const setupPageSeo: SeoConfig = {
  title: "Secure first-run setup",
  description:
    "Complete ThunderForge VTT bootstrap with a one-time admin code, local credentials, or an approved OAuth provider.",
  keywords: [
    "ThunderForge setup",
    "bootstrap admin",
    "virtual tabletop onboarding",
  ],
  canonicalPath: "/setup",
  noindex: true,
  prefetchHrefs: ["/setup/callback", "/admin"],
};

interface SetupPageProps {
  setupStatus: SetupStatus;
  onSetupComplete: () => Promise<unknown> | unknown;
}

interface AdminCredentials {
  username: string;
  password: string;
}

export default function SetupPage({
  setupStatus,
  onSetupComplete,
}: SetupPageProps) {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const { code } = useParams();

  const [adminCode, setAdminCode] = useState(code ?? "");
  const [status, setStatus] = useState<SetupStatusWithSettings>(
    setupStatus as SetupStatusWithSettings,
  );
  const [accountCreated, setAccountCreated] = useState(false);
  const [secondFactorConfirmed, setSecondFactorConfirmed] = useState(false);
  const [credentials, setCredentials] = useState<AdminCredentials | null>(null);
  const [values, setValues] = useState<Record<string, string>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  /**
   * The step the operator has *chosen*, or null while they have chosen none.
   *
   * Null is not "step 0": it means "wherever this pass had got to", which is
   * derived from what the server stored (FR-006) rather than remembered. It
   * was a `useState(0)` plus an effect that corrected it after the first
   * status read; that is a cascading render, `react-hooks/set-state-in-effect`
   * says so, and the correction is a derivation rather than a side effect.
   */
  const [chosenIndex, setChosenIndex] = useState<number | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [isCompleting, setIsCompleting] = useState(false);
  const [notice, setNotice] = useState<string | null>(
    searchParams.get("oauth_error"),
  );
  const [readiness, setReadiness] = useState<ReadinessReport | null>(null);
  const [completion, setCompletion] = useState<string | null>(null);
  const [completionFailure, setCompletionFailure] = useState<string | null>(
    null,
  );
  const [missing, setMissing] = useState<string[]>([]);

  const requiredSettings: RequiredSetting[] = useMemo(
    () => status.required_settings ?? [],
    [status],
  );

  /**
   * Everything the instance knows about this pass, in one read: what is
   * answered, who is signed in, and whether that account holds a second
   * factor.
   *
   * Reading and applying are separate so that the mount effect can apply in a
   * `.then` callback rather than in the effect body — the shape
   * `react-hooks/set-state-in-effect` asks for, and the shape
   * `TwoFactorEnrolmentPanel` already uses for the same reason.
   */
  const readSnapshot = useCallback(async () => {
    const next = await readSetupStatus().catch(() => null);
    const session = await getCurrentSession().catch(() => null);
    const admin = Boolean(session?.session?.user?.is_admin);

    // A server that does not report `second_factor_confirmed` yet: ask the
    // account itself. The field is the contract's, and this is what to do
    // until it exists.
    const secondFactor =
      typeof next?.second_factor_confirmed === "boolean"
        ? next.second_factor_confirmed
        : admin
          ? Boolean(
              (await readTwoFactorStatus().catch(() => null))?.confirmedAt,
            )
          : false;

    return { next, admin, secondFactor };
  }, []);

  const applySnapshot = useCallback(
    ({
      next,
      admin,
      secondFactor,
    }: Awaited<ReturnType<typeof readSnapshot>>) => {
      if (next) {
        setStatus(next);
        // What the operator has typed wins over what the server stored: a
        // refresh must not overwrite an edit in progress.
        setValues((current) => ({
          ...initialValues(next.required_settings ?? []),
          ...current,
        }));
      }
      // Both of these only ever ratchet forward. A session read that fails —
      // a transient network error, a `/session` call racing the cookie that
      // `/setup/basic` just issued — must not throw an operator back to the
      // account step to create an administrator that already exists.
      setAccountCreated((previous) => previous || admin);
      setSecondFactorConfirmed((previous) => previous || secondFactor);
    },
    [],
  );

  /** Called after every write: the server's answer is the truth, ours a guess. */
  const refresh = useCallback(
    () => readSnapshot().then(applySnapshot),
    [readSnapshot, applySnapshot],
  );

  useEffect(() => {
    void readSnapshot().then(applySnapshot);
  }, [readSnapshot, applySnapshot]);

  const steps = useMemo(
    () =>
      buildSetupSteps({
        requiredSettings,
        accountCreated,
        secondFactorConfirmed,
      }),
    [requiredSettings, accountCreated, secondFactorConfirmed],
  );

  const walk = useMemo(() => visibleSteps(steps), [steps]);
  const skippedCount = steps.length - walk.length;

  // Where a resumed pass lands, until the operator moves: the first step the
  // instance says is unanswered. Once they have moved, their choice stands —
  // re-deriving it on every refresh would drag somebody who stepped back to
  // correct something forward again.
  const index = Math.min(
    chosenIndex ?? firstUnansweredStep(steps),
    Math.max(walk.length - 1, 0),
  );
  const current: SetupStep | undefined = walk[index];

  useEffect(() => {
    if (current?.kind === "review" && !readiness) {
      void fetchInstanceReadiness().then((report) => {
        if (report) {
          setReadiness(report);
        }
      });
    }
  }, [current, readiness]);

  const onChange = (key: string, value: string) => {
    setValues((previous) => ({ ...previous, [key]: value }));
    setErrors((previous) => {
      if (!previous[key]) {
        return previous;
      }
      const next = { ...previous };
      delete next[key];
      return next;
    });
  };

  const goTo = (nextIndex: number) => {
    setChosenIndex(Math.max(0, Math.min(nextIndex, walk.length - 1)));
    setNotice(null);
  };

  /** Step forward, writing this step's answers first (contract rule 2). */
  const onNext = async () => {
    if (!current) {
      return;
    }

    if (current.kind === "settings" && current.askable.length > 0) {
      const problems = validateStep(current, values);
      if (Object.keys(problems).length > 0) {
        setErrors(problems);
        return;
      }

      const payload = stepPayload(current, values);
      if (Object.keys(payload).length > 0) {
        if (!adminCode.trim()) {
          setNotice("Enter the one-time admin code before saving this step.");
          return;
        }

        setIsSaving(true);
        try {
          await saveSetupSettings(adminCode.trim(), payload);
          await refresh();
        } catch (error) {
          if (error instanceof SetupRequestError) {
            if (error.field) {
              setErrors({ [error.field]: error.message });
            } else {
              setNotice(error.message);
            }

            // The environment took this over while the wizard had it on
            // screen. Re-reading turns the input into a fixed rendering rather
            // than leaving an operator arguing with a field.
            if (error.code === "fixed_by_environment") {
              await refresh();
            }
          } else {
            setNotice(
              error instanceof Error ? error.message : "That step failed.",
            );
          }
          return;
        } finally {
          setIsSaving(false);
        }
      }
    }

    goTo(index + 1);
  };

  const onComplete = async () => {
    setCompletionFailure(null);
    setMissing([]);

    if (!adminCode.trim()) {
      setCompletionFailure(
        "Enter the one-time admin code issued by the server.",
      );
      return;
    }

    setIsCompleting(true);
    try {
      const result = await completeSetup(adminCode.trim());
      setCompletion(result.message);
      setReadiness(result.readiness ?? (await fetchInstanceReadiness()));
      // Setup is over; the administration screen is where the operator now
      // lives. See `onLeave` for why this is no longer an interstitial.
      await onLeave();
    } catch (error) {
      if (error instanceof SetupRequestError) {
        setCompletionFailure(error.message);
        setMissing(error.missing);
      } else {
        setCompletionFailure(
          error instanceof Error ? error.message : "Setup could not finish.",
        );
      }
    } finally {
      setIsCompleting(false);
    }
  };

  /**
   * Finishing setup lands on the administration screen.
   *
   * This used to be a button, because the readiness report rendered on this
   * page was the **only** place an operator would ever see what their instance
   * could not yet do, and navigating away the moment `/complete` returned
   * would have unmounted it unread. That was true when it was written and is
   * not true now: readiness is a permanent admin section
   * (`/admin/readiness`), listed in the nav, derived on every read. Keeping a
   * one-time copy of it behind an extra click made the last step of setup a
   * screen the operator had to dismiss to get anywhere.
   *
   * The report is still built above and still rendered — a completion that
   * happens to fail on the way out leaves it on screen — but nobody has to
   * acknowledge it to leave.
   */
  const onLeave = async () => {
    await onSetupComplete();
    navigate("/admin?bootstrap=complete", { replace: true });
  };

  const onRevisit = (settingKey: string) => {
    const target = walk.findIndex((step) =>
      step.settings.some((setting) => setting.key === settingKey),
    );
    if (target >= 0) {
      goTo(target);
    }
  };

  return (
    <>
      <SEO {...setupPageSeo} />
      <AuthLayout
        eyebrow="First-run setup"
        title="Make this deployment into an instance."
        description="This instance has no administrator and no identity yet. Setup asks for what it needs, tells you what the environment has already fixed, and finishes by saying what it can and cannot do."
        aside={
          <Card surface="parchment" className="grid gap-4 p-6">
            <div className="grid gap-1">
              <h2 className="text-lg font-semibold">Where you are</h2>
              <p className="text-sm text-muted-foreground">
                These steps come from what this instance declares it needs, not
                from a fixed list.
              </p>
            </div>
            <ol data-testid="setup-progress" className="grid gap-2">
              {walk.map((step, position) => (
                <li
                  key={step.id}
                  data-testid={`setup-progress-${step.id}`}
                  data-state={
                    position === index
                      ? "active"
                      : step.complete
                        ? "complete"
                        : "pending"
                  }
                  className={cn(
                    "flex items-center gap-2 rounded-lg border border-border px-3 py-2 text-sm",
                    position === index && "border-ring",
                    step.complete && "border-primary/40 bg-primary/5",
                  )}
                >
                  <span className="inline-flex size-6 items-center justify-center rounded-full bg-muted text-xs font-semibold">
                    {step.complete ? "✓" : position + 1}
                  </span>
                  <span>{step.title}</span>
                </li>
              ))}
            </ol>
            {skippedCount > 0 ? (
              <p
                data-testid="setup-skipped-steps"
                className="text-sm text-muted-foreground"
              >
                {skippedCount} step{skippedCount === 1 ? "" : "s"} skipped: this
                deployment&rsquo;s environment already fixes everything on
                {skippedCount === 1 ? " it" : " them"}.
              </p>
            ) : null}
            <Field
              label="One-time admin code"
              htmlFor="setup-admin-code"
              accent="Required"
              hint="Printed in the server log when this instance started. Every step is written with it."
            >
              <Input
                data-testid="setup-admin-code"
                id="setup-admin-code"
                name="adminCode"
                autoComplete="one-time-code"
                value={adminCode}
                onChange={(event) => setAdminCode(event.target.value)}
                placeholder="ABCD-EFGH-JKLM"
              />
            </Field>
          </Card>
        }
      >
        <div data-testid="setup-wizard" className="grid gap-6">
          <Card
            surface="stone"
            className="grid gap-6 p-6"
            data-testid={current ? `setup-step-${current.id}` : undefined}
          >
            <header className="grid gap-1">
              <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                Step {Math.min(index + 1, walk.length)} of {walk.length}
              </p>
              <h2
                data-testid="setup-step-title"
                className="text-2xl font-semibold"
              >
                {current?.title ?? "Setup"}
              </h2>
            </header>

            {notice ? (
              <div data-testid="setup-notice">
                <StatusBadge variant="warning">{notice}</StatusBadge>
              </div>
            ) : null}

            {current?.kind === "account" ? (
              <AccountStep
                providers={status.configured_oauth_providers}
                adminCode={adminCode}
                created={accountCreated}
                onCreated={(created) => {
                  setCredentials(created);
                  setAccountCreated(true);
                  void refresh();
                  goTo(index + 1);
                }}
              />
            ) : null}

            {current?.kind === "settings" ? (
              <SettingsStep
                step={current}
                values={values}
                errors={errors}
                onChange={onChange}
              />
            ) : null}

            {current?.kind === "second-factor" ? (
              <SecondFactorStep
                credentials={credentials}
                confirmed={secondFactorConfirmed}
                onConfirmed={() => {
                  setSecondFactorConfirmed(true);
                  void refresh();
                }}
              />
            ) : null}

            {current?.kind === "review" ? (
              <ReviewStep
                requiredSettings={requiredSettings}
                readiness={readiness}
                completion={completion}
                failure={completionFailure}
                missing={missing}
                isCompleting={isCompleting}
                onComplete={() => void onComplete()}
                onRevisit={onRevisit}
              />
            ) : null}

            <footer className="flex flex-wrap items-center gap-3 border-t border-border pt-4">
              <Button
                data-testid="setup-back"
                type="button"
                variant="ghost"
                disabled={index === 0}
                onClick={() => goTo(index - 1)}
              >
                Back
              </Button>

              {current && current.kind !== "review" ? (
                <Button
                  data-testid="setup-next"
                  type="button"
                  variant="secondary"
                  icon="rune"
                  disabled={
                    isSaving ||
                    (current.kind === "account" && !accountCreated) ||
                    (current.kind === "second-factor" && !secondFactorConfirmed)
                  }
                  onClick={() => void onNext()}
                >
                  {isSaving ? "Saving..." : "Save and continue"}
                </Button>
              ) : null}

              {current?.kind === "settings" &&
              current.askable.length > 0 &&
              current.askable.every(
                (setting) => setting.requirement === "OPTIONAL",
              ) ? (
                /* FR-003: an entirely optional step may be walked past, and
                 * the review will then say what was left unset. */
                <Button
                  data-testid="setup-skip-step"
                  type="button"
                  variant="ghost"
                  disabled={isSaving}
                  onClick={() => goTo(index + 1)}
                >
                  Skip this
                </Button>
              ) : null}

              {/* Kept as a fallback for the one case that still needs it: a
                  completion that succeeded and whose navigation did not, which
                  would otherwise strand the operator on a finished wizard. */}
              {completion ? (
                <Button
                  data-testid="setup-leave"
                  type="button"
                  variant="primary"
                  icon="shield"
                  onClick={() => void onLeave()}
                >
                  Go to the administration screen
                </Button>
              ) : null}
            </footer>
          </Card>
        </div>
      </AuthLayout>
    </>
  );
}
