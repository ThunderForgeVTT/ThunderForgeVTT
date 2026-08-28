import { useState, useEffect } from "react";
import {
  generateInviteCode,
  getWorld,
  updateWorldAllowPlayerCreatedActors,
} from "@/api/world";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Input } from "@/components/ui/input";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { inviteStateLabel } from "@/db/collections/worldInvitesCollection";
import { useWorldInvites } from "@/hooks/useWorldInvites";

interface CampaignSettingsPanelProps {
  worldId: string;
}

/**
 * CampaignSettingsPanel manages invites and world-level campaign settings.
 *
 * Features:
 * - Generate new invite codes
 * - Display active invites with usage counters
 * - Copy-to-clipboard invite URLs
 * - Show each link's real state, and revoke or refresh it
 * Spec 023 (FR-011): the player roster and its role-change/remove-member
 * controls moved to the dedicated Players sidebar section
 * (`PlayersPage.tsx`) — this panel no longer duplicates them.
 */
export function CampaignSettingsPanel({ worldId }: CampaignSettingsPanelProps) {
  // Spec 027: this panel used to run its own inline query through a private
  // `postGraphQL` copy. It now shares `useWorldInvites`, so there is one place
  // that knows how to read a link's state — and it goes through the hardened
  // transport like everything else.
  const {
    invites,
    loading: invitesLoading,
    error: invitesError,
    refetch: loadInvites,
    revoke,
    rotate,
  } = useWorldInvites(worldId);

  const [isGenerating, setIsGenerating] = useState(false);
  const [copiedCode, setCopiedCode] = useState<string | null>(null);
  const [busyInviteId, setBusyInviteId] = useState<string | null>(null);
  const [confirmingRevokeId, setConfirmingRevokeId] = useState<string | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const [allowPlayerCreatedActors, setAllowPlayerCreatedActors] =
    useState(false);
  const [isUpdatingAllowSetting, setIsUpdatingAllowSetting] = useState(false);

  useEffect(() => {
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

  const generateInviteUrl = (code: string) =>
    `${window.location.origin}/join/${code}`;

  const copyLink = async (code: string) => {
    try {
      await navigator.clipboard.writeText(generateInviteUrl(code));
      setCopiedCode(code);
      setTimeout(() => setCopiedCode(null), 2000);
    } catch {
      setError("Failed to copy to clipboard");
    }
  };

  const handleGenerateInvite = async () => {
    setError(null);
    setIsGenerating(true);
    try {
      const created = await generateInviteCode(worldId, 5);
      await copyLink(created.inviteCode);
      await loadInvites();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to generate invite code",
      );
    } finally {
      setIsGenerating(false);
    }
  };

  /**
   * Spec 027 (FR-003): the replacement code is copied straight to the
   * clipboard. A GM refreshes a link precisely because they need to hand out a
   * new one, so making them hunt for it afterwards misses the point.
   */
  const handleRotate = async (inviteId: string) => {
    setError(null);
    setBusyInviteId(inviteId);
    try {
      const newCode = await rotate(inviteId);
      await copyLink(newCode);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to refresh this link",
      );
    } finally {
      setBusyInviteId(null);
    }
  };

  const handleRevoke = async (inviteId: string) => {
    setError(null);
    setBusyInviteId(inviteId);
    try {
      await revoke(inviteId);
      setConfirmingRevokeId(null);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to revoke this link",
      );
    } finally {
      setBusyInviteId(null);
    }
  };

  const stateVariant = (state: string) =>
    state === "ACTIVE" ? "success" : state === "REVOKED" ? "danger" : "warning";

  const displayError = error ?? invitesError?.message ?? null;

  return (
    <section>
      <Card surface="parchment" className="grid gap-6 p-6">
        <div>
          <h2 className="text-xl font-semibold">Campaign Settings</h2>
          <p className="text-muted-foreground">
            Manage invites and player-created character settings
          </p>
        </div>

        {displayError && (
          <StatusBadge variant="danger">{displayError}</StatusBadge>
        )}

        {/* Invite Players Section */}
        <div className="grid gap-3">
          <h3 className="font-semibold">Invite Players</h3>
          <p className="text-sm text-muted-foreground">
            Generate join links to share with other players. Each link allows a
            specific number of joins.
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
            <div className="grid gap-3" data-testid="invite-link-list">
              {invites.map((invite) => {
                const isBusy = busyInviteId === invite.id;
                const isRevoked = invite.state === "REVOKED";
                return (
                  <div
                    key={invite.id}
                    className="grid gap-3 rounded-lg border border-border p-4"
                    data-testid="invite-link-row"
                    data-invite-state={invite.state}
                  >
                    <div className="flex flex-wrap items-center gap-2">
                      {/* Spec 027 (FR-010): the real state, not a bare
                          "3/10 uses" string. Before this, a revoked link
                          rendered identically to a working one, and anything
                          unusable was labelled "Expired" regardless of why. */}
                      {/* Wrapped rather than passing `data-testid` to
                          StatusBadge: that component takes only children,
                          variant and className, so extra props are silently
                          dropped and the hook never reaches the DOM. */}
                      <span data-testid="invite-link-state">
                        <StatusBadge variant={stateVariant(invite.state)}>
                          {inviteStateLabel(invite.state)}
                        </StatusBadge>
                      </span>
                      <span className="text-sm text-muted-foreground">
                        {invite.remaining_uses === null ||
                        invite.remaining_uses === undefined
                          ? `${invite.used_count} joins`
                          : `${invite.remaining_uses} of ${invite.max_uses} uses left`}
                      </span>
                      {invite.rotated_from && (
                        <span className="text-xs text-muted-foreground italic">
                          replaced an earlier link
                        </span>
                      )}
                    </div>

                    <div className="flex flex-wrap gap-2">
                      <Input
                        type="text"
                        readOnly
                        value={generateInviteUrl(invite.invite_code)}
                        aria-label="Invite link"
                      />
                      <Button
                        variant="secondary"
                        size="sm"
                        disabled={isRevoked}
                        onClick={() => void copyLink(invite.invite_code)}
                        icon={
                          copiedCode === invite.invite_code ? "check" : "copy"
                        }
                      >
                        {copiedCode === invite.invite_code
                          ? "Copied!"
                          : "Copy link"}
                      </Button>
                      {/* Refresh works on an expired or exhausted link too —
                          a GM can always revive a dead one. Only an already
                          revoked link has nothing left to rotate. */}
                      <Button
                        variant="secondary"
                        size="sm"
                        disabled={isBusy || isRevoked}
                        onClick={() => void handleRotate(invite.id)}
                        icon="spark"
                        data-testid="invite-link-refresh"
                      >
                        {isBusy ? "Working…" : "Refresh"}
                      </Button>
                      {!isRevoked &&
                        (confirmingRevokeId === invite.id ? (
                          <>
                            <Button
                              variant="danger"
                              size="sm"
                              disabled={isBusy}
                              onClick={() => void handleRevoke(invite.id)}
                              data-testid="invite-link-revoke-confirm"
                            >
                              Revoke permanently
                            </Button>
                            <Button
                              variant="ghost"
                              size="sm"
                              disabled={isBusy}
                              onClick={() => setConfirmingRevokeId(null)}
                            >
                              Cancel
                            </Button>
                          </>
                        ) : (
                          <Button
                            variant="ghost"
                            size="sm"
                            disabled={isBusy}
                            onClick={() => setConfirmingRevokeId(invite.id)}
                            data-testid="invite-link-revoke"
                          >
                            Revoke
                          </Button>
                        ))}
                    </div>

                    {confirmingRevokeId === invite.id && (
                      <p className="text-xs text-muted-foreground">
                        This cannot be undone. Anyone who already joined with
                        this link keeps their place — only future joins are
                        stopped.
                      </p>
                    )}

                    <div className="flex gap-4 text-xs text-muted-foreground">
                      <span>
                        Created:{" "}
                        {new Date(invite.created_at).toLocaleDateString()}
                      </span>
                      {invite.expires_at && (
                        <span>
                          Expires:{" "}
                          {new Date(invite.expires_at).toLocaleDateString()}
                        </span>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Spec 017 (FR-007): player-created character setting */}
        <div className="grid gap-3">
          <h3 className="font-semibold">Player-created characters</h3>
          <p className="text-sm text-muted-foreground">
            When on, a joining player without a GM-designated character can
            create their own on the Actor Selection screen. Off by default.
          </p>
          <label
            className="flex items-center gap-2 text-sm"
            data-testid="allow-player-created-actors-toggle"
          >
            <input
              type="checkbox"
              checked={allowPlayerCreatedActors}
              disabled={isUpdatingAllowSetting}
              onChange={(e) =>
                void handleToggleAllowPlayerCreatedActors(e.target.checked)
              }
            />
            Allow players to create their own actors
          </label>
        </div>
      </Card>
    </section>
  );
}
