import React, { useCallback, useEffect, useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import * as Popover from "@radix-ui/react-popover";
import {
  createToken,
  deleteToken,
  getTokens,
  setOwnPrimaryTokenPhoto,
  updateToken,
} from "../api/tokens";
import { getWorldActors } from "../api/actors";
import { getGameSystemManifest } from "../api/gameSystems";
import { useActorSystemData } from "../hooks/useActorSystemData";
import {
  resolveSizeScale,
  type SizeCategoriesLookup,
} from "../utils/sizeCategory";
import { readString } from "../lib/systemData";
import { TOKEN_TYPES, type TokenRecord, type TokenType } from "../types/token";
import type { WorldActorRecord } from "../types/actor";
import "../styles/TokenPanel.scss";

interface TokenPanelProps {
  sceneId: string;
  /** Current authenticated user's id, for gating primary-photo edits. */
  currentUserId: string | null;
  /** Whether the current user is this scene's GM (owner). */
  isSceneOwner: boolean;
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
  /** World this scene belongs to — used only to offer an NPC picker on
   * token creation (spec 018 T047: staging a Genie NPC of a given
   * `size_category` defaults its token's `scale` per the NPC's game
   * system manifest `sizeCategories` lookup, research.md R6). Omit to
   * keep the plain blank-token creation flow. */
  worldId?: string;
}

/**
 * TokenPanel: React component for managing scene-scoped tokens.
 *
 * Spec 004 / ADR-040: rewired off the legacy world-scoped `world_tokens`
 * table onto the same `tokens` table the canvas engine renders/drags
 * (src/server/src/graphql/mutations_tokens.rs) — moving a token here and
 * dragging it on the canvas are now the same row, not two disconnected
 * ones. Bulk create/delete and health-bar editing remain GM-only; a
 * non-GM player only ever sees/edits their own primary token's photo.
 */
export const TokenPanel: React.FC<TokenPanelProps> = ({
  sceneId,
  currentUserId,
  isSceneOwner,
  isOpen,
  onOpenChange,
  worldId,
}) => {
  const [tokens, setTokens] = useState<TokenRecord[]>([]);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [newTokenHealth, setNewTokenHealth] = useState<number | undefined>();
  const [newTokenMaxHealth, setNewTokenMaxHealth] = useState<
    number | undefined
  >();
  const [selectedTokenId, setSelectedTokenId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Spec 018 T047: NPC roster + size-category -> scale resolution for the
  // "Create Token" dialog's optional NPC picker. Kept fully optional and
  // additive — a blank `newTokenActorId` behaves exactly like before
  // (createToken called with no `actorId`/`scale` override).
  const [npcActors, setNpcActors] = useState<WorldActorRecord[]>([]);
  const [newTokenActorId, setNewTokenActorId] = useState<string>("");
  // `null` means "whatever the NPC selection implies"; a value means the
  // Game Master said otherwise. Keeping the two apart avoids the usual
  // override-tracking flag: staging an NPC still defaults the kind to `npc`,
  // and an explicit choice keeps winning even if the NPC selection changes
  // underneath it.
  const [newTokenTypeChoice, setNewTokenTypeChoice] =
    useState<TokenType | null>(null);
  // Stored with the system it was fetched for, so "which size categories
  // apply right now" is derived during render instead of being reset from
  // inside the effect below — a manifest from a previously-picked NPC's
  // system can never be read as this one's.
  const [loadedSizeCategories, setLoadedSizeCategories] = useState<{
    gameSystemId: string;
    categories: SizeCategoriesLookup | undefined;
  } | null>(null);

  const selectedActor = npcActors.find((a) => a.id === newTokenActorId) ?? null;
  const { data: selectedActorSystemData } = useActorSystemData(
    newTokenActorId,
    selectedActor?.gameSystemId ?? undefined,
  );

  useEffect(() => {
    if (!isOpen || !worldId || !isSceneOwner) return;
    getWorldActors(worldId)
      .then((actors) => setNpcActors(actors.filter((a) => a.isNpc)))
      .catch((err) => {
        console.error("Failed to load NPC roster for token creation:", err);
      });
  }, [isOpen, worldId, isSceneOwner]);

  const selectedGameSystemId = selectedActor?.gameSystemId;
  const sizeCategories =
    selectedGameSystemId &&
    loadedSizeCategories?.gameSystemId === selectedGameSystemId
      ? loadedSizeCategories.categories
      : undefined;

  useEffect(() => {
    if (!selectedGameSystemId) {
      return;
    }
    let active = true;
    getGameSystemManifest(selectedGameSystemId)
      .then((manifest) => {
        if (active) {
          setLoadedSizeCategories({
            gameSystemId: selectedGameSystemId,
            categories: manifest.sizeCategories as
              | SizeCategoriesLookup
              | undefined,
          });
        }
      })
      .catch((err) => {
        console.error(
          "Failed to load game system manifest for token scale:",
          err,
        );
        if (active) {
          setLoadedSizeCategories({
            gameSystemId: selectedGameSystemId,
            categories: undefined,
          });
        }
      });
    return () => {
      active = false;
    };
  }, [selectedGameSystemId]);

  /** The `scale` a new token defaults to given the currently-selected NPC
   * (if any) — `undefined` when no NPC is selected, so `createToken` falls
   * back to the server's existing default rather than forcing a value. */
  /**
   * The kind a new token will be created as.
   *
   * Staging an NPC implies an NPC token — the same inference the column's
   * backfill made for existing rows, so the two agree rather than each
   * guessing separately.
   */
  const effectiveTokenType: TokenType =
    newTokenTypeChoice ?? (newTokenActorId ? "npc" : "character");

  const resolvedNewTokenScale: number | undefined = newTokenActorId
    ? resolveSizeScale(
        sizeCategories,
        readString(selectedActorSystemData?.trait_data, "size_category") ??
          null,
      )
    : undefined;
  // Root cause of the primary-checkbox hang (spec 006 US2, found via
  // live instrumentation — see research.md §3): the checkbox's `disabled`
  // was gated on `token.ownerUserId`, which only flips (via `refresh()`)
  // after the owner-assignment mutation's network round trip resolves.
  // Tabbing from the owner input to the checkbox happens synchronously,
  // in the same tick as that Tab keypress — well before the round trip
  // resolves. A *disabled* element is not in the native tab order, so
  // the browser's real Tab traversal skipped straight past the
  // still-disabled checkbox to whatever came after it in the DOM,
  // handing focus to something outside `Popover.Content` (e.g. the next
  // token's own trigger). Radix's default outside-focus dismissal then
  // read that as "focus left the popover" and closed it — with the
  // checkbox's own click/check event never reaching an element the
  // popover still considered live, hence the hang. Tracking each
  // owner-input's live typed value here (independent of the
  // mutation/refetch cycle) lets the checkbox enable itself the instant
  // there's a non-empty value, well before Tab is ever pressed.
  const [ownerDrafts, setOwnerDrafts] = useState<Record<string, string>>({});

  const refresh = useCallback(() => {
    if (!sceneId) return;
    getTokens(sceneId)
      .then(setTokens)
      .catch((err) => {
        console.error("Failed to load scene tokens:", err);
      });
  }, [sceneId]);

  useEffect(() => {
    if (!isOpen) return;
    refresh();
  }, [isOpen, refresh]);

  const handleCreateToken = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      await createToken({
        sceneId,
        x: 0,
        y: 0,
        health: newTokenHealth,
        maxHealth: newTokenMaxHealth,
        // Spec 018 T047: a token created for a selected NPC defaults to
        // that NPC's size-category scale (resolveSizeScale, ../utils/
        // sizeCategory.ts) rather than the server's plain default.
        actorId: newTokenActorId || undefined,
        scale: resolvedNewTokenScale,
        tokenType: effectiveTokenType,
      });
      setNewTokenHealth(undefined);
      setNewTokenMaxHealth(undefined);
      setNewTokenActorId("");
      setNewTokenTypeChoice(null);
      setCreateDialogOpen(false);
      refresh();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Unknown error creating token",
      );
    } finally {
      setLoading(false);
    }
  }, [
    sceneId,
    newTokenHealth,
    newTokenMaxHealth,
    newTokenActorId,
    resolvedNewTokenScale,
    effectiveTokenType,
    refresh,
  ]);

  const handleDeleteToken = useCallback(
    async (tokenId: string) => {
      if (!window.confirm("Delete this token?")) return;
      setLoading(true);
      setError(null);
      try {
        await deleteToken(tokenId);
        setSelectedTokenId(null);
        refresh();
      } catch (err) {
        setError(
          err instanceof Error ? err.message : "Unknown error deleting token",
        );
      } finally {
        setLoading(false);
      }
    },
    [refresh],
  );

  const handleSetPrimaryPhoto = useCallback(
    async (tokenId: string, photoUrl: string) => {
      setLoading(true);
      setError(null);
      try {
        await setOwnPrimaryTokenPhoto(tokenId, photoUrl);
        refresh();
      } catch (err) {
        setError(
          err instanceof Error ? err.message : "Unknown error setting photo",
        );
      } finally {
        setLoading(false);
      }
    },
    [refresh],
  );

  /** GM-only: grant/revoke a player's control of a token, or (re)designate
   * their primary token. Reuses the full-control `updateToken` mutation. */
  const handleSetOwnership = useCallback(
    async (tokenId: string, ownerUserId: string | null, isPrimary: boolean) => {
      setLoading(true);
      setError(null);
      // Optimistic local update: `checked`/`value` below are fully
      // server-state-controlled with no local buffer, so without this
      // there is a real (if brief) window — the network round trip
      // between click and `refresh()` resolving — where the checkbox
      // visibly reverts to its old state before snapping to the new
      // one. Found live while writing this feature's e2e coverage
      // (spec 004 T022): a real user could plausibly click again during
      // that window, thinking the first click didn't register.
      let rollbackTokens: TokenRecord[] | null = null;
      setTokens((prev) => {
        rollbackTokens = prev;
        return prev.map((t) => {
          if (t.tokenId === tokenId) {
            return { ...t, ownerUserId, isPrimary };
          }
          // Mirror the server's "exactly one primary per (scene, owner)"
          // invariant locally too, so a second token belonging to the
          // same owner doesn't transiently also show as primary.
          if (isPrimary && ownerUserId && t.ownerUserId === ownerUserId) {
            return { ...t, isPrimary: false };
          }
          return t;
        });
      });
      try {
        await updateToken(tokenId, {
          ownerUserId: ownerUserId ?? undefined,
          isPrimary,
        });
        refresh();
      } catch (err) {
        if (rollbackTokens) setTokens(rollbackTokens);
        setError(
          err instanceof Error
            ? err.message
            : "Unknown error updating ownership",
        );
      } finally {
        setLoading(false);
      }
    },
    [refresh],
  );

  const getTokenAvatar = (token: TokenRecord): string =>
    token.photoUrl ??
    `https://api.dicebear.com/9.x/adventurer-neutral/svg?seed=${token.tokenId}`;

  const getHealthPercentage = (
    health?: number | null,
    maxHealth?: number | null,
  ): number => {
    if (health == null || maxHealth == null || maxHealth <= 0) return 0;
    return (health / maxHealth) * 100;
  };

  return (
    <Dialog.Root open={isOpen} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="token-panel-overlay" />
        <Dialog.Content className="token-panel-content">
          <Dialog.Title className="token-panel-title">
            Token Management
          </Dialog.Title>
          <Dialog.Description className="token-panel-description">
            Create, manage, and position tokens on this scene.
          </Dialog.Description>

          {error && <div className="token-panel-error">{error}</div>}

          <div className="token-list">
            {tokens.length === 0 ? (
              <div className="token-list-empty">
                No tokens yet. Create one to get started.
              </div>
            ) : (
              tokens.map((token) => {
                const isMyPrimary =
                  token.isPrimary && token.ownerUserId === currentUserId;
                const canEditPhoto = isSceneOwner || isMyPrimary;

                return (
                  <Popover.Root
                    key={token.tokenId}
                    open={selectedTokenId === token.tokenId}
                    onOpenChange={(open) =>
                      setSelectedTokenId(open ? token.tokenId : null)
                    }
                  >
                    <Popover.Trigger asChild>
                      <div
                        className="token-list-item"
                        role="button"
                        data-testid={`token-list-item-${token.tokenId}`}
                      >
                        <img
                          src={getTokenAvatar(token)}
                          alt="Token"
                          className="token-avatar"
                        />
                        <div className="token-info">
                          <div className="token-label">
                            {token.isPrimary ? "Primary — " : ""}
                            Token {token.tokenId.slice(0, 8)}
                          </div>
                          {token.health != null && (
                            <div className="token-health">
                              <div className="health-bar-container">
                                <div
                                  className="health-bar"
                                  style={{
                                    width: `${getHealthPercentage(token.health, token.maxHealth)}%`,
                                  }}
                                />
                              </div>
                              <span className="health-text">
                                {token.health}/{token.maxHealth}
                              </span>
                            </div>
                          )}
                        </div>
                      </div>
                    </Popover.Trigger>

                    <Popover.Content className="token-popover-content">
                      <div className="token-details">
                        <h4>Token Details</h4>
                        <p data-testid={`token-position-${token.tokenId}`}>
                          Position: ({token.x.toFixed(1)}, {token.y.toFixed(1)})
                        </p>
                        <p>ID: {token.tokenId}</p>

                        {canEditPhoto && (
                          <div className="form-group">
                            <label htmlFor={`photo-${token.tokenId}`}>
                              Photo URL
                            </label>
                            <input
                              id={`photo-${token.tokenId}`}
                              data-testid={`token-photo-input-${token.tokenId}`}
                              type="text"
                              defaultValue={token.photoUrl ?? ""}
                              placeholder="https://..."
                              onBlur={(e) => {
                                const value = e.target.value.trim();
                                if (value)
                                  void handleSetPrimaryPhoto(
                                    token.tokenId,
                                    value,
                                  );
                              }}
                            />
                          </div>
                        )}

                        {isSceneOwner && (
                          <div className="form-group">
                            <label htmlFor={`owner-${token.tokenId}`}>
                              Owner user ID
                            </label>
                            <input
                              id={`owner-${token.tokenId}`}
                              data-testid={`token-owner-input-${token.tokenId}`}
                              type="text"
                              defaultValue={token.ownerUserId ?? ""}
                              placeholder="(unassigned)"
                              onChange={(e) => {
                                const value = e.target.value;
                                setOwnerDrafts((prev) => ({
                                  ...prev,
                                  [token.tokenId]: value,
                                }));
                              }}
                              onBlur={(e) => {
                                const value = e.target.value.trim();
                                void handleSetOwnership(
                                  token.tokenId,
                                  value || null,
                                  token.isPrimary,
                                );
                              }}
                            />
                            <label>
                              <input
                                type="checkbox"
                                data-testid={`token-primary-checkbox-${token.tokenId}`}
                                checked={token.isPrimary}
                                disabled={
                                  !(
                                    ownerDrafts[token.tokenId] ??
                                    token.ownerUserId ??
                                    ""
                                  ).trim()
                                }
                                onChange={(e) =>
                                  void handleSetOwnership(
                                    token.tokenId,
                                    token.ownerUserId,
                                    e.target.checked,
                                  )
                                }
                              />
                              Primary token for this owner
                            </label>
                          </div>
                        )}

                        {isSceneOwner && (
                          <button
                            className="token-delete-button"
                            onClick={() => handleDeleteToken(token.tokenId)}
                            disabled={loading}
                          >
                            {loading ? "Deleting..." : "Delete"}
                          </button>
                        )}
                      </div>
                    </Popover.Content>
                  </Popover.Root>
                );
              })
            )}
          </div>

          {isSceneOwner && (
            <Dialog.Root
              open={createDialogOpen}
              onOpenChange={setCreateDialogOpen}
            >
              <Dialog.Trigger asChild>
                <button
                  className="token-create-button"
                  data-testid="token-create-trigger"
                  disabled={loading}
                >
                  {loading ? "Creating..." : "+ Create Token"}
                </button>
              </Dialog.Trigger>

              <Dialog.Portal>
                <Dialog.Overlay className="token-panel-overlay" />
                <Dialog.Content className="token-create-dialog">
                  <Dialog.Title>Create New Token</Dialog.Title>
                  <Dialog.Description>
                    Fill in token details. Leave blank for defaults.
                  </Dialog.Description>

                  <div className="token-form">
                    {npcActors.length > 0 && (
                      <div className="form-group">
                        <label htmlFor="token-npc">Stage NPC (optional)</label>
                        <select
                          id="token-npc"
                          data-testid="token-create-npc-select"
                          value={newTokenActorId}
                          onChange={(e) => setNewTokenActorId(e.target.value)}
                        >
                          <option value="">(blank token)</option>
                          {npcActors.map((actor) => (
                            <option key={actor.id} value={actor.id}>
                              {actor.label}
                            </option>
                          ))}
                        </select>
                        {newTokenActorId && (
                          <p
                            className="token-npc-scale-hint"
                            data-testid="token-create-npc-scale-hint"
                          >
                            Default token scale: {resolvedNewTokenScale}x
                          </p>
                        )}
                      </div>
                    )}

                    <div className="form-group">
                      <label htmlFor="token-type">Token type</label>
                      <select
                        id="token-type"
                        data-testid="token-create-type-select"
                        value={effectiveTokenType}
                        onChange={(e) =>
                          setNewTokenTypeChoice(e.target.value as TokenType)
                        }
                      >
                        {TOKEN_TYPES.map((kind) => (
                          <option key={kind.value} value={kind.value}>
                            {kind.label}
                          </option>
                        ))}
                      </select>
                      <p className="token-type-hint">
                        Tokens without art are drawn in a distinct colour per
                        type, so a crowded map can be read at a glance.
                      </p>
                    </div>

                    <div className="form-group">
                      <label htmlFor="token-health">Current Health</label>
                      <input
                        id="token-health"
                        type="number"
                        value={newTokenHealth ?? ""}
                        onChange={(e) =>
                          setNewTokenHealth(
                            e.target.value
                              ? parseInt(e.target.value, 10)
                              : undefined,
                          )
                        }
                        placeholder="e.g., 50"
                      />
                    </div>

                    <div className="form-group">
                      <label htmlFor="token-max-health">Max Health</label>
                      <input
                        id="token-max-health"
                        type="number"
                        value={newTokenMaxHealth ?? ""}
                        onChange={(e) =>
                          setNewTokenMaxHealth(
                            e.target.value
                              ? parseInt(e.target.value, 10)
                              : undefined,
                          )
                        }
                        placeholder="e.g., 100"
                      />
                    </div>
                  </div>

                  <div className="token-dialog-actions">
                    <button
                      className="button-cancel"
                      onClick={() => setCreateDialogOpen(false)}
                      disabled={loading}
                    >
                      Cancel
                    </button>
                    <button
                      className="button-create"
                      data-testid="token-create-submit"
                      onClick={handleCreateToken}
                      disabled={loading}
                    >
                      {loading ? "Creating..." : "Create Token"}
                    </button>
                  </div>
                </Dialog.Content>
              </Dialog.Portal>
            </Dialog.Root>
          )}

          <Dialog.Close asChild>
            <button className="token-panel-close" aria-label="Close">
              ✕
            </button>
          </Dialog.Close>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
};
