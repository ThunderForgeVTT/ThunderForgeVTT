import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Switch } from "@/components/ui/switch";
import type {
  OAuthProviderConfig,
  UpdateOAuthProviderInput,
} from "@/types/admin";

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
  const isEnvSourced = provider.configSource === "ENV";
  const [displayName, setDisplayName] = useState(provider.displayName);
  const [oauthClientId, setOauthClientId] = useState(
    provider.oauthClientId ?? "",
  );
  const [oauthClientSecret, setOauthClientSecret] = useState("");
  const [userinfoUrl, setUserinfoUrl] = useState(provider.userinfoUrl ?? "");
  const [scopes, setScopes] = useState(provider.scopes.join(" "));
  const [enabled, setEnabled] = useState(provider.enabled);
  const [status, setStatus] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  const providerSubtitle = useMemo(
    () =>
      provider.configured ? "Configured provider" : "Awaiting credentials",
    [provider.configured],
  );

  const handleSubmit = async () => {
    setIsSaving(true);
    setStatus(null);

    try {
      // Env-sourced rows: only `enabled` is ever writable (ADR-041) — the
      // server ignores every other field on such a row anyway, but omitting
      // them here keeps the request honest about what this save can do.
      await onSave(
        provider.id,
        isEnvSourced
          ? { enabled }
          : {
              displayName,
              oauthClientId: oauthClientId.trim() || undefined,
              oauthClientSecret: oauthClientSecret.trim() || undefined,
              enabled,
              userinfoUrl: userinfoUrl.trim() || undefined,
              scopes: scopes
                .split(/\s+/)
                .map((item) => item.trim())
                .filter(Boolean),
            },
      );
      setOauthClientSecret("");
      setStatus("Provider configuration updated.");
    } catch (error) {
      setStatus(
        error instanceof Error ? error.message : "Failed to update provider.",
      );
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <article className="grid gap-4 rounded-lg border border-border bg-secondary/40 p-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3 className="font-semibold">{displayName}</h3>
          <p className="text-muted-foreground">{providerSubtitle}</p>
          {isEnvSourced ? (
            <p
              className="mt-1 text-xs text-muted-foreground"
              data-testid={`${provider.id}-env-sourced-indicator`}
            >
              Configured via environment variable — credentials, URLs, and label
              are set by the server and can't be edited here. Only
              enabled/disabled is adjustable.
            </p>
          ) : null}
        </div>
        <label className="flex items-center gap-2 text-sm">
          <Switch
            checked={enabled}
            onCheckedChange={(checked) => setEnabled(checked)}
          />
          <span>{enabled ? "Enabled" : "Disabled"}</span>
        </label>
      </div>

      <div className="grid gap-4">
        <Field label="Display name" htmlFor={`${provider.id}-display-name`}>
          <Input
            id={`${provider.id}-display-name`}
            value={displayName}
            onChange={(event) => setDisplayName(event.target.value)}
            placeholder="Google"
            disabled={isEnvSourced}
            readOnly={isEnvSourced}
          />
        </Field>
        <Field label="Client ID" htmlFor={`${provider.id}-client-id`}>
          <Input
            id={`${provider.id}-client-id`}
            value={oauthClientId}
            onChange={(event) => setOauthClientId(event.target.value)}
            placeholder="OAuth client identifier"
            disabled={isEnvSourced}
            readOnly={isEnvSourced}
          />
        </Field>
        <Field
          label="Client secret"
          htmlFor={`${provider.id}-client-secret`}
          hint={
            isEnvSourced
              ? "Set via environment variable — not editable here."
              : provider.hasClientSecret
                ? "Leave blank to keep the stored secret."
                : "Add the first client secret for this provider."
          }
        >
          <Input
            id={`${provider.id}-client-secret`}
            type="password"
            value={oauthClientSecret}
            onChange={(event) => setOauthClientSecret(event.target.value)}
            placeholder={isEnvSourced ? "••••••••" : "Optional secret rotation"}
            disabled={isEnvSourced}
            readOnly={isEnvSourced}
          />
        </Field>
        <Field label="Userinfo URL" htmlFor={`${provider.id}-userinfo-url`}>
          <Input
            id={`${provider.id}-userinfo-url`}
            value={userinfoUrl}
            onChange={(event) => setUserinfoUrl(event.target.value)}
            placeholder="https://example.com/oauth/userinfo"
            disabled={isEnvSourced}
            readOnly={isEnvSourced}
          />
        </Field>
        <Field
          label="Scopes"
          htmlFor={`${provider.id}-scopes`}
          hint="Use space-separated OAuth scopes."
        >
          <Input
            id={`${provider.id}-scopes`}
            value={scopes}
            onChange={(event) => setScopes(event.target.value)}
            placeholder="openid profile email"
            disabled={isEnvSourced}
            readOnly={isEnvSourced}
          />
        </Field>
      </div>

      <div className="flex flex-wrap items-center gap-3">
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
