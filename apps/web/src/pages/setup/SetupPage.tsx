import type { FormEvent } from "react";
import { useState } from "react";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Field } from "@/components/ui/field/Field";
import { Grid } from "@/components/ui/grid/Grid";
import { RuneDivider } from "@/components/ui/rune-divider/RuneDivider";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { setupBasic, startSetupOAuth } from "@/services/auth";
import type { SetupStatus } from "@/types/auth";
import type { SeoConfig } from "@/types/seo";
import styles from "./SetupPage.module.scss";

export const setupPageSeo: SeoConfig = {
  title: "Secure first-run setup",
  description:
    "Complete ThunderForge VTT bootstrap with a one-time admin code, local credentials, or an approved OAuth provider.",
  keywords: ["ThunderForge setup", "bootstrap admin", "virtual tabletop onboarding"],
  canonicalPath: "/setup",
  noindex: true,
  prefetchHrefs: ["/setup/callback", "/counter"],
};

interface SetupPageProps {
  setupStatus: SetupStatus;
  onSetupComplete: () => Promise<unknown> | unknown;
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
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isStartingOAuth, setIsStartingOAuth] = useState<string | null>(null);

  const oauthError = searchParams.get("oauth_error");
  const resolvedAdminCode = adminCode || code || "";
  const resolvedStatus = status ?? oauthError;

  const onSubmitBasic = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    if (password !== passwordConfirmation) {
      setStatus("Passwords do not match.");
      return;
    }

    setIsSubmitting(true);
    setStatus(null);

    try {
      const result = await setupBasic(resolvedAdminCode, username, email, password);
      setStatus(result);
      await onSetupComplete();
      navigate("/counter?bootstrap=complete", { replace: true });
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Setup failed.");
    } finally {
      setIsSubmitting(false);
    }
  };

  const onStartOAuth = async (providerKey: string) => {
    setIsStartingOAuth(providerKey);
    setStatus(null);

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

  return (
    <>
      <SEO {...setupPageSeo} />
      <Container>
        <main className={styles.shell}>
          <section className={styles.hero}>
            <p className={styles.eyebrow}>Initial bootstrap</p>
            <h1>Secure the first administrator account.</h1>
            <p>
              ThunderForge is in first-run mode. Enter the one-time admin code
              from the server and finish setup with either local credentials or a
              configured OAuth provider.
            </p>
          </section>

          <RuneDivider label="Choose a founding ritual" />

          <Grid columns="two">
            <Card surface="leather" className={styles.card}>
              <form onSubmit={onSubmitBasic} className={styles.card}>
                <h2>Username and password</h2>
                <p>Create the first admin with local credentials.</p>

                <Field label="Bootstrap admin code" htmlFor="setup-admin-code">
                  <input
                    id="setup-admin-code"
                    value={resolvedAdminCode}
                    onChange={(event) => setAdminCode(event.target.value)}
                    placeholder="ABCD-EFGH-JKLM"
                  />
                </Field>

                <Field label="Username" htmlFor="setup-username">
                  <input
                    id="setup-username"
                    value={username}
                    onChange={(event) => setUsername(event.target.value)}
                    placeholder="founder"
                  />
                </Field>

                <Field label="Email" htmlFor="setup-email">
                  <input
                    id="setup-email"
                    type="email"
                    value={email}
                    onChange={(event) => setEmail(event.target.value)}
                    placeholder="admin@example.com"
                  />
                </Field>

                <Field label="Password" htmlFor="setup-password">
                  <input
                    id="setup-password"
                    type="password"
                    value={password}
                    onChange={(event) => setPassword(event.target.value)}
                    placeholder="Create a strong password"
                  />
                </Field>

                <Field
                  label="Confirm password"
                  htmlFor="setup-password-confirmation"
                >
                  <input
                    id="setup-password-confirmation"
                    type="password"
                    value={passwordConfirmation}
                    onChange={(event) => setPasswordConfirmation(event.target.value)}
                    placeholder="Confirm the password"
                  />
                </Field>

                <Button
                  type="submit"
                  variant="success"
                  size="lg"
                  disabled={isSubmitting}
                  icon="shield"
                >
                  {isSubmitting ? "Creating admin..." : "Create admin account"}
                </Button>
              </form>
            </Card>

            <Card surface="parchment" className={styles.card}>
              <h2>OAuth bootstrap</h2>
              <p>
                Start with a configured provider and create the initial admin from
                that identity.
              </p>

              <Field label="Bootstrap admin code" htmlFor="setup-oauth-code">
                <input
                  id="setup-oauth-code"
                  value={resolvedAdminCode}
                  onChange={(event) => setAdminCode(event.target.value)}
                  placeholder="ABCD-EFGH-JKLM"
                />
              </Field>

              <Field
                label="Preferred username"
                htmlFor="setup-oauth-username"
                hint="Optional. Defaults to the email or username returned by the provider."
              >
                <input
                  id="setup-oauth-username"
                  value={oauthUsername}
                  onChange={(event) => setOauthUsername(event.target.value)}
                  placeholder="Optional username override"
                />
              </Field>

              <div className={styles.providerList}>
                {setupStatus.configured_oauth_providers.length === 0 ? (
                  <StatusBadge variant="warning">
                    No OAuth providers are configured yet.
                  </StatusBadge>
                ) : (
                  setupStatus.configured_oauth_providers.map((provider) => (
                    <Button
                      key={provider.provider_key}
                      variant="secondary"
                      icon="wand"
                      fullWidth
                      disabled={Boolean(isStartingOAuth)}
                      onClick={() => void onStartOAuth(provider.provider_key)}
                    >
                      {isStartingOAuth === provider.provider_key
                        ? `Starting ${provider.display_name}...`
                        : `Continue with ${provider.display_name}`}
                    </Button>
                  ))
                )}
              </div>
            </Card>
          </Grid>

          {resolvedStatus ? (
            <StatusBadge
              variant={
                resolvedStatus.toLowerCase().includes("failed") ? "danger" : "info"
              }
            >
              {resolvedStatus}
            </StatusBadge>
          ) : null}
        </main>
      </Container>
    </>
  );
}
