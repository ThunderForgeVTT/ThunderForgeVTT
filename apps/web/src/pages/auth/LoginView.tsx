import type { FormEvent } from "react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import { getSetupStatus, startOAuthLogin } from "@/api/auth";
import { Avatar } from "@/components/ui/avatar/Avatar";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { RuneDivider } from "@/components/ui/rune-divider/RuneDivider";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { AuthLayout } from "@/layouts/auth-layout/AuthLayout";
import { useAuth } from "@/hooks/useAuth";
import type { SetupProvider } from "@/types/auth";
import { cn } from "@/lib/utils";

type LoginStep = "credentials" | "twoFactor";
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
  const { completeTwoFactorChallenge, login, redirectAfterLogin } = useAuth();
  const [identifier, setIdentifier] = useState("");
  const [password, setPassword] = useState("");
  const [twoFactorCode, setTwoFactorCode] = useState("");
  const [twoFactorChallengeId, setTwoFactorChallengeId] = useState<string | null>(
    null,
  );
  const [loginStep, setLoginStep] = useState<LoginStep>("credentials");
  const [showPassword, setShowPassword] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [providers, setProviders] = useState<SetupProvider[]>([]);
  const [touched, setTouched] = useState<Partial<Record<LoginField, boolean>>>({});
  const [credentialAttempted, setCredentialAttempted] = useState(false);
  const [twoFactorAttempted, setTwoFactorAttempted] = useState(false);
  const twoFactorInputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    let active = true;

    void getSetupStatus()
      .then((response) => {
        if (active) {
          setProviders(response.configured_oauth_providers);
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
    if (loginStep === "twoFactor") {
      const timer = window.setTimeout(() => {
        twoFactorInputRef.current?.focus();
      }, 20);

      return () => window.clearTimeout(timer);
    }

    return undefined;
  }, [loginStep]);

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
        setLoginStep("twoFactor");
        setStatus("Credentials accepted. Enter your two-factor code to finish signing in.");
        return;
      }

      setStatus(response.message);
      if (response.session?.authenticated) {
        navigate(
          redirectTarget(location.search) ?? redirectAfterLogin(response.session.user),
          { replace: true },
        );
      }
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Login failed.");
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
      navigate(redirectTarget(location.search) ?? redirectAfterLogin(response.session?.user), {
        replace: true,
      });
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Failed to verify 2FA.");
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
        setStatus("A new two-factor challenge was issued. Enter the latest code.");
        return;
      }

      setStatus(response.message);
      if (response.session?.authenticated) {
        navigate(
          redirectTarget(location.search) ?? redirectAfterLogin(response.session.user),
          { replace: true },
        );
      }
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Failed to refresh the challenge.");
    } finally {
      setIsSubmitting(false);
    }
  };

  const onReturnToCredentials = () => {
    setLoginStep("credentials");
    setTwoFactorChallengeId(null);
    setTwoFactorCode("");
    setTwoFactorAttempted(false);
    setStatus("Adjust your credentials, then sign in again.");
  };

  const loginSteps = [
    {
      label: "Step 1",
      copy: "Enter your username or email and password.",
      state: loginStep === "credentials" ? "active" : "complete",
    },
    {
      label: "Step 2",
      copy: "Only shown if this account has two-factor authentication enabled.",
      state: loginStep === "twoFactor" ? "active" : "idle",
    },
  ] as const;

  return (
    <AuthLayout
      eyebrow="Access"
      title="Sign in to ThunderForge."
      description="Enter your credentials to continue. If this account has two-factor authentication enabled, you'll be asked for a code as a second step."
      aside={
        <div className="grid gap-4">
          <Card>
            <div className="grid gap-3">
              <h2 className="text-lg font-semibold">How sign-in works</h2>
              <p className="text-muted-foreground">
                Login resolves in stages so you only see what's relevant right
                now.
              </p>
              <ol className="grid list-decimal gap-1.5 pl-4 text-sm text-muted-foreground">
                <li>Enter your username or email with your password.</li>
                <li>If required, complete a 6-digit two-factor code.</li>
                <li>You're redirected straight to your account.</li>
              </ol>
            </div>
          </Card>

          <Card>
            <div className="grid gap-3">
              <div className="grid gap-1.5">
                <p className="inline-flex items-center gap-2 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                  <FantasyIcon name="wand" size={16} />
                  Other options
                </p>
                <h2 className="text-lg font-semibold">Other routes</h2>
              </div>
              <div className="grid gap-2">
                <Link to="/register" className="font-medium text-primary hover:underline">
                  Create a local account
                </Link>
                <Link to="/welcome" className="font-medium text-primary hover:underline">
                  Review the welcome hall
                </Link>
              </div>
              <div className="mt-2 inline-flex items-center [&>*:not(:first-child)]:-ml-2">
                <Avatar seed="guild-warden" name="Guild warden" />
                <Avatar seed="map-smith" name="Map smith" />
              </div>
            </div>
          </Card>
        </div>
      }
    >
      <div className="grid gap-6">
        <Card>
          <div className="grid gap-6">
            <div className="grid gap-1.5">
              <p className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                Sign in
              </p>
              <h2 className="text-2xl font-semibold">
                One step at a time.
              </h2>
              <p className="text-muted-foreground">
                Two-factor verification only appears if your account requires
                it, keeping the first step focused and quick.
              </p>
            </div>
            <div className="grid gap-3 sm:grid-cols-2">
              {loginSteps.map((step, index) => (
                <article
                  key={step.label}
                  className={cn(
                    "rounded-lg border border-border p-4",
                    step.state === "active" && "border-primary bg-secondary",
                    step.state === "complete" && "opacity-70",
                  )}
                >
                  <span className="mb-2 inline-flex size-6 items-center justify-center rounded-full bg-muted text-xs font-semibold">
                    {step.state === "complete" ? "✓" : index + 1}
                  </span>
                  <h3 className="font-semibold">{step.label}</h3>
                  <p className="text-sm text-muted-foreground">{step.copy}</p>
                </article>
              ))}
            </div>
          </div>
        </Card>

        <div className="relative grid gap-4">
          <Card
            className={cn(loginStep === "twoFactor" && "opacity-60")}
            data-ambient-sound="guild-hall-candles"
          >
            <form onSubmit={onSubmitCredentials} className="grid gap-4">
              <div className="grid gap-1.5">
                <p className="inline-flex items-center gap-2 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                  <FantasyIcon name="shield" size={16} />
                  Primary login
                </p>
                <h2 className="text-lg font-semibold">Enter your credentials.</h2>
                <p className="text-muted-foreground">
                  Sign in with the email address or username tied to your
                  local ThunderForge account.
                </p>
              </div>

              {loginStep === "twoFactor" ? (
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
                    disabled={isSubmitting || loginStep === "twoFactor"}
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
                      disabled={isSubmitting || loginStep === "twoFactor"}
                    />
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      className="absolute top-1/2 right-1 -translate-y-1/2"
                      onClick={() => setShowPassword((current) => !current)}
                      aria-label={showPassword ? "Hide password" : "Show password"}
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
                  disabled={isSubmitting || loginStep === "twoFactor"}
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
                "border-2",
                currentStatusVariant === "danger" && "border-destructive",
                currentStatusVariant === "success" && "border-emerald-600",
              )}
            >
              <form onSubmit={onSubmitTwoFactor} className="grid gap-4">
                <div className="grid gap-1.5">
                  <p className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                    Two-factor verification
                  </p>
                  <h3 className="text-lg font-semibold">Enter your code.</h3>
                  <p className="text-muted-foreground">
                    Enter the code from your authenticator app to finish
                    signing in.
                  </p>
                </div>

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
                    aria-describedby="login-two-factor-hint"
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

                <p
                  id="login-two-factor-hint"
                  className="text-sm text-muted-foreground"
                >
                  Codes refresh automatically. Only request a new one if this
                  code expires before you can submit it.
                </p>
              </form>
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
