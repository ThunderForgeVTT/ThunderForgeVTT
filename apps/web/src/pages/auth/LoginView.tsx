import type { FormEvent } from "react";
import { useEffect, useMemo, useReducer, useRef, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import { getSetupStatus, startOAuthLogin } from "@/api/auth";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { RuneDivider } from "@/components/ui/rune-divider/RuneDivider";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { AuthLayout } from "@/layouts/auth-layout/AuthLayout";
import { TwoFactorEnrolmentSteps } from "@/components/security/TwoFactorEnrolmentSteps";
import { useAuth } from "@/hooks/useAuth";
import {
  beginTwoFactorEnrolment,
  confirmTwoFactorEnrolment,
} from "@/api/twoFactor";
import {
  initialTwoFactorEnrolmentState,
  isConfirmableCode,
  twoFactorEnrolmentReducer,
} from "@/services/twoFactorEnrolment";
import type { InstanceAccessPolicy, SetupProvider } from "@/types/auth";
import { cn } from "@/lib/utils";

/**
 * Spec 041 FR-019. `enrol` is the third one, and it is the difference between
 * a policy an operator can turn on and a lockout an operator can turn on.
 *
 * Before it existed, an instance that required a second factor answered every
 * account that had not enrolled with a challenge it could not answer: the
 * server minted a *verification* challenge, the account had no stored secret,
 * and `verify_two_factor_for_user` answers `false` for exactly that. There was
 * no way out from this screen. Now the server distinguishes the two cases on
 * the wire — `two_factor_required` versus `two_factor_enrolment_required` —
 * so this screen can tell "your code was wrong" from "you have not got one
 * yet, here is how", and the second is a step rather than a refusal.
 */
type LoginStep = "credentials" | "twoFactor" | "enrol";
type LoginField = "identifier" | "password" | "twoFactorCode";

const twoFactorCodePattern = /^\d{6}$/;

function redirectTarget(search: string) {
  const params = new URLSearchParams(search);
  const returnTo = params.get("returnTo");
  return returnTo && returnTo.startsWith("/") ? returnTo : null;
}

function statusVariant(message: string | null) {
  if (!message) {
    return "info" as const;
  }

  const normalized = message.toLowerCase();
  if (
    normalized.includes("invalid") ||
    normalized.includes("failed") ||
    normalized.includes("error")
  ) {
    return "danger" as const;
  }

  if (
    normalized.includes("success") ||
    normalized.includes("authenticated") ||
    normalized.includes("complete")
  ) {
    return "success" as const;
  }

  if (normalized.includes("require") || normalized.includes("await")) {
    return "warning" as const;
  }

  return "info" as const;
}

export function LoginView() {
  const navigate = useNavigate();
  const location = useLocation();
  const { completeTwoFactorChallenge, login, redirectAfterLogin, refresh } =
    useAuth();
  const [identifier, setIdentifier] = useState("");
  const [password, setPassword] = useState("");
  const [twoFactorCode, setTwoFactorCode] = useState("");
  const [twoFactorChallengeId, setTwoFactorChallengeId] = useState<
    string | null
  >(null);
  const [loginStep, setLoginStep] = useState<LoginStep>("credentials");
  // The same reducer the account-settings panel drives (FR-001a). Nothing
  // about the flow is re-implemented here: this screen owns the entrance and
  // the exit, and `TwoFactorEnrolmentSteps` owns everything in between.
  const [enrolment, dispatchEnrolment] = useReducer(
    twoFactorEnrolmentReducer,
    initialTwoFactorEnrolmentState,
  );
  const [enrolmentCode, setEnrolmentCode] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [providers, setProviders] = useState<SetupProvider[]>([]);
  // Spec 035 (FR-003a). Hiding the sign-up link is not what closes the
  // instance — the server refuses independently (ADR-072). This exists so the
  // page can be honest about why the link is gone.
  const [accessPolicy, setAccessPolicy] =
    useState<InstanceAccessPolicy>("open");
  const [touched, setTouched] = useState<Partial<Record<LoginField, boolean>>>(
    {},
  );
  const [credentialAttempted, setCredentialAttempted] = useState(false);
  const [twoFactorAttempted, setTwoFactorAttempted] = useState(false);
  const twoFactorInputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    let active = true;

    void getSetupStatus()
      .then((response) => {
        if (active) {
          setProviders(response.configured_oauth_providers);
          setAccessPolicy(response.access_policy);
        }
      })
      .catch(() => {
        if (active) {
          setProviders([]);
        }
      });

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (loginStep === "twoFactor" || enrolment.step === "provisioning") {
      const timer = window.setTimeout(() => {
        twoFactorInputRef.current?.focus();
      }, 20);

      return () => window.clearTimeout(timer);
    }

    return undefined;
  }, [loginStep, enrolment.step]);

  const markTouched = (...fields: LoginField[]) => {
    setTouched((current) => ({
      ...current,
      ...Object.fromEntries(fields.map((field) => [field, true])),
    }));
  };

  const credentialErrors = {
    identifier: identifier.trim()
      ? undefined
      : "Enter the username or email for this account.",
    password: password ? undefined : "Enter your password.",
  } as const;

  const twoFactorErrors = {
    twoFactorCode: twoFactorCodePattern.test(twoFactorCode.trim())
      ? undefined
      : "Enter the 6-digit code from your authenticator.",
  } as const;

  const credentialFieldError = (field: keyof typeof credentialErrors) =>
    credentialAttempted || touched[field] ? credentialErrors[field] : undefined;

  const twoFactorFieldError = (field: keyof typeof twoFactorErrors) =>
    twoFactorAttempted || touched[field] ? twoFactorErrors[field] : undefined;

  const currentStatusVariant = useMemo(() => statusVariant(status), [status]);

  const onSubmitCredentials = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setCredentialAttempted(true);
    markTouched("identifier", "password");

    if (credentialErrors.identifier || credentialErrors.password) {
      setStatus("Fix the highlighted fields before continuing.");
      return;
    }

    setIsSubmitting(true);
    setStatus(null);

    try {
      const response = await login({
        identifier: identifier.trim(),
        password,
      });

      if (response.loginTwoFactorChallengeId) {
        setTwoFactorChallengeId(response.loginTwoFactorChallengeId);
        setTwoFactorCode("");
        setTwoFactorAttempted(false);

        // FR-019: required, and has not enrolled. The challenge id is the
        // authorisation for the enrolment that follows — it was minted from
        // the password just accepted — so no password is re-typed and no
        // session is needed.
        if (response.status === "two_factor_enrolment_required") {
          setLoginStep("enrol");
          setStatus(response.message);
          void startEnrolment(response.loginTwoFactorChallengeId);
          return;
        }

        setLoginStep("twoFactor");
        setStatus(
          "Credentials accepted. Enter your two-factor code to finish signing in.",
        );
        return;
      }

      setStatus(response.message);
      if (response.session?.authenticated) {
        navigate(
          redirectTarget(location.search) ??
            redirectAfterLogin(response.session.user),
          { replace: true },
        );
      }
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Login failed.");
    } finally {
      setIsSubmitting(false);
    }
  };

  /**
   * FR-019, the entrance. The challenge the login response just carried is the
   * authorisation — see `enrolmentAuthorisationBody` in `@/api/twoFactor` for
   * why it is that and not the password sitting in state one field above.
   */
  const startEnrolment = async (challengeId: string) => {
    dispatchEnrolment({ type: "start" });
    setEnrolmentCode("");

    try {
      const pending = await beginTwoFactorEnrolment({ challengeId });
      dispatchEnrolment({ type: "started", enrolment: pending });
    } catch (error) {
      dispatchEnrolment({
        type: "startFailed",
        message:
          error instanceof Error
            ? error.message
            : "Could not start two-factor setup.",
      });
    }
  };

  const onConfirmEnrolment = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    if (enrolment.step !== "provisioning" || !twoFactorChallengeId) {
      return;
    }

    if (!isConfirmableCode(enrolmentCode)) {
      dispatchEnrolment({
        type: "codeRejected",
        message: "Enter the six digits your authenticator is showing.",
      });
      return;
    }

    dispatchEnrolment({ type: "submitCode" });

    try {
      const confirmation = await confirmTwoFactorEnrolment(
        { challengeId: twoFactorChallengeId },
        enrolmentCode,
      );
      setEnrolmentCode("");
      dispatchEnrolment({ type: "confirmed", confirmation });
      setStatus(
        confirmation.signedIn
          ? "Two-factor is on and you are signed in. Save your recovery codes before you continue."
          : "Two-factor is on. Save your recovery codes, then sign in again.",
      );
    } catch (error) {
      // Not a restart, and not a lost sign-in: a refused code leaves both the
      // pending secret and the challenge exactly where they were (FR-001c),
      // because the server only spends either one on success.
      dispatchEnrolment({
        type: "codeRejected",
        message:
          error instanceof Error
            ? error.message
            : "That code was not accepted. Try the next one.",
      });
    }
  };

  /**
   * FR-020. Confirming issued the session cookie, so the sign-in that was
   * interrupted is already finished on the server; this reads it back and goes
   * where the person was going before the requirement stopped them.
   */
  const onFinishEnrolment = async () => {
    dispatchEnrolment({ type: "acknowledgeRecoveryCodes" });
    setIsSubmitting(true);

    try {
      const response = await refresh();
      if (!response?.session?.authenticated) {
        setStatus("Two-factor is on. Sign in again to continue.");
        setLoginStep("credentials");
        return;
      }

      navigate(
        redirectTarget(location.search) ??
          redirectAfterLogin(response.session.user),
        { replace: true },
      );
    } catch {
      setStatus("Two-factor is on. Sign in again to continue.");
      setLoginStep("credentials");
    } finally {
      setIsSubmitting(false);
    }
  };

  const onSubmitTwoFactor = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setTwoFactorAttempted(true);
    markTouched("twoFactorCode");

    if (!twoFactorChallengeId) {
      setStatus("Your two-factor challenge expired. Sign in again.");
      setLoginStep("credentials");
      return;
    }

    if (twoFactorErrors.twoFactorCode) {
      setStatus("Enter a valid 6-digit code.");
      return;
    }

    setIsSubmitting(true);
    setStatus(null);

    try {
      const response = await completeTwoFactorChallenge(
        twoFactorChallengeId,
        twoFactorCode.trim(),
      );
      setStatus(response.message);
      navigate(
        redirectTarget(location.search) ??
          redirectAfterLogin(response.session?.user),
        {
          replace: true,
        },
      );
    } catch (error) {
      setStatus(
        error instanceof Error ? error.message : "Failed to verify 2FA.",
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  const onRefreshSecondSeal = async () => {
    setCredentialAttempted(true);
    markTouched("identifier", "password");

    if (credentialErrors.identifier || credentialErrors.password) {
      setStatus("Fix your credentials before requesting a new code.");
      setLoginStep("credentials");
      return;
    }

    setIsSubmitting(true);
    setStatus(null);

    try {
      const response = await login({
        identifier: identifier.trim(),
        password,
      });

      if (response.loginTwoFactorChallengeId) {
        setTwoFactorChallengeId(response.loginTwoFactorChallengeId);
        setTwoFactorCode("");
        setStatus(
          "A new two-factor challenge was issued. Enter the latest code.",
        );
        return;
      }

      setStatus(response.message);
      if (response.session?.authenticated) {
        navigate(
          redirectTarget(location.search) ??
            redirectAfterLogin(response.session.user),
          { replace: true },
        );
      }
    } catch (error) {
      setStatus(
        error instanceof Error
          ? error.message
          : "Failed to refresh the challenge.",
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  const onReturnToCredentials = () => {
    setLoginStep("credentials");
    setTwoFactorChallengeId(null);
    setTwoFactorCode("");
    setTwoFactorAttempted(false);
    // FR-004: abandoning is a client-side act. The pending secret on the
    // server is not a second factor and the account is exactly as it was.
    dispatchEnrolment({ type: "abandon" });
    setEnrolmentCode("");
    setStatus("Adjust your credentials, then sign in again.");
  };

  return (
    <AuthLayout
      aside={
        <Card className="p-5">
          <div className="grid gap-2">
            {accessPolicy === "open" ? (
              <Link
                to={`/register${location.search}`}
                className="font-medium text-primary hover:underline"
              >
                Create a local account
              </Link>
            ) : (
              // FR-003a: say so, rather than silently omitting the link. A
              // page that just drops sign-up is indistinguishable from a
              // broken one, and it leaves someone holding an unclicked
              // invitation with no way to tell the instance is working.
              <p
                className="text-sm text-muted-foreground"
                data-testid="instance-not-open-notice"
              >
                {accessPolicy === "invite_only"
                  ? "This instance is invite only. Open the invitation link you were sent to create an account."
                  : "This instance is not accepting new accounts."}
              </p>
            )}
            <Link
              to="/welcome"
              className="font-medium text-primary hover:underline"
            >
              Review the welcome hall
            </Link>
          </div>
        </Card>
      }
    >
      <div className="grid gap-5">
        <div className="relative grid gap-4">
          <Card
            className={cn("p-6", loginStep !== "credentials" && "opacity-60")}
            data-ambient-sound="guild-hall-candles"
          >
            <form onSubmit={onSubmitCredentials} className="grid gap-4">
              <h2 className="text-lg font-semibold">Sign in</h2>

              {loginStep !== "credentials" ? (
                <div className="grid gap-2 rounded-lg border border-border bg-secondary p-3">
                  <div className="flex items-center justify-between gap-3">
                    <span className="inline-flex items-center gap-2 text-sm font-medium">
                      <FantasyIcon name="spark" size={14} />
                      Credentials accepted
                    </span>
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      onClick={onReturnToCredentials}
                    >
                      Edit
                    </Button>
                  </div>
                  <p className="text-sm text-muted-foreground">
                    Signed in as <strong>{identifier.trim()}</strong>. Finish
                    the two-factor step to enter.
                  </p>
                </div>
              ) : null}

              <div className="grid gap-4">
                <Field
                  label="Email address or username"
                  htmlFor="login-identifier"
                  accent="Required"
                  error={credentialFieldError("identifier")}
                >
                  <Input
                    id="login-identifier"
                    name="identifier"
                    autoComplete="username"
                    value={identifier}
                    onBlur={() => markTouched("identifier")}
                    onChange={(event) => setIdentifier(event.target.value)}
                    placeholder="founder@thunderforge.app"
                    disabled={isSubmitting || loginStep !== "credentials"}
                  />
                </Field>

                <Field
                  label="Password"
                  htmlFor="login-password"
                  accent="Required"
                  error={credentialFieldError("password")}
                >
                  <div className="relative">
                    <Input
                      id="login-password"
                      name="password"
                      type={showPassword ? "text" : "password"}
                      autoComplete="current-password"
                      value={password}
                      onBlur={() => markTouched("password")}
                      onChange={(event) => setPassword(event.target.value)}
                      className="pr-16"
                      placeholder="Enter your password"
                      disabled={isSubmitting || loginStep !== "credentials"}
                    />
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      className="absolute top-1/2 right-1 -translate-y-1/2"
                      onClick={() => setShowPassword((current) => !current)}
                      aria-label={
                        showPassword ? "Hide password" : "Show password"
                      }
                    >
                      {showPassword ? "Hide" : "Show"}
                    </Button>
                  </div>
                </Field>
              </div>

              <div className="flex flex-wrap gap-3">
                <Button
                  type="submit"
                  variant="primary"
                  size="lg"
                  disabled={isSubmitting || loginStep !== "credentials"}
                  icon="shield"
                >
                  {isSubmitting && loginStep === "credentials"
                    ? "Signing in..."
                    : "Sign In"}
                </Button>
              </div>

              {loginStep === "credentials" && providers.length ? (
                <div className="grid gap-3">
                  <RuneDivider label="OAuth sign-in" />
                  <div className="flex flex-wrap gap-3">
                    {providers.map((provider) => (
                      <Button
                        key={provider.provider_key}
                        type="button"
                        variant="secondary"
                        icon="wand"
                        onClick={() =>
                          startOAuthLogin(
                            provider.provider_key,
                            redirectTarget(location.search) ?? "/welcome",
                          )
                        }
                      >
                        Continue with {provider.display_name}
                      </Button>
                    ))}
                  </div>
                </div>
              ) : null}
            </form>
          </Card>

          {loginStep === "twoFactor" ? (
            <Card
              className={cn(
                "border-2 p-6",
                currentStatusVariant === "danger" && "border-destructive",
                currentStatusVariant === "success" && "border-emerald-600",
              )}
            >
              <form onSubmit={onSubmitTwoFactor} className="grid gap-4">
                <h3 className="text-lg font-semibold">Two-factor code</h3>

                <Field
                  label="Authentication code"
                  htmlFor="login-two-factor"
                  accent="Required"
                  error={twoFactorFieldError("twoFactorCode")}
                  hint="Use the latest 6-digit code from your authenticator."
                >
                  <Input
                    ref={twoFactorInputRef}
                    id="login-two-factor"
                    name="twoFactorCode"
                    inputMode="numeric"
                    autoComplete="one-time-code"
                    maxLength={6}
                    value={twoFactorCode}
                    onBlur={() => markTouched("twoFactorCode")}
                    onChange={(event) =>
                      setTwoFactorCode(
                        event.target.value.replace(/\D/g, "").slice(0, 6),
                      )
                    }
                    className="text-center text-lg tracking-[0.3em]"
                    placeholder="123456"
                  />
                </Field>

                <div className="flex flex-wrap gap-3">
                  <Button
                    type="submit"
                    variant="primary"
                    size="lg"
                    disabled={isSubmitting}
                    icon="spark"
                  >
                    {isSubmitting ? "Verifying..." : "Verify"}
                  </Button>
                  <Button
                    type="button"
                    variant="secondary"
                    size="lg"
                    disabled={isSubmitting}
                    onClick={() => void onRefreshSecondSeal()}
                  >
                    Resend code
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="lg"
                    disabled={isSubmitting}
                    onClick={onReturnToCredentials}
                  >
                    Back to credentials
                  </Button>
                </div>
              </form>
            </Card>
          ) : null}

          {loginStep === "enrol" ? (
            <Card className="border-2 p-6" data-testid="login-two-factor-enrol">
              <div className="grid gap-5">
                <div className="grid gap-1">
                  <h3 className="text-lg font-semibold">
                    Set up two-factor authentication
                  </h3>
                  <p className="text-sm text-muted-foreground">
                    This instance requires a second factor. Your password was
                    accepted — add ThunderForge to an authenticator app and
                    prove it works with one code, and you will be signed in.
                  </p>
                </div>

                {enrolment.step === "starting" ? (
                  <StatusBadge variant="info">
                    Preparing your authenticator setup…
                  </StatusBadge>
                ) : null}

                {enrolment.step === "idle" ? (
                  <div className="grid gap-3">
                    {enrolment.error ? (
                      <StatusBadge variant="danger">
                        {enrolment.error}
                      </StatusBadge>
                    ) : null}
                    <div>
                      <Button
                        type="button"
                        variant="primary"
                        icon="shield"
                        disabled={isSubmitting || !twoFactorChallengeId}
                        onClick={() => {
                          if (twoFactorChallengeId) {
                            void startEnrolment(twoFactorChallengeId);
                          }
                        }}
                      >
                        Try again
                      </Button>
                    </div>
                  </div>
                ) : null}

                <TwoFactorEnrolmentSteps
                  state={enrolment}
                  code={enrolmentCode}
                  onCodeChange={setEnrolmentCode}
                  onConfirm={onConfirmEnrolment}
                  onAbandon={onReturnToCredentials}
                  abandonLabel="Back to credentials"
                  onAcknowledge={() => void onFinishEnrolment()}
                  acknowledgeLabel="I have saved these codes — continue"
                  acknowledgedNotice="Two-factor authentication is on. Taking you where you were going…"
                  codeInputRef={twoFactorInputRef}
                  isBusy={isSubmitting}
                />
              </div>
            </Card>
          ) : null}
        </div>

        <div aria-live="polite">
          {status ? (
            <StatusBadge variant={currentStatusVariant}>{status}</StatusBadge>
          ) : null}
        </div>
      </div>
    </AuthLayout>
  );
}
