import type { FormEvent } from "react";
import { useState } from "react";
import { setupBasic, startSetupOAuth } from "@/services/auth";
import { Button } from "@/components/ui/button/Button";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type { SetupProvider } from "@/types/auth";
import { cn } from "@/lib/utils";

/**
 * The first administrator (FR-002).
 *
 * This is the one step of the wizard that is **not** derived from the
 * registry, because a user row is not a setting and no declaration could ever
 * produce it. It is also why the wizard cannot be a pure loop over
 * `required_settings`.
 *
 * Both routes that existed before this refactor are kept, unchanged in
 * behaviour: local credentials, and bootstrap through an already configured
 * OAuth provider. What changed is that neither of them is the end of setup any
 * more — creating the account leaves the operator on the next step rather than
 * navigating to `/admin`.
 *
 * # The password is handed upward
 *
 * `onCreated` carries the password out of this component because the second
 * factor step needs it: `beginTwoFactorEnrolment` authorises with
 * `{ username, password }` (see `TwoFactorEnrolmentCredentials` in
 * `@/api/twoFactor`, whose own comment names first-run setup as one of the two
 * entrances that uses that shape). Asking for the password a second time one
 * step later would be a worse trade than holding it for the length of one
 * pass, and the second factor step drops it the moment enrolment ends.
 *
 * The OAuth route cannot hand one up, because there isn't one. See
 * `SecondFactorStep` for what happens then.
 */
export interface AccountStepProps {
  providers: SetupProvider[];
  adminCode: string;
  created: boolean;
  onCreated: (credentials: { username: string; password: string }) => void;
}

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

export function evaluatePasswordStrength(password: string): PasswordStrength {
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

export function AccountStep({
  providers,
  adminCode,
  created,
  onCreated,
}: AccountStepProps) {
  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [passwordConfirmation, setPasswordConfirmation] = useState("");
  const [oauthUsername, setOauthUsername] = useState("");
  const [attempted, setAttempted] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isStartingOAuth, setIsStartingOAuth] = useState<string | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  const strength = evaluatePasswordStrength(password);

  const errors = {
    username:
      username.trim().length >= 3
        ? undefined
        : "Choose a username with at least 3 characters.",
    email: emailPattern.test(email.trim())
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

  const errorFor = (field: keyof typeof errors) =>
    attempted ? errors[field] : undefined;

  if (created) {
    return (
      <StatusBadge variant="success">
        This instance has its first administrator, and you are signed in as
        them.
      </StatusBadge>
    );
  }

  const onSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setAttempted(true);
    setFailure(null);

    if (!adminCode.trim()) {
      setFailure("Enter the one-time admin code issued by the server.");
      return;
    }

    if (Object.values(errors).some(Boolean)) {
      return;
    }

    setIsSubmitting(true);
    try {
      await setupBasic(adminCode, username.trim(), email.trim(), password);
      onCreated({ username: username.trim(), password });
    } catch (error) {
      setFailure(
        error instanceof Error
          ? error.message
          : "The administrator account could not be created.",
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  const onStartOAuth = async (providerKey: string, displayName: string) => {
    if (!adminCode.trim()) {
      setFailure("OAuth bootstrap still requires the server's admin code.");
      return;
    }

    setIsStartingOAuth(providerKey);
    setFailure(null);

    try {
      await startSetupOAuth(providerKey, adminCode, oauthUsername || username);
    } catch (error) {
      setFailure(
        error instanceof Error
          ? error.message
          : `${displayName} could not be reached.`,
      );
      setIsStartingOAuth(null);
    }
  };

  return (
    <div className="grid gap-6">
      <p className="text-sm text-muted-foreground">
        This account owns the instance. It is the only one that exists until you
        invite somebody, and it is the account the rest of setup is performed
        as.
      </p>

      <form onSubmit={onSubmit} className="grid gap-4">
        <Field
          label="Username"
          htmlFor="setup-account-username"
          accent="Required"
          error={errorFor("username")}
          hint="This name appears throughout the app and in every world you run."
        >
          <Input
            data-testid="setup-account-username"
            id="setup-account-username"
            name="username"
            autoComplete="username"
            value={username}
            onChange={(event) => setUsername(event.target.value)}
            placeholder="founder"
          />
        </Field>

        <Field
          label="Email"
          htmlFor="setup-account-email"
          accent="Required"
          error={errorFor("email")}
          hint="Used for recovery and notices. It is not verified — this instance may not be able to send mail yet."
        >
          <Input
            data-testid="setup-account-email"
            id="setup-account-email"
            name="email"
            type="email"
            autoComplete="email"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
          />
        </Field>

        <Field
          label="Password"
          htmlFor="setup-account-password"
          accent="Required"
          error={errorFor("password")}
        >
          <Input
            data-testid="setup-account-password"
            id="setup-account-password"
            name="password"
            type="password"
            autoComplete="new-password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
          />
        </Field>

        <div className="grid gap-2 rounded-lg border border-border p-3">
          <div className="flex items-center justify-between text-sm">
            <span className="text-muted-foreground">Password strength</span>
            <strong data-testid="setup-password-strength">
              {strength.label}
            </strong>
          </div>
          <div
            className="h-1.5 w-full overflow-hidden rounded-full bg-muted"
            aria-hidden="true"
          >
            <div
              className={cn(
                "h-full rounded-full transition-all",
                STRENGTH_BAR_CLASSES[strength.tone],
              )}
              style={{ width: `${Math.max(strength.score, 0) * 25}%` }}
            />
          </div>
          <p className="text-xs text-muted-foreground">{strength.copy}</p>
        </div>

        <Field
          label="Confirm password"
          htmlFor="setup-account-password-confirmation"
          accent="Required"
          error={errorFor("passwordConfirmation")}
        >
          <Input
            data-testid="setup-account-password-confirmation"
            id="setup-account-password-confirmation"
            name="passwordConfirmation"
            type="password"
            autoComplete="new-password"
            value={passwordConfirmation}
            onChange={(event) => setPasswordConfirmation(event.target.value)}
          />
        </Field>

        <div>
          <Button
            data-testid="setup-account-submit"
            type="submit"
            variant="primary"
            icon="shield"
            disabled={isSubmitting}
          >
            {isSubmitting
              ? "Creating administrator..."
              : "Create the administrator"}
          </Button>
        </div>
      </form>

      <section className="grid gap-4 border-t border-border pt-6">
        <div className="grid gap-1">
          <h3 className="text-sm font-semibold">
            Or bootstrap from a provider this instance already trusts
          </h3>
          <p className="text-sm text-muted-foreground">
            The first administrator can come from a configured OAuth provider
            instead of a local password.
          </p>
        </div>

        <Field
          label="Preferred username"
          htmlFor="setup-oauth-username"
          accent="Optional"
          hint="Leave blank to inherit the provider's identity."
        >
          <Input
            data-testid="setup-oauth-username"
            id="setup-oauth-username"
            name="oauthUsername"
            autoComplete="username"
            value={oauthUsername}
            onChange={(event) => setOauthUsername(event.target.value)}
          />
        </Field>

        <div className="grid gap-3">
          {providers.length > 0 ? (
            providers.map((provider) => (
              <Button
                key={provider.provider_key}
                data-testid={`setup-oauth-start-${provider.provider_key}`}
                variant="secondary"
                icon="wand"
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
            <div data-testid="setup-oauth-unavailable">
              <StatusBadge variant="warning">
                No OAuth providers are configured, so the local account above is
                the way in.
              </StatusBadge>
            </div>
          )}
        </div>
      </section>

      {failure ? (
        <div data-testid="setup-account-error">
          <StatusBadge variant="danger">{failure}</StatusBadge>
        </div>
      ) : null}
    </div>
  );
}
