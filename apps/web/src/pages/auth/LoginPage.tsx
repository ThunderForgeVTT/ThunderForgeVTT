import type { FormEvent } from "react";
import { useState } from "react";
import { Link } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Field } from "@/components/ui/field/Field";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { AuthLayout } from "@/layouts/auth-layout/AuthLayout";
import { basicLogin } from "@/services/auth";
import type { SeoConfig } from "@/types/seo";
import styles from "./AuthPage.module.scss";

export const loginPageSeo: SeoConfig = {
  title: "Login",
  description:
    "Access ThunderForge VTT to manage your instance, review setup status, and enter collaborative worlds.",
  keywords: ["ThunderForge login", "virtual tabletop login", "tabletop control room"],
  canonicalPath: "/login",
  preloadAssets: [{ href: "/brand-mark.svg", as: "image", type: "image/svg+xml" }],
  prefetchHrefs: ["/signup", "/counter"],
};

export default function LoginPage() {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const onSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setIsSubmitting(true);
    setStatus(null);

    try {
      const result = await basicLogin(username, password);
      setStatus(result);
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
          <Card>
            <div className={styles.auxiliary}>
              <h2>What you can do here</h2>
              <p>
                Authenticate with the current instance, check server readiness, and
                move directly into collaborative world views.
              </p>
              <div className={styles.linkList}>
                <Link to="/signup">Create a local account</Link>
                <Link to="/counter">Review the dashboard preview</Link>
              </div>
            </div>
          </Card>
        }
      >
        <Card>
          <form onSubmit={onSubmit} className={styles.form}>
            <div className={styles.header}>
              <h2>Local credentials</h2>
              <p>Use a username and password configured for this ThunderForge instance.</p>
            </div>

            <Field label="Username" htmlFor="login-username">
              <input
                id="login-username"
                name="username"
                autoComplete="username"
                value={username}
                onChange={(event) => setUsername(event.target.value)}
                placeholder="founder"
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

            <div className={styles.actions}>
              <Button type="submit" variant="primary" size="lg" disabled={isSubmitting}>
                {isSubmitting ? "Signing in..." : "Login"}
              </Button>
            </div>

            {status ? <StatusBadge>{status}</StatusBadge> : null}
          </form>
        </Card>
      </AuthLayout>
    </>
  );
}
