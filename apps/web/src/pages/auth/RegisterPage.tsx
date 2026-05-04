import type { FormEvent } from "react";
import { useEffect, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import { getSetupStatus, startOAuthLogin } from "@/api/auth";
import { Avatar } from "@/components/ui/avatar/Avatar";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Field } from "@/components/ui/field/Field";
import { RuneDivider } from "@/components/ui/rune-divider/RuneDivider";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { AuthLayout } from "@/layouts/auth-layout/AuthLayout";
import { useAuth } from "@/hooks/useAuth";
import type { SetupProvider } from "@/types/auth";
import type { SeoConfig } from "@/types/seo";
import styles from "./AuthPage.module.scss";

export const registerPageSeo: SeoConfig = {
  title: "Create account",
  description:
    "Create a local ThunderForge VTT account with secure password storage and session-backed authentication.",
  keywords: [
    "ThunderForge register",
    "create tabletop account",
    "virtual tabletop sign up",
  ],
  canonicalPath: "/register",
  prefetchHrefs: ["/login"],
};

function redirectTarget(search: string) {
  const params = new URLSearchParams(search);
  const returnTo = params.get("returnTo");
  return returnTo && returnTo.startsWith("/") ? returnTo : null;
}

export default function RegisterPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const { register, redirectAfterLogin } = useAuth();
  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [passwordConfirmation, setPasswordConfirmation] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [providers, setProviders] = useState<SetupProvider[]>([]);

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

  const onSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    if (password !== passwordConfirmation) {
      setStatus("Passwords do not match.");
      return;
    }

    setIsSubmitting(true);
    setStatus(null);

    try {
      const response = await register({
        username,
        email,
        password,
      });
      setStatus(response.message);

      if (response.session?.authenticated) {
        navigate(
          redirectTarget(location.search) ?? redirectAfterLogin(response.session.user),
          { replace: true },
        );
      }
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Registration failed.");
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <>
      <SEO {...registerPageSeo} />
      <AuthLayout
        eyebrow="Onboarding"
        title="Create a local ThunderForge account."
        description="Phase 1 uses secure cookie sessions backed by the existing Rust auth stack, leaving room for worlds, actors, permissions, and multiplayer ownership later."
        aside={
          <Card surface="parchment">
            <div className={styles.auxiliary}>
              <h2>Already have access?</h2>
              <p>Return to login or review the welcome hall this session unlocks.</p>
              <div className={styles.avatarGroup}>
                <div className={styles.avatarRow}>
                  <Avatar seed="archivist" name="Archivist" />
                  <Avatar seed="summoner" name="Summoner" />
                </div>
                <p>Accounts created here can later attach to worlds, actors, invites, and policy scopes.</p>
              </div>
              <RuneDivider label="Return paths" />
              <div className={styles.linkList}>
                <Link to="/login">Return to login</Link>
                <Link to="/welcome">Open the welcome hall</Link>
              </div>
            </div>
          </Card>
        }
      >
        <Card surface="leather">
          <form onSubmit={onSubmit} className={styles.form}>
            <div className={styles.header}>
              <h2>Create credentials</h2>
              <p>Registration stores Argon2 password hashes and starts a database-backed session immediately.</p>
            </div>

            <Field label="Username" htmlFor="register-username">
              <input
                id="register-username"
                name="username"
                autoComplete="username"
                value={username}
                onChange={(event) => setUsername(event.target.value)}
                placeholder="world-builder"
              />
            </Field>

            <Field label="Email address" htmlFor="register-email">
              <input
                id="register-email"
                name="email"
                type="email"
                autoComplete="email"
                value={email}
                onChange={(event) => setEmail(event.target.value)}
                placeholder="builder@thunderforge.app"
              />
            </Field>

            <Field label="Password" htmlFor="register-password" hint="Use at least 12 characters.">
              <input
                id="register-password"
                name="password"
                type="password"
                autoComplete="new-password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                placeholder="Create a password"
              />
            </Field>

            <Field label="Confirm password" htmlFor="register-password-confirmation">
              <input
                id="register-password-confirmation"
                name="passwordConfirmation"
                type="password"
                autoComplete="new-password"
                value={passwordConfirmation}
                onChange={(event) => setPasswordConfirmation(event.target.value)}
                placeholder="Confirm your password"
              />
            </Field>

            <div className={styles.actions}>
              <Button
                type="submit"
                variant="success"
                size="lg"
                disabled={isSubmitting}
                icon="quill"
              >
                {isSubmitting ? "Creating account..." : "Create account"}
              </Button>
            </div>

            {status ? <StatusBadge>{status}</StatusBadge> : null}

            {providers.length ? (
              <>
                <RuneDivider label="Existing linked provider" />
                <div className={styles.actions}>
                  {providers.map((provider) => (
                    <Button
                      key={provider.provider_key}
                      type="button"
                      variant="secondary"
                      icon="spark"
                      onClick={() =>
                        startOAuthLogin(
                          provider.provider_key,
                          redirectTarget(location.search) ?? "/welcome",
                        )
                      }
                    >
                      Sign in with {provider.display_name}
                    </Button>
                  ))}
                </div>
              </>
            ) : null}
          </form>
        </Card>
      </AuthLayout>
    </>
  );
}
