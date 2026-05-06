import { useState, useEffect } from "react";
import { withCsrf } from "@/api/auth";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import styles from "./CampaignSettingsPanel.module.scss";

interface Invite {
  id: string;
  inviteCode: string;
  maxUses: number;
  usedCount: number;
  expiresAt?: string;
  createdAt: string;
}

interface Member {
  id: string;
  userId: string;
  role: string;
  joinedAt: string;
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
 * CampaignSettingsPanel displays and manages invites and player roster.
 *
 * Features:
 * - Generate new invite codes
 * - Display active invites with usage counters
 * - Copy-to-clipboard invite URLs
 * - Show expiry and max uses
 * - Display player roster with role management
 * - Change member roles (Owner/GM only)
 * - Remove members from campaign
 */
export function CampaignSettingsPanel({ worldId }: CampaignSettingsPanelProps) {
  const [invites, setInvites] = useState<Invite[]>([]);
  const [members, setMembers] = useState<Member[]>([]);
  const [invitesLoading, setInvitesLoading] = useState(true);
  const [membersLoading, setMembersLoading] = useState(true);
  const [isGenerating, setIsGenerating] = useState(false);
  const [copiedCode, setCopiedCode] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [currentUserRole, setCurrentUserRole] = useState<string | null>(null);
  const [changingRoleFor, setChangingRoleFor] = useState<string | null>(null);

  // Load invites and members on mount
  useEffect(() => {
    void loadInvites();
    void loadMembers();
  }, [worldId]);

  const loadInvites = async () => {
    try {
      setInvitesLoading(true);
      const data = await postGraphQL<{ worldInvites: Invite[] }>(
        `
          query worldInvites($world_id: ID!) {
            worldInvites(world_id: $world_id) {
              id
              inviteCode
              maxUses
              usedCount
              expiresAt
              createdAt
            }
          }
        `,
        { world_id: worldId },
      );
      setInvites(data.worldInvites || []);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load invites");
    } finally {
      setInvitesLoading(false);
    }
  };

  const loadMembers = async () => {
    try {
      setMembersLoading(true);
      const data = await postGraphQL<{ worldMembers: Member[] }>(
        `
          query worldMembers($world_id: ID!) {
            worldMembers(world_id: $world_id) {
              id
              userId
              role
              joinedAt
            }
          }
        `,
        { world_id: worldId },
      );
      setMembers(data.worldMembers || []);

      // Determine current user's role (for now, just check if they can manage)
      // In a real app, this would come from auth context
      if (data.worldMembers && data.worldMembers.length > 0) {
        // Assume first member is current user for now (this needs auth context)
        setCurrentUserRole(data.worldMembers[0]?.role);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load members");
    } finally {
      setMembersLoading(false);
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
          mutation generateInviteCode($worldId: ID!, $maxUses: Int!) {
            generateInviteCode(worldId: $worldId, maxUses: $maxUses) {
              inviteCode
            }
          }
        `,
        {
          worldId,
          maxUses: 5,
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

  const canChangeRole = (targetMember: Member): boolean => {
    if (!currentUserRole) return false;
    const roleHierarchy = { Owner: 3, GM: 2, Player: 1 };
    const currentLevel = roleHierarchy[currentUserRole as keyof typeof roleHierarchy] || 0;
    const targetLevel = roleHierarchy[targetMember.role as keyof typeof roleHierarchy] || 0;
    return currentLevel > targetLevel;
  };

  const handleChangeRole = async (memberId: string, newRole: string) => {
    try {
      setError(null);
      setChangingRoleFor(memberId);

      await postGraphQL(
        `
          mutation updateMemberRole($world_id: ID!, $member_id: ID!, $role: String!) {
            updateMemberRole(world_id: $world_id, member_id: $member_id, role: $role) {
              id
              role
            }
          }
        `,
        {
          world_id: worldId,
          member_id: memberId,
          role: newRole,
        },
      );

      // Reload members to show the updated role
      await loadMembers();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to change member role");
    } finally {
      setChangingRoleFor(null);
    }
  };

  const handleRemoveMember = async (memberId: string) => {
    if (!window.confirm("Are you sure you want to remove this member from the campaign?")) {
      return;
    }

    try {
      setError(null);
      setChangingRoleFor(memberId);

      // TODO: Implement removeMember mutation in backend (Phase 4.10.F)
      // For now, this is a placeholder that shows a message
      setError("Member removal coming soon! Please use role management for now.");
      
      // Reload members to refresh state
      await loadMembers();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to remove member");
    } finally {
      setChangingRoleFor(null);
    }
  };

  return (
    <section className={styles.container}>
      <Card surface="parchment" className={styles.card}>
        <div className={styles.header}>
          <div>
            <h2>Campaign Settings</h2>
            <p className={styles.subtitle}>Manage invites and player roster</p>
          </div>
        </div>

        {error && <StatusBadge variant="danger">{error}</StatusBadge>}

        {/* Invite Players Section */}
        <div className={styles.section}>
          <h3>Invite Players</h3>
          <p className={styles.sectionDescription}>
            Generate invite codes to share with other players. Each code allows a specific number of joins.
          </p>

          <Button
            onClick={() => void handleGenerateInvite()}
            disabled={isGenerating}
            icon="link"
          >
            {isGenerating ? "Generating..." : "Generate Invite Code"}
          </Button>

          {invitesLoading ? (
            <Loader label="Loading invites..." />
          ) : invites.length === 0 ? (
            <p className={styles.emptyState}>No active invites yet. Generate one to get started.</p>
          ) : (
            <div className={styles.invitesList}>
              {invites.map((invite) => (
                <div key={invite.id} className={styles.inviteCard}>
                  <div className={styles.inviteHeader}>
                    <code className={styles.inviteCode}>{invite.inviteCode}</code>
                    <span className={styles.inviteStatus}>
                      {getInviteStatus(invite)}
                    </span>
                    {!isInviteValid(invite) && (
                      <StatusBadge variant="warning">Expired</StatusBadge>
                    )}
                  </div>

                  <div className={styles.inviteUrl}>
                    <input
                      type="text"
                      readOnly
                      value={generateInviteUrl(invite.inviteCode)}
                      className={styles.urlInput}
                    />
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => void handleCopyToClipboard(invite.inviteCode)}
                      icon={copiedCode === invite.inviteCode ? "check" : "copy"}
                    >
                      {copiedCode === invite.inviteCode ? "Copied!" : "Copy"}
                    </Button>
                  </div>

                  <div className={styles.inviteMeta}>
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

        {/* Player Roster Section */}
        <div className={styles.section}>
          <h3>Player Roster ({members.length})</h3>
          <p className={styles.sectionDescription}>
            View all players currently joined to this campaign.
          </p>

          {membersLoading ? (
            <Loader label="Loading roster..." />
          ) : members.length === 0 ? (
            <p className={styles.emptyState}>No players yet. Invite someone to join!</p>
          ) : (
            <div className={styles.membersList}>
              {members.map((member) => (
                <div key={member.id} className={styles.memberRow}>
                  <span className={styles.memberRole}>{member.role}</span>
                  <span className={styles.memberJoinDate}>
                    Joined: {new Date(member.joinedAt).toLocaleDateString()}
                  </span>
                  {canChangeRole(member) && (
                    <div className={styles.memberActions}>
                      <select
                        value={member.role}
                        onChange={(e) => void handleChangeRole(member.id, e.target.value)}
                        disabled={changingRoleFor === member.id}
                        className={styles.roleSelect}
                      >
                        <option value="Owner">Owner</option>
                        <option value="GM">GM</option>
                        <option value="Player">Player</option>
                      </select>
                      <Button
                        variant="danger"
                        size="sm"
                        onClick={() => void handleRemoveMember(member.id)}
                        disabled={changingRoleFor === member.id}
                        icon="trash"
                      >
                        Remove
                      </Button>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      </Card>
    </section>
  );
}
