import type { FormEvent } from "react";
import { useState } from "react";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Tabs } from "@/components/ui/tabs/Tabs";
import { Tooltip } from "@/components/ui/tooltip/Tooltip";
import { AuthLayout } from "@/layouts/auth-layout/AuthLayout";
import { setupBasic, startSetupOAuth } from "@/services/auth";
import type { SetupStatus } from "@/types/auth";
import type { SeoConfig } from "@/types/seo";
import { cn } from "@/lib/utils";

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

const STRENGTH_BAR_CLASSES: Record<PasswordStrengthTone, string> = {
  weak: "bg-destructive",
  fair: "bg-amber-500",
  good: "bg-emerald-500",
  strong: "bg-primary",
};

const emailPattern = /\S+@\S+\.\S+/;

function evaluatePasswordStrength(password: string): PasswordStrength {
  let score = 0;
  if (password.length >= 12) score += 1;
  if (/[A-Z]/.test(password) && /[a-z]/.test(password)) score += 1;
  if (/\d/.test(password)) score += 1;
  if (/[^A-Za-z0-9]/.test(password)) score += 1;

  if (password.length === 0) {
    return {
      label: "None",
      score: 0,
      tone: "weak",
      copy: "A long passphrase with mixed character types is harder to break.",
    };
  }

  if (score <= 1) {
    return {
      label: "Weak",
      score,
      tone: "weak",
      copy: "Add more length and variety before using this password.",
    };
  }

  if (score === 2) {
    return {
      label: "Fair",
      score,
      tone: "fair",
      copy: "A serviceable start. Add a number or symbol to strengthen it.",
    };
  }

  if (score === 3) {
    return {
      label: "Good",
      score,
      tone: "good",
      copy: "Solid password. A longer phrase would improve it further.",
    };
  }

  return {
    label: "Strong",
    score,
    tone: "strong",
    copy: "This passphrase has strong entropy and is well suited for the founding account.",
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
        : "Choose a username with at least 3 characters.",
    email:
      emailPattern.test(email.trim())
        ? undefined
        : "Enter a valid email address for recovery and notices.",
    password:
      password.length >= 12
        ? undefined
        : "Use at least 12 characters for a stronger password.",
    passwordConfirmation:
      password === passwordConfirmation
        ? undefined
        : "The confirmation must match the chosen password.",
  } as const;

  const oauthErrors = {
    adminCode: resolvedAdminCode.trim()
      ? undefined
      : "OAuth bootstrap still requires the server's one-time admin code.",
    oauthUsername:
      oauthUsername.trim().length === 0 || oauthUsername.trim().length >= 3
        ? undefined
        : "If you override the provider identity, use at least 3 characters.",
  } as const;

  const setupSteps = [
    {
      label: "Enter the code",
      copy: "Provide the one-time admin code from the server to unlock setup.",
      state: resolvedAdminCode.trim() ? "complete" : "active",
    },
    {
      label: "Choose a method",
      copy: "Select local credentials or a configured OAuth provider.",
      state:
        username.trim() || oauthUsername.trim() || isStartingOAuth
          ? "complete"
          : resolvedAdminCode.trim()
            ? "active"
            : "idle",
    },
    {
      label: "Finish setup",
      copy: "Complete with a secure password or federated identity to open the instance.",
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
      value: "code",
      label: "Code",
      icon: "rune" as const,
      content: (
        <div className="grid gap-3 text-sm text-muted-foreground">
          <p>
            The bootstrap code is single-use. It binds this setup session to
            the server instance before any account can claim it.
          </p>
          <ul className="grid list-disc gap-1.5 pl-4">
            <li>Paste the code exactly as issued by the server.</li>
            <li>Use the local path if no OAuth providers are configured yet.</li>
            <li>Keep the code private until setup is complete.</li>
          </ul>
        </div>
      ),
    },
    {
      value: "local",
      label: "Local",
      icon: "quill" as const,
      content: (
        <div className="grid gap-3 text-sm text-muted-foreground">
          <p>
            Local bootstrap is the fastest route for a fresh deployment. You
            define the username, recovery email, and password in one pass.
          </p>
          <p>
            Recommended when you want direct control before enabling broader
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
        <div className="grid gap-3 text-sm text-muted-foreground">
          <p>
            OAuth bootstrap creates the first administrator from a trusted
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
      setStatus("Setup is incomplete. Correct the marked fields.");
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
      navigate("/admin?bootstrap=complete", { replace: true });
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
      setStatus("OAuth setup cannot start yet. Fix the highlighted fields.");
      return;
    }

    setIsStartingOAuth(providerKey);
    setStatus(`Opening ${displayName}...`);

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
        eyebrow="First-run setup"
        title="Set up your first administrator account."
        description="ThunderForge is awaiting its first administrator. Enter the one-time code from the server, choose the method that fits your instance, and finish with secure credentials or a configured OAuth provider."
        aside={
          <div className="grid gap-4">
            <Card surface="parchment" className="p-6">
              <div className="grid gap-2">
                <p className="flex items-center gap-2 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                  <FantasyIcon name="spells" size={16} />
                  Setup steps
                </p>
                <h2 className="text-lg font-semibold">Getting started</h2>
                <p className="text-sm text-muted-foreground">
                  The setup flow is brief, but every step should feel clear.
                </p>
              </div>
              <ol className="mt-4 grid list-decimal gap-2 pl-4 text-sm text-muted-foreground">
                <li>Claim the bootstrap code from the server logs or startup output.</li>
                <li>Choose your path: local credentials or a configured OAuth provider.</li>
                <li>Finish setup and enter the admin welcome page as the first administrator.</li>
              </ol>
              <p className="mt-4 text-sm text-muted-foreground">
                {hasConfiguredProviders
                  ? `${setupStatus.configured_oauth_providers.length} OAuth provider${setupStatus.configured_oauth_providers.length === 1 ? "" : "s"} available.`
                  : "No OAuth providers are configured yet, so the local path is the safest option."}
              </p>
            </Card>

            <Card surface="stone" className="p-6">
              <div className="grid gap-2">
                <p className="flex items-center gap-2 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                  <FantasyIcon name="rune" size={16} />
                  Reference
                </p>
                <h2 className="text-lg font-semibold">Quick reference</h2>
              </div>
              <Tabs items={guideTabs} defaultValue="code" className="mt-4" />
            </Card>
          </div>
        }
      >
        <div className="grid gap-6">
          <Card surface="stone" className="p-6">
            <div className="flex flex-wrap items-start justify-between gap-6">
              <div className="grid max-w-xl gap-2">
                <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                  Instance bootstrap
                </p>
                <h2 className="text-2xl font-semibold">
                  Choose the setup method for this instance.
                </h2>
                <p className="text-muted-foreground">
                  Local credentials favor direct control. OAuth bootstrap lets
                  a trusted identity step through an already configured
                  provider. Both paths use the same server-approved first-run
                  flow.
                </p>
              </div>
              <div className="grid justify-items-end gap-1 text-right">
                <span className="text-xs text-muted-foreground">
                  First administrator
                </span>
                <strong>
                  {hasConfiguredProviders ? "Two paths available" : "Local setup ready"}
                </strong>
                <small className="text-xs text-muted-foreground">
                  {hasConfiguredProviders
                    ? `${setupStatus.configured_oauth_providers.length} configured providers`
                    : "Awaiting first setup"}
                </small>
              </div>
            </div>

            <div className="mt-6 grid gap-3 sm:grid-cols-3">
              {setupSteps.map((step, index) => (
                <article
                  key={step.label}
                  className={cn(
                    "rounded-lg border border-border p-4",
                    step.state === "complete" && "border-primary/40 bg-primary/5",
                    step.state === "active" && "border-ring",
                  )}
                >
                  <span className="mb-2 inline-flex size-6 items-center justify-center rounded-full bg-muted text-xs font-semibold">
                    {step.state === "complete" ? "✓" : index + 1}
                  </span>
                  <h3 className="font-semibold">{step.label}</h3>
                  <p className="mt-1 text-sm text-muted-foreground">
                    {step.copy}
                  </p>
                </article>
              ))}
            </div>
          </Card>

          <div className="grid gap-6 lg:grid-cols-2">
            <Card surface="leather" className="p-6">
              <form onSubmit={onSubmitBasic} className="grid gap-6">
                <header className="grid gap-2">
                  <div className="flex items-center justify-between gap-2">
                    <p className="flex items-center gap-2 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                      <FantasyIcon name="quill" size={16} />
                      Local setup
                    </p>
                    <Tooltip content="Use a long passphrase and matching recovery email so the first administrator account is easy to preserve and hard to compromise.">
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        icon="spark"
                        aria-label="Password guidance"
                      >
                        <span className="sr-only">Password guidance</span>
                      </Button>
                    </Tooltip>
                  </div>
                  <h3 className="text-lg font-semibold">
                    Create administrator account
                  </h3>
                  <p className="text-sm text-muted-foreground">
                    Set up the first local administrator with a username,
                    recovery email, and a strong password.
                  </p>
                  <div className="flex flex-wrap gap-2">
                    <span className="rounded-full bg-muted px-3 py-1 text-xs text-muted-foreground">
                      Immediate control
                    </span>
                    <span className="rounded-full bg-muted px-3 py-1 text-xs text-muted-foreground">
                      Recovery ready
                    </span>
                    <span className="rounded-full bg-muted px-3 py-1 text-xs text-muted-foreground">
                      Password strength check
                    </span>
                  </div>
                </header>

                <div className="grid gap-4">
                  <Field
                    label="Bootstrap admin code"
                    htmlFor="setup-admin-code"
                    accent="Required"
                    error={localFieldError("adminCode")}
                    hint="Use the one-time code emitted by the server."
                  >
                    <Input
                      id="setup-admin-code"
                      name="adminCode"
                      autoComplete="one-time-code"
                      value={resolvedAdminCode}
                      onBlur={() => markTouched("adminCode")}
                      onChange={(event) => setAdminCode(event.target.value)}
                      placeholder="ABCD-EFGH-JKLM"
                    />
                  </Field>

                  <Field
                    label="Username"
                    htmlFor="setup-username"
                    accent="Required"
                    error={localFieldError("username")}
                    hint="This name appears throughout the app and future worlds."
                  >
                    <Input
                      id="setup-username"
                      name="username"
                      autoComplete="username"
                      value={username}
                      onBlur={() => markTouched("username")}
                      onChange={(event) => setUsername(event.target.value)}
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
                    <Input
                      id="setup-email"
                      name="email"
                      type="email"
                      autoComplete="email"
                      value={email}
                      onBlur={() => markTouched("email")}
                      onChange={(event) => setEmail(event.target.value)}
                      placeholder="admin@example.com"
                    />
                  </Field>

                  <Field
                    label="Password"
                    htmlFor="setup-password"
                    accent="Required"
                    error={localFieldError("password")}
                  >
                    <Input
                      id="setup-password"
                      name="password"
                      type="password"
                      autoComplete="new-password"
                      value={password}
                      onBlur={() => markTouched("password")}
                      onChange={(event) => setPassword(event.target.value)}
                      placeholder="Create a strong password"
                    />
                  </Field>

                  <div className="grid gap-2 rounded-lg border border-border p-3">
                    <div className="flex items-center justify-between text-sm">
                      <span className="text-muted-foreground">
                        Password strength
                      </span>
                      <strong>{passwordStrength.label}</strong>
                    </div>
                    <div
                      className="h-1.5 w-full overflow-hidden rounded-full bg-muted"
                      aria-hidden="true"
                    >
                      <div
                        className={cn(
                          "h-full rounded-full transition-all",
                          STRENGTH_BAR_CLASSES[passwordStrength.tone],
                        )}
                        style={{ width: `${Math.max(passwordStrength.score, 0) * 25}%` }}
                      />
                    </div>
                    <p className="text-xs text-muted-foreground">
                      {passwordStrength.copy}
                    </p>
                  </div>

                  <Field
                    label="Confirm password"
                    htmlFor="setup-password-confirmation"
                    accent="Required"
                    error={localFieldError("passwordConfirmation")}
                    hint="The confirmation must match before setup can finish."
                  >
                    <Input
                      id="setup-password-confirmation"
                      name="passwordConfirmation"
                      type="password"
                      autoComplete="new-password"
                      value={passwordConfirmation}
                      onBlur={() => markTouched("passwordConfirmation")}
                      onChange={(event) =>
                        setPasswordConfirmation(event.target.value)
                      }
                      placeholder="Confirm the password"
                    />
                  </Field>
                </div>

                <Button
                  type="submit"
                  variant="primary"
                  size="lg"
                  fullWidth
                  disabled={isSubmitting}
                  icon="shield"
                >
                  {isSubmitting
                    ? "Creating administrator..."
                    : "Create Administrator Account"}
                </Button>

                <footer className="border-t border-border pt-4 text-sm text-muted-foreground">
                  <p>
                    The local method stores credentials on this instance and
                    opens the admin welcome page immediately after a
                    successful setup.
                  </p>
                </footer>
              </form>
            </Card>

            <Card surface="parchment" className="p-6">
              <div className="grid gap-6">
                <header className="grid gap-2">
                  <p className="flex items-center gap-2 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                    <FantasyIcon name="wand" size={16} />
                    Federated bootstrap
                  </p>
                  <h3 className="text-lg font-semibold">Use OAuth Bootstrap</h3>
                  <p className="text-sm text-muted-foreground">
                    Invite a configured provider to create the first
                    administrator from a trusted external identity.
                  </p>
                  <div className="flex flex-wrap gap-2">
                    <span className="rounded-full bg-muted px-3 py-1 text-xs text-muted-foreground">
                      Federated identity
                    </span>
                    <span className="rounded-full bg-muted px-3 py-1 text-xs text-muted-foreground">
                      Reduced password handling
                    </span>
                    <span className="rounded-full bg-muted px-3 py-1 text-xs text-muted-foreground">
                      Guided redirect flow
                    </span>
                  </div>
                </header>

                <div className="grid gap-4">
                  <Field
                    label="Bootstrap admin code"
                    htmlFor="setup-oauth-code"
                    accent="Required"
                    error={oauthFieldError("adminCode")}
                    hint="The same one-time code also governs OAuth bootstrap."
                  >
                    <Input
                      id="setup-oauth-code"
                      name="oauthAdminCode"
                      autoComplete="one-time-code"
                      value={resolvedAdminCode}
                      onBlur={() => markTouched("adminCode")}
                      onChange={(event) => setAdminCode(event.target.value)}
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
                    <Input
                      id="setup-oauth-username"
                      name="oauthUsername"
                      autoComplete="username"
                      value={oauthUsername}
                      onBlur={() => markTouched("oauthUsername")}
                      onChange={(event) => setOauthUsername(event.target.value)}
                      placeholder="Optional username override"
                    />
                  </Field>
                </div>

                <div className="grid gap-3">
                  {hasConfiguredProviders ? (
                    setupStatus.configured_oauth_providers.map((provider) => (
                      <Button
                        key={provider.provider_key}
                        variant="secondary"
                        icon="wand"
                        fullWidth
                        disabled={Boolean(isStartingOAuth)}
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
                    <StatusBadge variant="warning">
                      No OAuth providers are configured yet.
                    </StatusBadge>
                  )}
                </div>

                <footer className="border-t border-border pt-4 text-sm text-muted-foreground">
                  <p>
                    Recommended when your instance already trusts an external
                    identity provider and you want the founding administrator
                    to begin with federation.
                  </p>
                </footer>
              </div>
            </Card>
          </div>

          {resolvedStatus ? (
            <StatusBadge variant={statusVariant(resolvedStatus)}>
              {resolvedStatus}
            </StatusBadge>
          ) : null}
        </div>
      </AuthLayout>
    </>
  );
}
