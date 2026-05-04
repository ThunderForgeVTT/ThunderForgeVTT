import type { CSSProperties, FormEvent } from "react";
import { useState } from "react";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Field } from "@/components/ui/field/Field";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Tabs } from "@/components/ui/tabs/Tabs";
import { Tooltip } from "@/components/ui/tooltip/Tooltip";
import { AuthLayout } from "@/layouts/auth-layout/AuthLayout";
import { setupBasic, startSetupOAuth } from "@/services/auth";
import type { SetupStatus } from "@/types/auth";
import type { SeoConfig } from "@/types/seo";
import { cn } from "@/utils/cn";
import { fantasyTheme } from "./fantasy-theme";
import buttonStyles from "./SetupButton.module.scss";
import inputStyles from "./SetupInput.module.scss";
import panelStyles from "./SetupPanel.module.scss";
import styles from "./SetupPage.module.scss";

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
  prefetchHrefs: ["/setup/callback", "/admin/welcome"],
};

interface SetupPageProps {
  setupStatus: SetupStatus;
  onSetupComplete: () => Promise<unknown> | unknown;
}

type SetupFieldName =
  | "adminCode"
  | "username"
  | "email"
  | "password"
  | "passwordConfirmation"
  | "oauthUsername";

type PasswordStrengthTone = "weak" | "fair" | "good" | "strong";

interface PasswordStrength {
  label: string;
  score: number;
  tone: PasswordStrengthTone;
  copy: string;
}

const emailPattern = /\S+@\S+\.\S+/;

function evaluatePasswordStrength(password: string): PasswordStrength {
  let score = 0;
  if (password.length >= 12) score += 1;
  if (/[A-Z]/.test(password) && /[a-z]/.test(password)) score += 1;
  if (/\d/.test(password)) score += 1;
  if (/[^A-Za-z0-9]/.test(password)) score += 1;

  if (password.length === 0) {
    return {
      label: "Dormant",
      score: 0,
      tone: "weak",
      copy: "A long passphrase with mixed character types makes the seal harder to break.",
    };
  }

  if (score <= 1) {
    return {
      label: "Faint",
      score,
      tone: "weak",
      copy: "Add more length and variety before entrusting the guild hall to this password.",
    };
  }

  if (score === 2) {
    return {
      label: "Steady",
      score,
      tone: "fair",
      copy: "A serviceable start. Add a number or symbol to reinforce the ward.",
    };
  }

  if (score === 3) {
    return {
      label: "Warded",
      score,
      tone: "good",
      copy: "The protective sigils are taking hold. A longer phrase still improves resilience.",
    };
  }

  return {
    label: "Mythic",
    score,
    tone: "strong",
    copy: "This passphrase carries strong entropy and fits the founding ritual well.",
  };
}

function statusVariant(message: string | null) {
  if (!message) {
    return "info" as const;
  }

  const normalized = message.toLowerCase();
  if (
    normalized.includes("fail") ||
    normalized.includes("error") ||
    normalized.includes("mismatch")
  ) {
    return "danger" as const;
  }

  if (
    normalized.includes("success") ||
    normalized.includes("complete") ||
    normalized.includes("created")
  ) {
    return "success" as const;
  }

  if (normalized.includes("warn") || normalized.includes("missing")) {
    return "warning" as const;
  }

  return "info" as const;
}

export default function SetupPage({
  setupStatus,
  onSetupComplete,
}: SetupPageProps) {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const { code } = useParams();
  const [adminCode, setAdminCode] = useState("");
  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [passwordConfirmation, setPasswordConfirmation] = useState("");
  const [oauthUsername, setOauthUsername] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [touched, setTouched] = useState<Partial<Record<SetupFieldName, boolean>>>(
    {},
  );
  const [localAttempted, setLocalAttempted] = useState(false);
  const [oauthAttempted, setOAuthAttempted] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isStartingOAuth, setIsStartingOAuth] = useState<string | null>(null);

  const oauthError = searchParams.get("oauth_error");
  const resolvedAdminCode = adminCode || code || "";
  const resolvedStatus = status ?? oauthError;
  const passwordStrength = evaluatePasswordStrength(password);
  const hasConfiguredProviders = setupStatus.configured_oauth_providers.length > 0;

  const localErrors = {
    adminCode: resolvedAdminCode.trim()
      ? undefined
      : "Enter the one-time admin code issued by the server.",
    username:
      username.trim().length >= 3
        ? undefined
        : "Choose a steward name with at least 3 characters.",
    email:
      emailPattern.test(email.trim())
        ? undefined
        : "Enter a valid email address for recovery and notices.",
    password:
      password.length >= 12
        ? undefined
        : "Use at least 12 characters to strengthen the ward.",
    passwordConfirmation:
      password === passwordConfirmation
        ? undefined
        : "The confirmation must match the chosen password.",
  } as const;

  const oauthErrors = {
    adminCode: resolvedAdminCode.trim()
      ? undefined
      : "The OAuth bootstrap still requires the server's one-time admin code.",
    oauthUsername:
      oauthUsername.trim().length === 0 || oauthUsername.trim().length >= 3
        ? undefined
        : "If you override the provider identity, use at least 3 characters.",
  } as const;

  const ritualSteps = [
    {
      label: "Receive the seal",
      copy: "Present the one-time admin code from the server to unlock founding access.",
      state: resolvedAdminCode.trim() ? "complete" : "active",
    },
    {
      label: "Choose the rite",
      copy: "Select local credentials or summon a configured OAuth envoy.",
      state:
        username.trim() || oauthUsername.trim() || isStartingOAuth
          ? "complete"
          : resolvedAdminCode.trim()
            ? "active"
            : "idle",
    },
    {
      label: "Consecrate the steward",
      copy: "Finish with a secure password or federated identity and open the realm.",
      state:
        passwordStrength.score >= 3 &&
        !localErrors.email &&
        !localErrors.passwordConfirmation
          ? "complete"
          : username.trim() || oauthUsername.trim()
            ? "active"
            : "idle",
    },
  ] as const;

  const guideTabs = [
    {
      value: "seal",
      label: "Seal",
      icon: "rune" as const,
      content: (
        <div className={styles.guideContent}>
          <p>
            The bootstrap code is your one-use sigil. It binds this founding
            session to the server instance before any account can claim it.
          </p>
          <ul className={panelStyles.guideList}>
            <li>Paste the code exactly as issued by the server.</li>
            <li>Use the local path if no OAuth envoys are configured yet.</li>
            <li>Keep the code private until the ritual is complete.</li>
          </ul>
        </div>
      ),
    },
    {
      value: "local",
      label: "Local",
      icon: "quill" as const,
      content: (
        <div className={styles.guideContent}>
          <p>
            Local bootstrap is the fastest route for a fresh deployment. You
            define the username, recovery email, and password seal in one pass.
          </p>
          <p>
            Recommended when you want direct stewardship before enabling broader
            federation.
          </p>
        </div>
      ),
    },
    {
      value: "oauth",
      label: "OAuth",
      icon: "wand" as const,
      content: (
        <div className={styles.guideContent}>
          <p>
            OAuth bootstrap consecrates the first administrator from a trusted
            provider already configured on the instance.
          </p>
          <p>
            Use an override username only when you want a local display name
            that differs from the upstream identity.
          </p>
        </div>
      ),
    },
  ];

  const localPanelStyle = {
    "--setup-panel-accent": fantasyTheme.colors.gold,
  } as CSSProperties;

  const oauthPanelStyle = {
    "--setup-panel-accent": fantasyTheme.colors.violet,
  } as CSSProperties;

  const markTouched = (...fields: SetupFieldName[]) => {
    setTouched((current) => ({
      ...current,
      ...Object.fromEntries(fields.map((field) => [field, true])),
    }));
  };

  const onSubmitBasic = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setLocalAttempted(true);
    markTouched(
      "adminCode",
      "username",
      "email",
      "password",
      "passwordConfirmation",
    );

    if (
      localErrors.adminCode ||
      localErrors.username ||
      localErrors.email ||
      localErrors.password ||
      localErrors.passwordConfirmation
    ) {
      setStatus("The founding ritual is incomplete. Correct the marked sigils.");
      return;
    }

    if (password !== passwordConfirmation) {
      setStatus("Passwords do not match.");
      return;
    }

    setIsSubmitting(true);
    setStatus(null);

    try {
      const result = await setupBasic(
        resolvedAdminCode,
        username,
        email,
        password,
      );
      setStatus(result);
      await onSetupComplete();
      navigate("/admin/welcome?bootstrap=complete", { replace: true });
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Setup failed.");
    } finally {
      setIsSubmitting(false);
    }
  };

  const onStartOAuth = async (providerKey: string, displayName: string) => {
    setOAuthAttempted(true);
    markTouched("adminCode", "oauthUsername");

    if (oauthErrors.adminCode || oauthErrors.oauthUsername) {
      setStatus("The envoy gate cannot open yet. Restore the highlighted fields.");
      return;
    }

    setIsStartingOAuth(providerKey);
    setStatus(`Opening the envoy gate for ${displayName}...`);

    try {
      await startSetupOAuth(
        providerKey,
        resolvedAdminCode,
        oauthUsername || username,
      );
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "OAuth setup failed.");
      setIsStartingOAuth(null);
    }
  };

  const localFieldError = (field: keyof typeof localErrors) =>
    localAttempted || touched[field] ? localErrors[field] : undefined;

  const oauthFieldError = (field: keyof typeof oauthErrors) =>
    oauthAttempted || touched[field] ? oauthErrors[field] : undefined;

  return (
    <>
      <SEO {...setupPageSeo} />
      <AuthLayout
        eyebrow="First-run ritual"
        title="Open the guild tome and forge your first administrator."
        description="ThunderForge is awaiting its founding steward. Present the one-time seal from the server, choose the rite that fits your instance, and complete the bootstrap with secure credentials or a sanctioned OAuth envoy."
        aside={
          <div className={styles.asideStack}>
            <Card surface="parchment" className={cn(panelStyles.panel, panelStyles.asidePanel)}>
              <div className={styles.asideHeading}>
                <p className={panelStyles.sectionKicker}>
                  <FantasyIcon name="spells" size={16} />
                  Ritual steps
                </p>
                <h2 className={panelStyles.sectionTitle}>Founding guidance</h2>
                <p className={styles.asideCopy}>
                  The setup rite is brief, but every step should feel deliberate
                  and clear.
                </p>
              </div>
              <ol className={styles.stepGuide}>
                <li>Claim the bootstrap seal from the server logs or startup output.</li>
                <li>Choose your path: local credentials or a configured OAuth envoy.</li>
                <li>Complete the rite and enter the admin welcome hall as the first steward.</li>
              </ol>
              <p className={styles.ritualNote}>
                {hasConfiguredProviders
                  ? `${setupStatus.configured_oauth_providers.length} OAuth envoy${setupStatus.configured_oauth_providers.length === 1 ? "" : "s"} stand ready for federation.`
                  : "No OAuth envoys are configured yet, so the local rite remains the safest path."}
              </p>
            </Card>

            <Card surface="stone" className={cn(panelStyles.panel, panelStyles.asidePanel)}>
              <div className={styles.asideHeading}>
                <p className={panelStyles.sectionKicker}>
                  <FantasyIcon name="rune" size={16} />
                  Ritual atlas
                </p>
                <h2 className={panelStyles.sectionTitle}>Quick reference</h2>
              </div>
              <Tabs items={guideTabs} defaultValue="seal" className={styles.guideTabs} />
            </Card>
          </div>
        }
      >
        <div className={styles.layout} data-setup-theme="fantasy">
          <Card
            surface="stone"
            className={styles.headerBand}
            data-ambient-sound={fantasyTheme.audioHooks.setupAmbience}
          >
            <div className={styles.headerTop}>
              <div className={styles.heroHeader}>
                <p className={styles.heroEyebrow}>Guild hall bootstrap</p>
                <h2 className={styles.heroTitle}>
                  Choose the bootstrap rite that will awaken your command tome.
                </h2>
                <p className={styles.heroCopy}>
                  Local credentials favor direct stewardship. OAuth bootstrap
                  lets a trusted identity step through an already configured
                  envoy gate. Both paths preserve the same server-approved
                  first-run flow.
                </p>
              </div>
              <div className={styles.heroSeal}>
                <span>First steward</span>
                <strong>
                  {hasConfiguredProviders ? "Two paths open" : "Local rite ready"}
                </strong>
                <small>
                  {hasConfiguredProviders
                    ? `${setupStatus.configured_oauth_providers.length} configured envoys`
                    : "Awaiting first consecration"}
                </small>
              </div>
            </div>

            <div className={styles.stepList}>
              {ritualSteps.map((step, index) => (
                <article
                  key={step.label}
                  className={cn(
                    styles.step,
                    step.state === "complete" && styles.stepComplete,
                    step.state === "active" && styles.stepActive,
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

          <div className={styles.pathGrid}>
            <Card
              surface="leather"
              className={cn(panelStyles.panel, panelStyles.localPanel)}
              style={localPanelStyle}
            >
              <form onSubmit={onSubmitBasic} className={styles.form}>
                <header className={panelStyles.sectionHeader}>
                  <p className={panelStyles.sectionKicker}>
                    <FantasyIcon name="quill" size={16} />
                    Local stewardship
                  </p>
                  <div className={styles.titleRow}>
                    <h3 className={panelStyles.sectionTitle}>
                      Create administrator account
                    </h3>
                    <Tooltip content="Use a long passphrase and matching recovery email so the first steward account is easy to preserve and hard to compromise.">
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className={buttonStyles.helpButton}
                        icon="spark"
                        aria-label="Password guidance"
                      >
                        <span className="sr-only">Password guidance</span>
                      </Button>
                    </Tooltip>
                  </div>
                  <p className={panelStyles.sectionSubtitle}>
                    Establish the first local steward with a readable identity,
                    recovery email, and a resilient password seal.
                  </p>
                  <div className={panelStyles.pillRow}>
                    <span className={panelStyles.pill}>Immediate control</span>
                    <span className={panelStyles.pill}>Recovery ready</span>
                    <span className={panelStyles.pill}>Password warding</span>
                  </div>
                </header>

                <div className={styles.fieldGrid}>
                  <Field
                    label="Bootstrap admin code"
                    htmlFor="setup-admin-code"
                    accent="Required"
                    error={localFieldError("adminCode")}
                    hint="Use the one-time seal emitted by the server."
                  >
                    <input
                      id="setup-admin-code"
                      name="adminCode"
                      autoComplete="one-time-code"
                      value={resolvedAdminCode}
                      onBlur={() => markTouched("adminCode")}
                      onChange={(event) => setAdminCode(event.target.value)}
                      className={inputStyles.input}
                      placeholder="ABCD-EFGH-JKLM"
                    />
                  </Field>

                  <Field
                    label="Username"
                    htmlFor="setup-username"
                    accent="Required"
                    error={localFieldError("username")}
                    hint="This name appears in the hall and future worlds."
                  >
                    <input
                      id="setup-username"
                      name="username"
                      autoComplete="username"
                      value={username}
                      onBlur={() => markTouched("username")}
                      onChange={(event) => setUsername(event.target.value)}
                      className={inputStyles.input}
                      placeholder="founder"
                    />
                  </Field>

                  <Field
                    label="Email"
                    htmlFor="setup-email"
                    accent="Required"
                    error={localFieldError("email")}
                    hint="Used for recovery, notices, and future account flows."
                  >
                    <input
                      id="setup-email"
                      name="email"
                      type="email"
                      autoComplete="email"
                      value={email}
                      onBlur={() => markTouched("email")}
                      onChange={(event) => setEmail(event.target.value)}
                      className={inputStyles.input}
                      placeholder="admin@example.com"
                    />
                  </Field>

                  <Field
                    label="Password"
                    htmlFor="setup-password"
                    accent="Required"
                    error={localFieldError("password")}
                  >
                    <input
                      id="setup-password"
                      name="password"
                      type="password"
                      autoComplete="new-password"
                      value={password}
                      onBlur={() => markTouched("password")}
                      onChange={(event) => setPassword(event.target.value)}
                      className={inputStyles.input}
                      placeholder="Create a strong password"
                    />
                  </Field>

                  <div className={cn(inputStyles.strength, inputStyles[passwordStrength.tone])}>
                    <div className={inputStyles.strengthHeader}>
                      <span>Password strength</span>
                      <strong>{passwordStrength.label}</strong>
                    </div>
                    <div className={inputStyles.strengthTrack} aria-hidden="true">
                      <div
                        className={inputStyles.strengthFill}
                        style={{ width: `${Math.max(passwordStrength.score, 0) * 25}%` }}
                      />
                    </div>
                    <p className={inputStyles.strengthCopy}>{passwordStrength.copy}</p>
                  </div>

                  <Field
                    label="Confirm password"
                    htmlFor="setup-password-confirmation"
                    accent="Required"
                    error={localFieldError("passwordConfirmation")}
                    hint="The confirmation must match before the rite can finish."
                  >
                    <input
                      id="setup-password-confirmation"
                      name="passwordConfirmation"
                      type="password"
                      autoComplete="new-password"
                      value={passwordConfirmation}
                      onBlur={() => markTouched("passwordConfirmation")}
                      onChange={(event) =>
                        setPasswordConfirmation(event.target.value)
                      }
                      className={inputStyles.input}
                      placeholder="Confirm the password"
                    />
                  </Field>
                </div>

                <div className={styles.actions}>
                  <Button
                    type="submit"
                    variant="primary"
                    size="lg"
                    fullWidth
                    disabled={isSubmitting}
                    icon="shield"
                    className={buttonStyles.primaryAction}
                  >
                    {isSubmitting
                      ? "Consecrating administrator..."
                      : "Create Administrator Account"}
                  </Button>
                </div>

                <footer className={panelStyles.panelFooter}>
                  <p>
                    The local rite stores credentials on this instance and opens
                    the admin welcome hall immediately after a successful consecration.
                  </p>
                </footer>
              </form>
            </Card>

            <Card
              surface="parchment"
              className={cn(panelStyles.panel, panelStyles.oauthPanel)}
              style={oauthPanelStyle}
            >
              <div className={styles.form}>
                <header className={panelStyles.sectionHeader}>
                  <p className={panelStyles.sectionKicker}>
                    <FantasyIcon name="wand" size={16} />
                    Federated bootstrap
                  </p>
                  <h3 className={panelStyles.sectionTitle}>Use OAuth Bootstrap</h3>
                  <p className={cn(panelStyles.sectionSubtitle, styles.lightDescription)}>
                    Invite a configured provider to consecrate the first steward
                    from a trusted external identity.
                  </p>
                  <div className={panelStyles.pillRow}>
                    <span className={panelStyles.pill}>Federated identity</span>
                    <span className={panelStyles.pill}>Reduced password handling</span>
                    <span className={panelStyles.pill}>Guided redirect flow</span>
                  </div>
                </header>

                <div className={styles.fieldGrid}>
                  <Field
                    label="Bootstrap admin code"
                    htmlFor="setup-oauth-code"
                    accent="Required"
                    error={oauthFieldError("adminCode")}
                    hint="The same one-time seal also governs OAuth bootstrap."
                  >
                    <input
                      id="setup-oauth-code"
                      name="oauthAdminCode"
                      autoComplete="one-time-code"
                      value={resolvedAdminCode}
                      onBlur={() => markTouched("adminCode")}
                      onChange={(event) => setAdminCode(event.target.value)}
                      className={inputStyles.input}
                      placeholder="ABCD-EFGH-JKLM"
                    />
                  </Field>

                  <Field
                    label="Preferred username"
                    htmlFor="setup-oauth-username"
                    accent="Optional"
                    error={oauthFieldError("oauthUsername")}
                    hint="Leave blank to inherit the provider's preferred identity."
                  >
                    <input
                      id="setup-oauth-username"
                      name="oauthUsername"
                      autoComplete="username"
                      value={oauthUsername}
                      onBlur={() => markTouched("oauthUsername")}
                      onChange={(event) => setOauthUsername(event.target.value)}
                      className={inputStyles.input}
                      placeholder="Optional username override"
                    />
                  </Field>
                </div>

                <div className={styles.providerList}>
                  {hasConfiguredProviders ? (
                    setupStatus.configured_oauth_providers.map((provider) => (
                      <Button
                        key={provider.provider_key}
                        variant="secondary"
                        icon="wand"
                        fullWidth
                        disabled={Boolean(isStartingOAuth)}
                        className={buttonStyles.providerAction}
                        onClick={() =>
                          void onStartOAuth(
                            provider.provider_key,
                            provider.display_name,
                          )
                        }
                      >
                        {isStartingOAuth === provider.provider_key
                          ? `Opening ${provider.display_name}...`
                          : `Continue with ${provider.display_name}`}
                      </Button>
                    ))
                  ) : (
                    <StatusBadge variant="warning" className={styles.oauthStatus}>
                      No OAuth providers are configured yet.
                    </StatusBadge>
                  )}
                </div>

                <footer className={panelStyles.panelFooter}>
                  <p>
                    Recommended when your instance already trusts an external
                    identity provider and you want the founding steward to begin
                    with federation.
                  </p>
                </footer>
              </div>
            </Card>
          </div>

          {resolvedStatus ? (
            <StatusBadge
              variant={statusVariant(resolvedStatus)}
              className={styles.statusBanner}
            >
              {resolvedStatus}
            </StatusBadge>
          ) : null}
        </div>
      </AuthLayout>
    </>
  );
}
