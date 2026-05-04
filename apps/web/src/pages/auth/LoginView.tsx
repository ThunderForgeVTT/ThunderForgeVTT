import type { CSSProperties, FormEvent } from "react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import { getSetupStatus, startOAuthLogin } from "@/api/auth";
import { Avatar } from "@/components/ui/avatar/Avatar";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { Field } from "@/components/ui/field/Field";
import { RuneDivider } from "@/components/ui/rune-divider/RuneDivider";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { AuthLayout } from "@/layouts/auth-layout/AuthLayout";
import { useAuth } from "@/hooks/useAuth";
import type { SetupProvider } from "@/types/auth";
import { cn } from "@/utils/cn";
import { fantasyAuthTheme } from "./fantasy-auth-theme";
import styles from "./LoginView.module.scss";
import ritualStyles from "./TwoFactorPanel.module.scss";

type RitualStep = "credentials" | "twoFactor";
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
  const [ritualStep, setRitualStep] = useState<RitualStep>("credentials");
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
    if (ritualStep === "twoFactor") {
      const timer = window.setTimeout(() => {
        twoFactorInputRef.current?.focus();
      }, 20);

      return () => window.clearTimeout(timer);
    }

    return undefined;
  }, [ritualStep]);

  const markTouched = (...fields: LoginField[]) => {
    setTouched((current) => ({
      ...current,
      ...Object.fromEntries(fields.map((field) => [field, true])),
    }));
  };

  const credentialErrors = {
    identifier: identifier.trim()
      ? undefined
      : "Enter the steward name or email bound to this realm.",
    password: password
      ? undefined
      : "Enter the password seal to begin the sign-in ritual.",
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
      setStatus("The first seal is incomplete. Restore the highlighted fields.");
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
        setRitualStep("twoFactor");
        setStatus("The outer seal opens. Complete the second seal to finalize access.");
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
      setStatus("The second seal is missing. Restart the sign-in ritual.");
      setRitualStep("credentials");
      return;
    }

    if (twoFactorErrors.twoFactorCode) {
      setStatus("The second seal needs a valid 6-digit token.");
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
      setStatus("Return to the first seal and restore your credentials before renewing the challenge.");
      setRitualStep("credentials");
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
        setStatus("A fresh second seal has formed. Enter the latest authenticator code.");
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
      setStatus(error instanceof Error ? error.message : "Failed to refresh the second seal.");
    } finally {
      setIsSubmitting(false);
    }
  };

  const onReturnToCredentials = () => {
    setRitualStep("credentials");
    setTwoFactorChallengeId(null);
    setTwoFactorCode("");
    setTwoFactorAttempted(false);
    setStatus("Adjust your primary credentials, then invoke the second seal again.");
  };

  const ritualSteps = [
    {
      label: "First seal",
      copy: "Offer your identity and password to begin the guild hall access rite.",
      state: ritualStep === "credentials" ? "active" : "complete",
    },
    {
      label: "Second seal",
      copy: "Only summoned when the account is warded with two-factor protection.",
      state: ritualStep === "twoFactor" ? "active" : "idle",
    },
  ] as const;

  return (
    <AuthLayout
      eyebrow="Access"
      title="Enter ThunderForge through a progressive signing ritual."
      description="Credentials open the hall. If this steward is warded with two-factor protection, the second seal appears as the final confirmation instead of crowding the first step."
      aside={
        <div className={styles.aside}>
          <Card surface="parchment">
            <div className={styles.asideCard}>
              <h2>Ritual cadence</h2>
              <p className={styles.asideCopy}>
                The login flow now resolves in stages so attention stays on the
                current decision instead of every possible requirement at once.
              </p>
              <ol className={styles.asideList}>
                <li>Present username or email with the password seal.</li>
                <li>Only then, if required, complete the second seal with TOTP.</li>
                <li>Return straight to the proper welcome hall once the ward accepts you.</li>
              </ol>
            </div>
          </Card>

          <Card surface="parchment">
            <div className={styles.asideCard}>
              <div className={styles.header}>
                <p className={styles.sectionKicker}>
                  <FantasyIcon name="wand" size={16} />
                  Alternate paths
                </p>
                <h2>Other routes</h2>
              </div>
              <div className={styles.linkList}>
                <Link to="/register">Create a local account</Link>
                <Link to="/welcome">Review the welcome hall</Link>
              </div>
              <div className={styles.asideCopy}>
                <div className={styles.actionRow}>
                  <Avatar seed="guild-warden" name="Guild warden" />
                  <Avatar seed="map-smith" name="Map smith" />
                </div>
              </div>
            </div>
          </Card>
        </div>
      }
    >
      <div
        className={styles.layout}
        data-auth-theme="fantasy"
        style={
          {
            "--auth-rune-glow": fantasyAuthTheme.colors.goldBright,
          } as CSSProperties
        }
      >
        <Card surface="stone" className={styles.heroBand}>
          <div className={styles.heroContent}>
            <p className={styles.eyebrow}>Guild hall authentication</p>
            <h2 className={styles.heroTitle}>Advance one seal at a time.</h2>
            <p className={styles.heroCopy}>
              Two-factor verification appears only when the account demands it,
              preserving a calm first step while keeping the warding ceremony
              elegant, readable, and secure.
            </p>
          </div>
          <div className={styles.stepList}>
            {ritualSteps.map((step, index) => (
              <article
                key={step.label}
                className={cn(
                  styles.stepCard,
                  step.state === "active" && styles.stepActive,
                  step.state === "complete" && styles.stepComplete,
                )}
              >
                <span className={styles.stepMarker}>
                  {step.state === "complete" ? "✓" : index + 1}
                </span>
                <h3 className={styles.stepTitle}>{step.label}</h3>
                <p className={styles.stepCopy}>{step.copy}</p>
              </article>
            ))}
          </div>
        </Card>

        <div className={styles.ritualShell}>
          <div className={styles.panelStack}>
            <Card
              surface="parchment"
              className={cn(
                styles.credentialsPanel,
                ritualStep === "twoFactor" && styles.credentialsPanelSealed,
              )}
              data-ambient-sound="guild-hall-candles"
            >
              <form onSubmit={onSubmitCredentials} className={styles.form}>
                <div className={styles.header}>
                  <p className={styles.sectionKicker}>
                    <FantasyIcon name="shield" size={16} />
                    Primary login
                  </p>
                  <h2 className={styles.sectionTitle}>Present your first seal.</h2>
                  <p className={styles.sectionCopy}>
                    Sign in with the email address or username tied to your local
                    ThunderForge account.
                  </p>
                </div>

                {ritualStep === "twoFactor" ? (
                  <div className={styles.credentialsSummary}>
                    <div className={styles.summaryRow}>
                      <span className={styles.summaryPill}>
                        <FantasyIcon name="spark" size={14} />
                        Credentials accepted
                      </span>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className={styles.secondaryButton}
                        onClick={onReturnToCredentials}
                      >
                        Edit first seal
                      </Button>
                    </div>
                    <p>
                      The hall recognizes <strong>{identifier.trim()}</strong>.
                      Finish the second seal to enter.
                    </p>
                  </div>
                ) : null}

                <div className={styles.fieldGrid}>
                  <Field
                    label="Email address or username"
                    htmlFor="login-identifier"
                    accent="Required"
                    error={credentialFieldError("identifier")}
                  >
                    <input
                      id="login-identifier"
                      name="identifier"
                      autoComplete="username"
                      value={identifier}
                      onBlur={() => markTouched("identifier")}
                      onChange={(event) => setIdentifier(event.target.value)}
                      className={styles.input}
                      placeholder="founder@thunderforge.app"
                      disabled={isSubmitting || ritualStep === "twoFactor"}
                    />
                  </Field>

                  <Field
                    label="Password"
                    htmlFor="login-password"
                    accent="Required"
                    error={credentialFieldError("password")}
                  >
                    <div className={styles.passwordWrap}>
                      <input
                        id="login-password"
                        name="password"
                        type={showPassword ? "text" : "password"}
                        autoComplete="current-password"
                        value={password}
                        onBlur={() => markTouched("password")}
                        onChange={(event) => setPassword(event.target.value)}
                        className={cn(styles.input, styles.passwordInput)}
                        placeholder="Enter your password"
                        disabled={isSubmitting || ritualStep === "twoFactor"}
                      />
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className={styles.passwordToggle}
                        onClick={() => setShowPassword((current) => !current)}
                        aria-label={showPassword ? "Hide password" : "Show password"}
                      >
                        {showPassword ? "Hide" : "Show"}
                      </Button>
                    </div>
                  </Field>
                </div>

                <div className={styles.actionRow}>
                  <Button
                    type="submit"
                    variant="primary"
                    size="lg"
                    disabled={isSubmitting || ritualStep === "twoFactor"}
                    icon="shield"
                    className={styles.primaryButton}
                  >
                    {isSubmitting && ritualStep === "credentials"
                      ? "Opening hall..."
                      : "Sign In"}
                  </Button>
                </div>

                {ritualStep === "credentials" && providers.length ? (
                  <div className={styles.oauthSection}>
                    <RuneDivider label="OAuth sign-in" />
                    <div className={styles.oauthList}>
                      {providers.map((provider) => (
                        <Button
                          key={provider.provider_key}
                          type="button"
                          variant="secondary"
                          icon="wand"
                          className={styles.oauthButton}
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

              {ritualStep === "twoFactor" ? (
                <Card
                  surface="parchment"
                  className={cn(
                    ritualStyles.panel,
                    currentStatusVariant === "danger" && ritualStyles.errorState,
                    currentStatusVariant === "success" && ritualStyles.successState,
                  )}
                >
                  <form onSubmit={onSubmitTwoFactor} className={styles.form}>
                    <div className={ritualStyles.header}>
                      <p className={ritualStyles.kicker}>
                        <FantasyIcon name="rune" size={16} />
                        Two-factor ritual
                      </p>
                      <h3 className={ritualStyles.title}>Complete the second seal.</h3>
                      <p className={ritualStyles.copy}>
                        Enter your arcane token to finalize access.
                      </p>
                    </div>

                    <Field
                      label="Arcane token"
                      htmlFor="login-two-factor"
                      accent="Required"
                      error={twoFactorFieldError("twoFactorCode")}
                      hint="Use the latest 6-digit code from your authenticator."
                    >
                      <input
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
                        className={ritualStyles.input}
                        placeholder="123456"
                        aria-describedby="login-two-factor-hint"
                      />
                    </Field>

                    <div className={ritualStyles.actionRow}>
                      <Button
                        type="submit"
                        variant="primary"
                        size="lg"
                        disabled={isSubmitting}
                        icon="spark"
                        className={ritualStyles.verifyButton}
                      >
                        {isSubmitting ? "Verifying seal..." : "Verify"}
                      </Button>
                      <Button
                        type="button"
                        variant="secondary"
                        size="lg"
                        disabled={isSubmitting}
                        className={ritualStyles.utilityButton}
                        onClick={() => void onRefreshSecondSeal()}
                      >
                        Renew challenge
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="lg"
                        disabled={isSubmitting}
                        className={ritualStyles.utilityButton}
                        onClick={onReturnToCredentials}
                      >
                        Back to credentials
                      </Button>
                    </div>

                    <p id="login-two-factor-hint" className={ritualStyles.hint}>
                      Authenticator codes refresh naturally. Renew the challenge
                      only if this seal expires before you can submit the next code.
                    </p>
                  </form>
                </Card>
              ) : null}
            </Card>
          </div>

          <div className={styles.statusRegion} aria-live="polite">
            {status ? (
              <StatusBadge variant={currentStatusVariant} className={styles.statusBadge}>
                {status}
              </StatusBadge>
            ) : null}
          </div>
        </div>
      </div>
    </AuthLayout>
  );
}
