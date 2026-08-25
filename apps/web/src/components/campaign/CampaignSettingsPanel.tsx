import { useState, useEffect } from "react";
import { withCsrf } from "@/api/auth";
import { getWorld, updateWorldAllowPlayerCreatedActors } from "@/api/world";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Input } from "@/components/ui/input";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";

interface Invite {
  id: string;
  inviteCode: string;
  maxUses: number;
  usedCount: number;
  expiresAt?: string;
  createdAt: string;
}

interface CampaignSettingsPanelProps {
  worldId: string;
}

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
    body: JSON.stringify({
      query,
      variables,
    }),
  });

  type GraphQLResponse<T> = {
    data?: T;
    errors?: Array<{ message?: string }>;
  };

  const payload = (await response.json()) as GraphQLResponse<TData>;
  if (!response.ok) {
    throw new Error(payload.errors?.[0]?.message || "GraphQL request failed");
  }

  if (payload.errors?.length) {
    throw new Error(payload.errors[0]?.message || "GraphQL request failed");
  }

  if (!payload.data) {
    throw new Error("GraphQL response did not include data");
  }

  return payload.data;
}

/**
 * CampaignSettingsPanel manages invites and world-level campaign settings.
 *
 * Features:
 * - Generate new invite codes
 * - Display active invites with usage counters
 * - Copy-to-clipboard invite URLs
 * - Show expiry and max uses
 * - Toggle whether players may create their own actors
 *
 * Spec 023 (FR-011): the player roster and its role-change/remove-member
 * controls moved to the dedicated Players sidebar section
 * (`PlayersPage.tsx`) — this panel no longer duplicates them.
 */
export function CampaignSettingsPanel({ worldId }: CampaignSettingsPanelProps) {
  const [invites, setInvites] = useState<Invite[]>([]);
  const [invitesLoading, setInvitesLoading] = useState(true);
  const [isGenerating, setIsGenerating] = useState(false);
  const [copiedCode, setCopiedCode] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [allowPlayerCreatedActors, setAllowPlayerCreatedActors] = useState(false);
  const [isUpdatingAllowSetting, setIsUpdatingAllowSetting] = useState(false);

  // Load invites on mount
  useEffect(() => {
    void loadInvites();
    void getWorld(worldId).then((world) => {
      if (world) {
        setAllowPlayerCreatedActors(world.allowPlayerCreatedActors);
      }
    });
  }, [worldId]);

  const handleToggleAllowPlayerCreatedActors = async (allow: boolean) => {
    setIsUpdatingAllowSetting(true);
    setError(null);
    try {
      const updated = await updateWorldAllowPlayerCreatedActors(worldId, allow);
      setAllowPlayerCreatedActors(updated.allowPlayerCreatedActors);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to update setting");
    } finally {
      setIsUpdatingAllowSetting(false);
    }
  };

  const loadInvites = async () => {
    try {
      setInvitesLoading(true);
      const data = await postGraphQL<{ worldInvites: Invite[] }>(
        `
          query worldInvites($worldId: ID!) {
            worldInvites(worldId: $worldId) {
              id
              inviteCode
              maxUses
              usedCount
              expiresAt
              createdAt
            }
          }
        `,
        { worldId },
      );
      setInvites(data.worldInvites || []);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load invites");
    } finally {
      setInvitesLoading(false);
    }
  };

  const handleGenerateInvite = async () => {
    try {
      setError(null);
      setIsGenerating(true);

      const data = await postGraphQL<{
        generateInviteCode: {
          inviteCode: string;
        };
      }>(
        `
          mutation generateInviteCode($input: GenerateInviteCodeInput!) {
            generateInviteCode(input: $input) {
              inviteCode
            }
          }
        `,
        {
          input: {
            worldId,
            maxUses: 5,
          },
        },
      );

      const code = data.generateInviteCode?.inviteCode;
      if (code) {
        await navigator.clipboard.writeText(generateInviteUrl(code));
        setCopiedCode(code);
        setTimeout(() => setCopiedCode(null), 2000);

        // Reload invites to show the new one
        await loadInvites();
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to generate invite code");
    } finally {
      setIsGenerating(false);
    }
  };

  const handleCopyToClipboard = async (code: string) => {
    try {
      await navigator.clipboard.writeText(generateInviteUrl(code));
      setCopiedCode(code);
      setTimeout(() => setCopiedCode(null), 2000);
    } catch (err) {
      setError("Failed to copy to clipboard");
    }
  };

  const generateInviteUrl = (code: string) => {
    return `${window.location.origin}/join/${code}`;
  };

  const getInviteStatus = (invite: Invite) => {
    const used = invite.usedCount || 0;
    const max = invite.maxUses || 0;
    return `${used}/${max} uses`;
  };

  const isInviteValid = (invite: Invite) => {
    const usedCount = invite.usedCount || 0;
    const maxUses = invite.maxUses || 0;
    const isExpired = invite.expiresAt && new Date(invite.expiresAt) < new Date();
    return usedCount < maxUses && !isExpired;
  };

  return (
    <section>
      <Card surface="parchment" className="grid gap-6 p-6">
        <div>
          <h2 className="text-xl font-semibold">Campaign Settings</h2>
          <p className="text-muted-foreground">
            Manage invites and player-created character settings
          </p>
        </div>

        {error && <StatusBadge variant="danger">{error}</StatusBadge>}

        {/* Invite Players Section */}
        <div className="grid gap-3">
          <h3 className="font-semibold">Invite Players</h3>
          <p className="text-sm text-muted-foreground">
            Generate join links to share with other players. Each link allows a specific number of joins.
          </p>

          <Button
            onClick={() => void handleGenerateInvite()}
            disabled={isGenerating}
            icon="link"
            className="justify-self-start"
          >
            {isGenerating ? "Generating..." : "Generate Join Link"}
          </Button>

          {invitesLoading ? (
            <Loader label="Loading invites..." />
          ) : invites.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              No active invites yet. Generate one to get started.
            </p>
          ) : (
            <div className="grid gap-3">
              {invites.map((invite) => (
                <div
                  key={invite.id}
                  className="grid gap-3 rounded-lg border border-border p-4"
                >
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-sm text-muted-foreground">
                      {getInviteStatus(invite)}
                    </span>
                    {!isInviteValid(invite) && (
                      <StatusBadge variant="warning">Expired</StatusBadge>
                    )}
                  </div>

                  <div className="flex gap-2">
                    <Input
                      type="text"
                      readOnly
                      value={generateInviteUrl(invite.inviteCode)}
                    />
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => void handleCopyToClipboard(invite.inviteCode)}
                      icon={copiedCode === invite.inviteCode ? "check" : "copy"}
                    >
                      {copiedCode === invite.inviteCode ? "Copied!" : "Copy link"}
                    </Button>
                  </div>

                  <div className="flex gap-4 text-xs text-muted-foreground">
                    <span>Created: {new Date(invite.createdAt).toLocaleDateString()}</span>
                    {invite.expiresAt && (
                      <span>Expires: {new Date(invite.expiresAt).toLocaleDateString()}</span>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Spec 017 (FR-007): player-created character setting */}
        <div className="grid gap-3">
          <h3 className="font-semibold">Player-created characters</h3>
          <p className="text-sm text-muted-foreground">
            When on, a joining player without a GM-designated character can create their own
            on the Actor Selection screen. Off by default.
          </p>
          <label className="flex items-center gap-2 text-sm" data-testid="allow-player-created-actors-toggle">
            <input
              type="checkbox"
              checked={allowPlayerCreatedActors}
              disabled={isUpdatingAllowSetting}
              onChange={(e) => void handleToggleAllowPlayerCreatedActors(e.target.checked)}
            />
            Allow players to create their own actors
          </label>
        </div>
      </Card>
    </section>
  );
}
