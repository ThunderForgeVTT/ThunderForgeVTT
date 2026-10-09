import { useState } from "react";
import { generateInviteCode } from "@/api/world";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Input } from "@/components/ui/input";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type { WorldInviteDoc } from "@/db/collections/worldInvitesCollection";
import { inviteStateLabel } from "@/db/collections/worldInvitesCollection";
import { useWorldInvites } from "@/hooks/useWorldInvites";
import {
  DEFAULT_LINK_CHOICE,
  EXPIRY_LABELS,
  MAX_LINK_USES,
  describeUses,
  linkOptionsFrom,
  splitLinks,
  type LinkChoice,
  type LinkExpiry,
} from "@/pages/world/players/worldLinks";

export interface WorldLinksPanelProps {
  worldId: string;
  /** The display name of a link's creator, when the roster knows them. */
  creatorName: (userId: string) => string | undefined;
}

const SELECT_CLASS =
  "h-8 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50";

const NO_LIMIT = "none";

const linkUrl = (code: string) => `${window.location.origin}/join/${code}`;

const stateVariant = (state: string) =>
  state === "ACTIVE" ? "success" : state === "REVOKED" ? "danger" : "warning";

/**
 * Spec 088 (US1, FR-001 to FR-005): the world's links, on the players page,
 * for those who run the world. The page draws this only for them, and the
 * server refuses `worldInvites` and `generateInviteCode` to anyone else.
 *
 * A link admits existing ThunderForge accounts only. It has no use limit
 * unless the GM sets one, and lives 7 days unless they choose otherwise.
 * The list follows the world's events, so a join or a revoke shows here
 * without a reload.
 */
export function WorldLinksPanel({
  worldId,
  creatorName,
}: WorldLinksPanelProps) {
  const { invites, loading, error, refetch, revoke, rotate } =
    useWorldInvites(worldId);
  const [choice, setChoice] = useState<LinkChoice>(DEFAULT_LINK_CHOICE);
  const [isGenerating, setIsGenerating] = useState(false);
  const [copiedCode, setCopiedCode] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [confirmingId, setConfirmingId] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const copyLink = async (code: string) => {
    try {
      await navigator.clipboard.writeText(linkUrl(code));
      setCopiedCode(code);
      setTimeout(() => setCopiedCode(null), 2000);
    } catch {
      setActionError("Could not copy to the clipboard.");
    }
  };

  const handleGenerate = async () => {
    setActionError(null);
    setIsGenerating(true);
    try {
      const created = await generateInviteCode(
        worldId,
        linkOptionsFrom(choice, new Date()),
      );
      await copyLink(created.inviteCode);
      await refetch();
    } catch (err) {
      setActionError(
        err instanceof Error ? err.message : "Could not make the link.",
      );
    } finally {
      setIsGenerating(false);
    }
  };

  const handleRotate = async (inviteId: string) => {
    setActionError(null);
    setBusyId(inviteId);
    try {
      await copyLink(await rotate(inviteId));
    } catch (err) {
      setActionError(
        err instanceof Error ? err.message : "Could not refresh the link.",
      );
    } finally {
      setBusyId(null);
    }
  };

  const handleRevoke = async (inviteId: string) => {
    setActionError(null);
    setBusyId(inviteId);
    try {
      await revoke(inviteId);
      setConfirmingId(null);
    } catch (err) {
      setActionError(
        err instanceof Error ? err.message : "Could not revoke the link.",
      );
    } finally {
      setBusyId(null);
    }
  };

  const { active, past } = splitLinks(invites);
  const shownError = actionError ?? error?.message ?? null;

  const renderLink = (invite: WorldInviteDoc) => {
    const isBusy = busyId === invite.id;
    const isRevoked = invite.state === "REVOKED";
    const creator = creatorName(invite.created_by);
    return (
      <div
        key={invite.id}
        className="grid gap-3 rounded-lg border border-border p-4"
        data-testid="invite-link-row"
        data-invite-state={invite.state}
      >
        <div className="flex flex-wrap items-center gap-2">
          {/* Wrapped: StatusBadge drops props it does not know. */}
          <span data-testid="invite-link-state">
            <StatusBadge variant={stateVariant(invite.state)}>
              {inviteStateLabel(invite.state)}
            </StatusBadge>
          </span>
          <span
            className="text-sm text-muted-foreground"
            data-testid="invite-link-uses"
          >
            {describeUses(invite)}
          </span>
        </div>

        <div className="flex flex-wrap gap-2">
          <Input
            type="text"
            readOnly
            value={linkUrl(invite.invite_code)}
            aria-label="Invite link"
          />
          <Button
            variant="secondary"
            size="sm"
            disabled={invite.state !== "ACTIVE"}
            onClick={() => void copyLink(invite.invite_code)}
            icon={copiedCode === invite.invite_code ? "check" : "copy"}
          >
            {copiedCode === invite.invite_code ? "Copied!" : "Copy link"}
          </Button>
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
            (confirmingId === invite.id ? (
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
                  onClick={() => setConfirmingId(null)}
                >
                  Cancel
                </Button>
              </>
            ) : (
              <Button
                variant="ghost"
                size="sm"
                disabled={isBusy}
                onClick={() => setConfirmingId(invite.id)}
                data-testid="invite-link-revoke"
              >
                Revoke
              </Button>
            ))}
        </div>

        {confirmingId === invite.id && (
          <p className="text-xs text-muted-foreground">
            This cannot be undone. Anyone who already joined with this link
            keeps their place; only future joins are stopped.
          </p>
        )}

        <div className="flex flex-wrap gap-4 text-xs text-muted-foreground">
          <span>
            Made {new Date(invite.created_at).toLocaleDateString()}
            {creator ? ` by ${creator}` : ""}
          </span>
          <span data-testid="invite-link-expiry">
            {invite.expires_at
              ? `Expires ${new Date(invite.expires_at).toLocaleString()}`
              : "Never expires"}
          </span>
        </div>
      </div>
    );
  };

  return (
    <Card className="grid gap-4 p-4" data-testid="world-links-panel">
      <div className="grid gap-1">
        <h2 className="text-lg font-semibold">World links</h2>
        <p className="text-sm text-muted-foreground">
          A link lets someone who already has a ThunderForge account join this
          world as a player. It cannot be used to create an account.
        </p>
      </div>

      {shownError ? (
        <StatusBadge variant="danger">{shownError}</StatusBadge>
      ) : null}

      <div className="flex flex-wrap items-end gap-3">
        <label className="grid gap-1 text-sm">
          <span className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
            Uses
          </span>
          <select
            className={SELECT_CLASS}
            value={choice.limit === null ? NO_LIMIT : String(choice.limit)}
            onChange={(event) =>
              setChoice((current) => ({
                ...current,
                limit:
                  event.target.value === NO_LIMIT
                    ? null
                    : Number(event.target.value),
              }))
            }
            data-testid="world-link-limit"
          >
            <option value={NO_LIMIT}>No limit</option>
            {Array.from({ length: MAX_LINK_USES }, (_, i) => i + 1).map((n) => (
              <option key={n} value={n}>
                {n === 1 ? "1 use" : `${n} uses`}
              </option>
            ))}
          </select>
        </label>
        <label className="grid gap-1 text-sm">
          <span className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
            Expires after
          </span>
          <select
            className={SELECT_CLASS}
            value={choice.expiry}
            onChange={(event) =>
              setChoice((current) => ({
                ...current,
                expiry: event.target.value as LinkExpiry,
              }))
            }
            data-testid="world-link-expiry"
          >
            {(Object.keys(EXPIRY_LABELS) as LinkExpiry[]).map((key) => (
              <option key={key} value={key}>
                {EXPIRY_LABELS[key]}
              </option>
            ))}
          </select>
        </label>
        <Button
          onClick={() => void handleGenerate()}
          disabled={isGenerating}
          icon="link"
        >
          {isGenerating ? "Generating..." : "Generate Join Link"}
        </Button>
      </div>

      {loading && invites.length === 0 ? (
        <Loader label="Loading links..." />
      ) : active.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          No working links. Make one to invite a player.
        </p>
      ) : (
        <div className="grid gap-3" data-testid="invite-link-list">
          {active.map(renderLink)}
        </div>
      )}

      {past.length > 0 ? (
        <details data-testid="world-links-past">
          <summary className="cursor-pointer text-sm font-semibold">
            Past links ({past.length})
          </summary>
          <div className="mt-3 grid gap-3">{past.map(renderLink)}</div>
        </details>
      ) : null}
    </Card>
  );
}
