import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Field } from "@/components/ui/field/Field";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type { OAuthProviderConfig, UpdateOAuthProviderInput } from "@/types/admin";
import styles from "./OAuthProviderForm.module.scss";

interface OAuthProviderFormProps {
  provider: OAuthProviderConfig;
  onSave: (
    providerId: string,
    config: UpdateOAuthProviderInput,
  ) => Promise<OAuthProviderConfig>;
}

export function OAuthProviderForm({
  provider,
  onSave,
}: OAuthProviderFormProps) {
  const [displayName, setDisplayName] = useState(provider.displayName);
  const [oauthClientId, setOauthClientId] = useState(provider.oauthClientId ?? "");
  const [oauthClientSecret, setOauthClientSecret] = useState("");
  const [userinfoUrl, setUserinfoUrl] = useState(provider.userinfoUrl ?? "");
  const [scopes, setScopes] = useState(provider.scopes.join(" "));
  const [enabled, setEnabled] = useState(provider.enabled);
  const [status, setStatus] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  const providerSubtitle = useMemo(
    () =>
      provider.configured
        ? "Configured envoy"
        : "Awaiting credentials",
    [provider.configured],
  );

  const handleSubmit = async () => {
    setIsSaving(true);
    setStatus(null);

    try {
      await onSave(provider.id, {
        displayName,
        oauthClientId: oauthClientId.trim() || undefined,
        oauthClientSecret: oauthClientSecret.trim() || undefined,
        enabled,
        userinfoUrl: userinfoUrl.trim() || undefined,
        scopes: scopes
          .split(/\s+/)
          .map((item) => item.trim())
          .filter(Boolean),
      });
      setOauthClientSecret("");
      setStatus("Provider configuration updated.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Failed to update provider.");
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <article className={styles.formCard}>
      <div className={styles.header}>
        <div>
          <h3>{displayName}</h3>
          <p>{providerSubtitle}</p>
        </div>
        <label className={styles.toggle}>
          <input
            type="checkbox"
            checked={enabled}
            onChange={(event) => setEnabled(event.target.checked)}
          />
          <span>{enabled ? "Enabled" : "Disabled"}</span>
        </label>
      </div>

      <div className={styles.fields}>
        <Field label="Display name" htmlFor={`${provider.id}-display-name`}>
          <input
            id={`${provider.id}-display-name`}
            value={displayName}
            onChange={(event) => setDisplayName(event.target.value)}
            placeholder="Google"
          />
        </Field>
        <Field label="Client ID" htmlFor={`${provider.id}-client-id`}>
          <input
            id={`${provider.id}-client-id`}
            value={oauthClientId}
            onChange={(event) => setOauthClientId(event.target.value)}
            placeholder="OAuth client identifier"
          />
        </Field>
        <Field
          label="Client secret"
          htmlFor={`${provider.id}-client-secret`}
          hint={provider.hasClientSecret ? "Leave blank to keep the stored secret." : "Add the first client secret for this envoy."}
        >
          <input
            id={`${provider.id}-client-secret`}
            type="password"
            value={oauthClientSecret}
            onChange={(event) => setOauthClientSecret(event.target.value)}
            placeholder="Optional secret rotation"
          />
        </Field>
        <Field label="Userinfo URL" htmlFor={`${provider.id}-userinfo-url`}>
          <input
            id={`${provider.id}-userinfo-url`}
            value={userinfoUrl}
            onChange={(event) => setUserinfoUrl(event.target.value)}
            placeholder="https://example.com/oauth/userinfo"
          />
        </Field>
        <Field
          label="Scopes"
          htmlFor={`${provider.id}-scopes`}
          hint="Use space-separated OAuth scopes."
        >
          <input
            id={`${provider.id}-scopes`}
            value={scopes}
            onChange={(event) => setScopes(event.target.value)}
            placeholder="openid profile email"
          />
        </Field>
      </div>

      <div className={styles.footer}>
        <Button
          type="button"
          variant="secondary"
          icon="wand"
          onClick={() => void handleSubmit()}
          disabled={isSaving}
        >
          {isSaving ? "Saving..." : "Update provider"}
        </Button>
        {status ? <StatusBadge variant="info">{status}</StatusBadge> : null}
      </div>
    </article>
  );
}
