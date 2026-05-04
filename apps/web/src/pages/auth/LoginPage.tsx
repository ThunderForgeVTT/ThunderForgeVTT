import type { FormEvent } from "react";
import { useState } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import { Avatar } from "@/components/ui/avatar/Avatar";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Field } from "@/components/ui/field/Field";
import { RuneDivider } from "@/components/ui/rune-divider/RuneDivider";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useAuth } from "@/hooks/useAuth";
import { AuthLayout } from "@/layouts/auth-layout/AuthLayout";
import type { SeoConfig } from "@/types/seo";
import styles from "./AuthPage.module.scss";

export const loginPageSeo: SeoConfig = {
  title: "Login",
  description:
    "Access ThunderForge VTT to manage your instance, review setup status, and enter collaborative worlds.",
  keywords: [
    "ThunderForge login",
    "virtual tabletop login",
    "tabletop control room",
  ],
  canonicalPath: "/login",
  preloadAssets: [
    { href: "/brand-mark.svg", as: "image", type: "image/svg+xml" },
  ],
  prefetchHrefs: ["/register", "/counter"],
};

function redirectTarget(search: string) {
  const params = new URLSearchParams(search);
  const returnTo = params.get("returnTo");
  return returnTo && returnTo.startsWith("/") ? returnTo : "/counter";
}

export default function LoginPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const { login } = useAuth();
  const [identifier, setIdentifier] = useState("");
  const [password, setPassword] = useState("");
  const [twoFactorCode, setTwoFactorCode] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const onSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setIsSubmitting(true);
    setStatus(null);

    try {
      const response = await login({
        identifier,
        password,
        two_factor_code: twoFactorCode.trim() || undefined,
      });
      setStatus(response.message);

      if (response.session?.authenticated) {
        navigate(redirectTarget(location.search), { replace: true });
      }
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Login failed.");
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <>
      <SEO {...loginPageSeo} />
      <AuthLayout
        eyebrow="Access"
        title="Sign in to your ThunderForge instance."
        description="Keep authentication flows simple, typed, and easy to extend as you connect more areas of the tabletop experience."
        aside={
          <Card surface="parchment">
            <div className={styles.auxiliary}>
              <h2>What you can do here</h2>
              <p>
                Authenticate with the current instance, check server readiness,
                and move directly into collaborative world views.
              </p>
              <div className={styles.avatarGroup}>
                <div className={styles.avatarRow}>
                  <Avatar seed="guild-warden" name="Guild warden" />
                  <Avatar seed="map-smith" name="Map smith" />
                </div>
                <p>
                  Dicebear portraits can become player profiles, NPC cards, and
                  token seeds.
                </p>
              </div>
              <RuneDivider label="Fast routes" />
              <div className={styles.linkList}>
                <Link to="/register">Create a local account</Link>
                <Link to="/counter">Review the dashboard preview</Link>
              </div>
            </div>
          </Card>
        }
      >
        <Card surface="leather">
          <form onSubmit={onSubmit} className={styles.form}>
            <div className={styles.header}>
              <h2>Local credentials</h2>
              <p>
                Sign in with the email address or username tied to your local ThunderForge account.
              </p>
            </div>

            <Field label="Email address or username" htmlFor="login-identifier">
              <input
                id="login-identifier"
                name="identifier"
                autoComplete="username"
                value={identifier}
                onChange={(event) => setIdentifier(event.target.value)}
                placeholder="founder@thunderforge.app"
              />
            </Field>

            <Field label="Password" htmlFor="login-password">
              <input
                id="login-password"
                name="password"
                type="password"
                autoComplete="current-password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                placeholder="Enter your password"
              />
            </Field>

            <Field
              label="Two-factor code"
              htmlFor="login-two-factor"
              hint="Optional unless your account requires 2FA."
            >
              <input
                id="login-two-factor"
                name="twoFactorCode"
                inputMode="numeric"
                value={twoFactorCode}
                onChange={(event) => setTwoFactorCode(event.target.value)}
                placeholder="123456"
              />
            </Field>

            <div className={styles.actions}>
              <Button
                type="submit"
                variant="primary"
                size="lg"
                disabled={isSubmitting}
                icon="shield"
              >
                {isSubmitting ? "Signing in..." : "Sign in"}
              </Button>
            </div>

            {status ? <StatusBadge>{status}</StatusBadge> : null}
          </form>
        </Card>
      </AuthLayout>
    </>
  );
}
