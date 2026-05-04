import React, { useEffect, useState } from "react";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { SetupStatus, setupBasic, startSetupOAuth } from "../api/auth";

interface SetupViewProps {
  setupStatus: SetupStatus;
  onSetupComplete: () => Promise<void> | void;
}

export default function SetupView({
  setupStatus,
  onSetupComplete,
}: SetupViewProps) {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const { code } = useParams();
  const [adminCode, setAdminCode] = useState("");
  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [passwordConfirmation, setPasswordConfirmation] = useState("");
  const [oauthUsername, setOauthUsername] = useState("");
  const [status, setStatus] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isStartingOAuth, setIsStartingOAuth] = useState<string | null>(null);

  useEffect(() => {
    if (code) {
      setAdminCode(code);
    }
  }, [code]);

  useEffect(() => {
    const oauthError = searchParams.get("oauth_error");
    if (oauthError) {
      setStatus(oauthError);
    }
  }, [navigate, onSetupComplete, searchParams]);

  const onSubmitBasic = async (event: React.FormEvent) => {
    event.preventDefault();

    if (password !== passwordConfirmation) {
      setStatus("Passwords do not match");
      return;
    }

    setIsSubmitting(true);
    try {
      const result = await setupBasic(adminCode, username, email, password);
      setStatus(result);
      await onSetupComplete();
      navigate("/counter?bootstrap=complete", { replace: true });
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Setup failed");
    } finally {
      setIsSubmitting(false);
    }
  };

  const onStartOAuth = async (providerKey: string) => {
    setIsStartingOAuth(providerKey);
    setStatus("");
    try {
      await startSetupOAuth(providerKey, adminCode, oauthUsername || username);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "OAuth setup failed");
      setIsStartingOAuth(null);
    }
  };

  return (
    <div className="setup-shell">
      <section className="setup-hero">
        <p className="setup-eyebrow">Initial bootstrap</p>
        <h1>Secure the first administrator account.</h1>
        <p className="setup-copy">
          ThunderForge is in first-run mode. Enter the one-time admin code
          printed by the server, then create the initial administrator with
          either local credentials or a configured OAuth provider.
        </p>
      </section>

      <div className="setup-grid">
        <form onSubmit={onSubmitBasic} className="card setup-card">
          <h2>Username and password</h2>
          <p>Create the first admin with local credentials.</p>

          <label htmlFor="setup-admin-code">Bootstrap admin code</label>
          <input
            id="setup-admin-code"
            value={adminCode}
            onChange={(event) => setAdminCode(event.target.value)}
            placeholder="ABCD-EFGH-JKLM"
          />

          <label htmlFor="setup-username">Username</label>
          <input
            id="setup-username"
            value={username}
            onChange={(event) => setUsername(event.target.value)}
            placeholder="founder"
          />

          <label htmlFor="setup-email">Email</label>
          <input
            id="setup-email"
            type="email"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            placeholder="admin@example.com"
          />

          <label htmlFor="setup-password">Password</label>
          <input
            id="setup-password"
            type="password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
          />

          <label htmlFor="setup-password-confirmation">Confirm password</label>
          <input
            id="setup-password-confirmation"
            type="password"
            value={passwordConfirmation}
            onChange={(event) => setPasswordConfirmation(event.target.value)}
          />

          <button
            type="submit"
            className="btn btn-success"
            disabled={isSubmitting}
          >
            {isSubmitting ? "Creating admin..." : "Create admin account"}
          </button>
        </form>

        <section className="card setup-card">
          <h2>OAuth bootstrap</h2>
          <p>
            Start with a provider and create the initial admin account from that
            identity.
          </p>

          <label htmlFor="setup-oauth-code">Bootstrap admin code</label>
          <input
            id="setup-oauth-code"
            value={adminCode}
            onChange={(event) => setAdminCode(event.target.value)}
            placeholder="ABCD-EFGH-JKLM"
          />

          <label htmlFor="setup-oauth-username">Preferred username</label>
          <input
            id="setup-oauth-username"
            value={oauthUsername}
            onChange={(event) => setOauthUsername(event.target.value)}
            placeholder="Optional, defaults from provider email"
          />

          <div className="setup-provider-list">
            {setupStatus.configured_oauth_providers.length === 0 ? (
              <p>No OAuth providers are configured yet.</p>
            ) : (
              setupStatus.configured_oauth_providers.map((provider) => (
                <button
                  key={provider.provider_key}
                  type="button"
                  className="btn"
                  disabled={Boolean(isStartingOAuth)}
                  onClick={() => void onStartOAuth(provider.provider_key)}
                >
                  {isStartingOAuth === provider.provider_key
                    ? `Starting ${provider.display_name}...`
                    : `Continue with ${provider.display_name}`}
                </button>
              ))
            )}
          </div>
        </section>
      </div>

      {status ? <p className="setup-status">{status}</p> : null}
    </div>
  );
}
