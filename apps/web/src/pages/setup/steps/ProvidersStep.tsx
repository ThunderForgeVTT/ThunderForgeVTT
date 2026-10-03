import { useCallback, useEffect, useState } from "react";
import { getOAuthProviders, updateOAuthProvider } from "@/api/admin";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { OAuthProvidersTable } from "@/pages/admin/components/OAuthProvidersTable";
import type {
  OAuthProviderConfig,
  UpdateOAuthProviderInput,
} from "@/types/admin";

/**
 * Letting people sign in with an account they already have.
 *
 * # Why it is a step and not a setting
 *
 * Every other screen in this wizard is derived from the settings registry:
 * one step per group, fields from declarations. A provider is not a setting —
 * it is a row in `oauth_providers` with its own mutation, its own
 * environment-authoritative source rule (ADR-041) and four of them seeded by
 * migration — so this step is written by hand and listed beside the account,
 * second-factor and review steps in `buildSetupSteps`.
 *
 * # Why it is here rather than in the admin panel only
 *
 * Because of what the previous step just decided. An invite-only instance
 * hands out a link, and that link admits its holder either by creating an
 * account or by signing in with any provider enabled here — `oauth.rs`
 * carries the invitation code through the authorization session and redeems
 * it on the way back. An operator who sets up invite-only and never finds
 * this screen has a table whose players must all type new passwords.
 *
 * # Why it reuses the admin table verbatim
 *
 * `OAuthProvidersTable` already solves the thing a wizard would get wrong:
 * four providers' worth of forms on one screen reads as four mandatory tasks.
 * One row each, one editor open at a time, and the row says whether it holds
 * credentials. Reusing it also means the screen an operator learns here is
 * the screen they return to later, which is the whole argument for a wizard
 * that is shaped like the product rather than like a questionnaire.
 *
 * Nothing on this step is required, and it carries no "next" of its own — the
 * wizard's own footer moves on, and `complete: true` means a resumed pass
 * does not land back here.
 */
export interface ProvidersStepProps {
  /**
   * The access policy as the Access step left it, so this step can say what
   * an invite link will actually do. Undefined when that step has not been
   * reached, which on a resumed pass is normal.
   */
  accessPolicy?: string;
}

export function ProvidersStep({ accessPolicy }: ProvidersStepProps) {
  const [providers, setProviders] = useState<OAuthProviderConfig[] | null>(
    null,
  );
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getOAuthProviders()
      .then((rows) => {
        if (!cancelled) {
          setProviders(rows);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setFailure(
            error instanceof Error
              ? error.message
              : "Could not load sign-in providers.",
          );
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // The table's own save contract: hand back the saved row. Merging it into
  // local state rather than re-querying keeps a secret an operator just typed
  // from being replaced by the `hasClientSecret` summary mid-edit.
  const onSave = useCallback(
    async (providerId: string, config: UpdateOAuthProviderInput) => {
      const saved = await updateOAuthProvider(providerId, config);
      setProviders((rows) =>
        (rows ?? []).map((row) => (row.id === saved.id ? saved : row)),
      );
      return saved;
    },
    [],
  );

  const inviteOnly = accessPolicy === "invite_only";

  return (
    <div className="grid gap-6" data-testid="setup-providers-step">
      <p
        data-testid="setup-providers-explainer"
        className="rounded-lg border border-border bg-muted/40 p-3 text-sm text-muted-foreground"
      >
        {inviteOnly
          ? "You chose invite-only, so nobody arrives here uninvited. A provider enabled below gives an invited player a second way to use their link: they can create an account with it, or sign in with an account they already have — the invitation is redeemed either way."
          : "Optional. This instance works with its own accounts alone; a provider enabled below lets someone sign in with an account they already have instead of inventing another password."}
      </p>

      {failure ? (
        <StatusBadge variant="warning" data-testid="setup-providers-failure">
          {failure}
        </StatusBadge>
      ) : null}

      {providers === null && !failure ? (
        <p className="text-sm text-muted-foreground">
          Loading sign-in providers&hellip;
        </p>
      ) : null}

      {providers !== null ? (
        <OAuthProvidersTable providers={providers} onSave={onSave} />
      ) : null}

      <p className="text-sm text-muted-foreground">
        You can configure or disable any of these later in the admin area.
        Leaving every one of them alone is a supported instance.
      </p>
    </div>
  );
}
