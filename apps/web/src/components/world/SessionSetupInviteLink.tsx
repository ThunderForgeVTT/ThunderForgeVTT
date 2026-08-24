import { useState } from "react";
import { withCsrf } from "@/api/auth";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";

const GRAPHQL_ENDPOINT = "/api/graphql";

async function postGraphQL<TData>(
  query: string,
  variables?: Record<string, unknown>,
): Promise<TData> {
  const response = await fetch(GRAPHQL_ENDPOINT, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({
      "Content-Type": "application/json",
    }),
    body: JSON.stringify({ query, variables }),
  });

  type GraphQLResponse<T> = {
    data?: T;
    errors?: Array<{ message?: string }>;
  };

  const payload = (await response.json()) as GraphQLResponse<TData>;
  if (!response.ok || payload.errors?.length) {
    throw new Error(payload.errors?.[0]?.message || "GraphQL request failed");
  }
  if (!payload.data) {
    throw new Error("GraphQL response did not include data");
  }
  return payload.data;
}

interface SessionSetupInviteLinkProps {
  worldId: string;
}

/**
 * Spec 017 (FR-015/SC-005): surfaces the same shareable invite URL already
 * generated from the world dashboard's "Generate Join Link" control
 * (`CampaignSettingsPanel.tsx`), directly on Session Setup, so the GM
 * doesn't need to leave this page to copy/distribute it. Reuses the same
 * `generateInviteCode` mutation — no new server surface for this.
 */
export function SessionSetupInviteLink({ worldId }: SessionSetupInviteLinkProps) {
  const [isGenerating, setIsGenerating] = useState(false);
  const [inviteUrl, setInviteUrl] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  const handleGenerate = async () => {
    setIsGenerating(true);
    setStatus(null);
    try {
      const data = await postGraphQL<{ generateInviteCode: { inviteCode: string } }>(
        `
          mutation generateInviteCode($input: GenerateInviteCodeInput!) {
            generateInviteCode(input: $input) {
              inviteCode
            }
          }
        `,
        { input: { worldId, maxUses: 5 } },
      );
      const code = data.generateInviteCode?.inviteCode;
      if (code) {
        const url = `${window.location.origin}/join/${code}`;
        setInviteUrl(url);
        await navigator.clipboard.writeText(url).catch(() => {});
        setStatus("Copied to clipboard.");
      }
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to generate invite link");
    } finally {
      setIsGenerating(false);
    }
  };

  return (
    <div className="grid gap-2" data-testid="session-setup-invite-link">
      <Button
        type="button"
        variant="secondary"
        size="sm"
        onClick={() => void handleGenerate()}
        disabled={isGenerating}
        data-testid="session-setup-generate-invite"
      >
        {isGenerating ? "Generating..." : "Copy invite link"}
      </Button>
      {inviteUrl ? (
        <Input readOnly value={inviteUrl} data-testid="session-setup-invite-url" />
      ) : null}
      {status ? <p className="text-xs text-muted-foreground">{status}</p> : null}
    </div>
  );
}
