import { useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { AdminDetailRow, AdminTable } from "./AdminTable";
import { OAuthProviderForm } from "./OAuthProviderForm";
import type {
  OAuthProviderConfig,
  UpdateOAuthProviderInput,
} from "@/types/admin";

/**
 * Every configured sign-in provider as one row each.
 *
 * # What changed, and why
 *
 * This was seven stacked forms, one under the other: roughly 700px per
 * provider, five identical field labels repeated seven times, and no way to
 * answer "which of these is actually switched on" without scrolling the whole
 * card. The four facts an operator reads across providers — who it is,
 * whether it holds credentials, where those came from, whether people can
 * sign in with it — are now four columns, and the form that edits one is
 * behind that row's own disclosure.
 *
 * # One row open at a time
 *
 * Not a preference: `OAuthProviderForm` holds its own draft state and its own
 * unsaved `enabled` toggle. Leaving several open invites an operator to fill
 * in three and save one, and the two they abandoned look saved. Opening a
 * second closes the first, which throws away a draft — so the disclosure says
 * "Edit" and "Done", and closing is always the operator's own click.
 */

const COLUMNS = [
  "Provider",
  "Status",
  "Credentials",
  "Availability",
  "Edit",
] as const;

const COLUMN_WIDTHS = ["28%", "18%", "26%", "16%", "12%"] as const;

interface OAuthProvidersTableProps {
  providers: OAuthProviderConfig[];
  onSave: (
    providerId: string,
    config: UpdateOAuthProviderInput,
  ) => Promise<OAuthProviderConfig>;
}

function credentialSummary(provider: OAuthProviderConfig): string {
  if (provider.configSource === "ENV") {
    return "From the environment";
  }
  return provider.hasClientSecret ? "Secret stored here" : "No secret yet";
}

export function OAuthProvidersTable({
  providers,
  onSave,
}: OAuthProvidersTableProps) {
  const [openId, setOpenId] = useState<string | null>(null);

  if (!providers.length) {
    return (
      <StatusBadge variant="warning">
        No persisted OAuth providers are currently configured.
      </StatusBadge>
    );
  }

  return (
    <AdminTable
      label="OAuth providers"
      columns={COLUMNS}
      columnWidths={COLUMN_WIDTHS}
      data-testid="oauth-providers-table"
    >
      <tbody>
        {providers.map((provider) => {
          const open = openId === provider.id;
          return [
            <tr
              key={provider.id}
              className="border-b border-border last:border-b-0"
              data-testid={`oauth-provider-row-${provider.id}`}
              data-enabled={provider.enabled ? "true" : "false"}
              // Which provider a stack carries environment credentials for is
              // a property of the stack, not of the product, so a test that
              // needs "the env-sourced one" has to find it by this rather
              // than by name. It was previously only discoverable from inside
              // the editor, which a row no longer opens by default.
              data-config-source={provider.configSource}
            >
              <th scope="row" className="px-3 py-3 text-left font-semibold">
                <span className="block">{provider.displayName}</span>
                <span className="block text-xs font-normal text-muted-foreground">
                  {provider.id}
                </span>
              </th>
              <td className="px-3 py-3">
                <StatusBadge variant={provider.configured ? "success" : "info"}>
                  {provider.configured ? "Configured" : "Awaiting credentials"}
                </StatusBadge>
              </td>
              <td className="px-3 py-3 text-muted-foreground">
                {credentialSummary(provider)}
              </td>
              <td className="px-3 py-3">
                <StatusBadge variant={provider.enabled ? "success" : "warning"}>
                  {provider.enabled ? "Enabled" : "Disabled"}
                </StatusBadge>
              </td>
              <td className="px-3 py-3">
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  aria-expanded={open}
                  aria-controls={`oauth-provider-editor-${provider.id}`}
                  data-testid={`oauth-provider-toggle-${provider.id}`}
                  onClick={() => setOpenId(open ? null : provider.id)}
                >
                  {open ? "Done" : "Edit"}
                </Button>
              </td>
            </tr>,
            open ? (
              <AdminDetailRow
                key={`${provider.id}-editor`}
                columnCount={COLUMNS.length}
                data-testid={`oauth-provider-editor-row-${provider.id}`}
              >
                <div id={`oauth-provider-editor-${provider.id}`}>
                  <OAuthProviderForm provider={provider} onSave={onSave} />
                </div>
              </AdminDetailRow>
            ) : null,
          ];
        })}
      </tbody>
    </AdminTable>
  );
}
