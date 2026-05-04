import { Link } from "react-router-dom";
import type { FormEvent } from "react";
import { useState } from "react";
import { Avatar } from "@/components/ui/avatar/Avatar";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Field } from "@/components/ui/field/Field";
import { RuneDivider } from "@/components/ui/rune-divider/RuneDivider";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { AuthLayout } from "@/layouts/auth-layout/AuthLayout";
import { basicSignUp } from "@/services/auth";
import type { SeoConfig } from "@/types/seo";
import styles from "./AuthPage.module.scss";

export const signUpPageSeo: SeoConfig = {
  title: "Create account",
  description:
    "Create a local ThunderForge VTT account with a form structure designed for reusable, type-safe growth.",
  keywords: ["ThunderForge sign up", "create tabletop account", "React auth page"],
  canonicalPath: "/signup",
  prefetchHrefs: ["/login"],
};

export default function SignUpPage() {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [passwordConfirmation, setPasswordConfirmation] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const onSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    if (password !== passwordConfirmation) {
      setStatus("Passwords do not match.");
      return;
    }

    setIsSubmitting(true);
    setStatus(null);

    try {
      const result = await basicSignUp(username, password);
      setStatus(result);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Sign-up failed.");
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <>
      <SEO {...signUpPageSeo} />
      <AuthLayout
        eyebrow="Onboarding"
        title="Create a local ThunderForge account."
        description="Use the same reusable primitives as login while keeping this page ready for more fields, policies, and post-signup flows."
        aside={
          <Card surface="parchment">
            <div className={styles.auxiliary}>
              <h2>Already have access?</h2>
              <p>Jump back to the login flow or review the instance dashboard preview.</p>
              <div className={styles.avatarGroup}>
                <div className={styles.avatarRow}>
                  <Avatar seed="archivist" name="Archivist" />
                  <Avatar seed="summoner" name="Summoner" />
                </div>
                <p>New accounts can later inherit world, scene, and permission presets.</p>
              </div>
              <RuneDivider label="Return paths" />
              <div className={styles.linkList}>
                <Link to="/login">Return to login</Link>
                <Link to="/counter">Open the dashboard preview</Link>
              </div>
            </div>
          </Card>
        }
      >
        <Card surface="leather">
          <form onSubmit={onSubmit} className={styles.form}>
            <div className={styles.header}>
              <h2>Create credentials</h2>
              <p>Keep local sign-up and login consistent with the same form system.</p>
            </div>

            <Field label="Username" htmlFor="signup-username">
              <input
                id="signup-username"
                name="username"
                autoComplete="username"
                value={username}
                onChange={(event) => setUsername(event.target.value)}
                placeholder="world-builder"
              />
            </Field>

            <Field label="Password" htmlFor="signup-password">
              <input
                id="signup-password"
                name="password"
                type="password"
                autoComplete="new-password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                placeholder="Create a password"
              />
            </Field>

            <Field label="Confirm password" htmlFor="signup-password-confirmation">
              <input
                id="signup-password-confirmation"
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
                {isSubmitting ? "Creating account..." : "Sign up"}
              </Button>
            </div>

            {status ? <StatusBadge>{status}</StatusBadge> : null}
          </form>
        </Card>
      </AuthLayout>
    </>
  );
}
