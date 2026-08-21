import React, { useCallback, useEffect, useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import * as Popover from '@radix-ui/react-popover';
import {
  createToken,
  deleteToken,
  getTokens,
  setOwnPrimaryTokenPhoto,
  updateToken,
} from '../api/tokens';
import type { TokenRecord } from '../types/token';
import '../styles/TokenPanel.scss';

interface TokenPanelProps {
  sceneId: string;
  /** Current authenticated user's id, for gating primary-photo edits. */
  currentUserId: string | null;
  /** Whether the current user is this scene's GM (owner). */
  isSceneOwner: boolean;
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
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
}) => {
  const [tokens, setTokens] = useState<TokenRecord[]>([]);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [newTokenHealth, setNewTokenHealth] = useState<number | undefined>();
  const [newTokenMaxHealth, setNewTokenMaxHealth] = useState<number | undefined>();
  const [selectedTokenId, setSelectedTokenId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    if (!sceneId) return;
    getTokens(sceneId)
      .then(setTokens)
      .catch((err) => {
        console.error('Failed to load scene tokens:', err);
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
      });
      setNewTokenHealth(undefined);
      setNewTokenMaxHealth(undefined);
      setCreateDialogOpen(false);
      refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unknown error creating token');
    } finally {
      setLoading(false);
    }
  }, [sceneId, newTokenHealth, newTokenMaxHealth, refresh]);

  const handleDeleteToken = useCallback(
    async (tokenId: string) => {
      if (!window.confirm('Delete this token?')) return;
      setLoading(true);
      setError(null);
      try {
        await deleteToken(tokenId);
        setSelectedTokenId(null);
        refresh();
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Unknown error deleting token');
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
        setError(err instanceof Error ? err.message : 'Unknown error setting photo');
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
      try {
        await updateToken(tokenId, {
          ownerUserId: ownerUserId ?? undefined,
          isPrimary,
        });
        refresh();
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Unknown error updating ownership');
      } finally {
        setLoading(false);
      }
    },
    [refresh],
  );

  const getTokenAvatar = (token: TokenRecord): string =>
    token.photoUrl ?? `https://api.dicebear.com/9.x/adventurer-neutral/svg?seed=${token.tokenId}`;

  const getHealthPercentage = (health?: number | null, maxHealth?: number | null): number => {
    if (health == null || maxHealth == null || maxHealth <= 0) return 0;
    return (health / maxHealth) * 100;
  };

  return (
    <Dialog.Root open={isOpen} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="token-panel-overlay" />
        <Dialog.Content className="token-panel-content">
          <Dialog.Title className="token-panel-title">Token Management</Dialog.Title>
          <Dialog.Description className="token-panel-description">
            Create, manage, and position tokens on this scene.
          </Dialog.Description>

          {error && <div className="token-panel-error">{error}</div>}

          <div className="token-list">
            {tokens.length === 0 ? (
              <div className="token-list-empty">No tokens yet. Create one to get started.</div>
            ) : (
              tokens.map((token) => {
                const isMyPrimary = token.isPrimary && token.ownerUserId === currentUserId;
                const canEditPhoto = isSceneOwner || isMyPrimary;

                return (
                  <Popover.Root
                    key={token.tokenId}
                    open={selectedTokenId === token.tokenId}
                    onOpenChange={(open) => setSelectedTokenId(open ? token.tokenId : null)}
                  >
                    <Popover.Trigger asChild>
                      <div
                        className="token-list-item"
                        role="button"
                        data-testid={`token-list-item-${token.tokenId}`}
                      >
                        <img src={getTokenAvatar(token)} alt="Token" className="token-avatar" />
                        <div className="token-info">
                          <div className="token-label">
                            {token.isPrimary ? 'Primary — ' : ''}
                            Token {token.tokenId.slice(0, 8)}
                          </div>
                          {token.health != null && (
                            <div className="token-health">
                              <div className="health-bar-container">
                                <div
                                  className="health-bar"
                                  style={{ width: `${getHealthPercentage(token.health, token.maxHealth)}%` }}
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
                            <label htmlFor={`photo-${token.tokenId}`}>Photo URL</label>
                            <input
                              id={`photo-${token.tokenId}`}
                              data-testid={`token-photo-input-${token.tokenId}`}
                              type="text"
                              defaultValue={token.photoUrl ?? ''}
                              placeholder="https://..."
                              onBlur={(e) => {
                                const value = e.target.value.trim();
                                if (value) void handleSetPrimaryPhoto(token.tokenId, value);
                              }}
                            />
                          </div>
                        )}

                        {isSceneOwner && (
                          <div className="form-group">
                            <label htmlFor={`owner-${token.tokenId}`}>Owner user ID</label>
                            <input
                              id={`owner-${token.tokenId}`}
                              data-testid={`token-owner-input-${token.tokenId}`}
                              type="text"
                              defaultValue={token.ownerUserId ?? ''}
                              placeholder="(unassigned)"
                              onBlur={(e) => {
                                const value = e.target.value.trim();
                                void handleSetOwnership(token.tokenId, value || null, token.isPrimary);
                              }}
                            />
                            <label>
                              <input
                                type="checkbox"
                                data-testid={`token-primary-checkbox-${token.tokenId}`}
                                checked={token.isPrimary}
                                disabled={!token.ownerUserId}
                                onChange={(e) =>
                                  void handleSetOwnership(token.tokenId, token.ownerUserId, e.target.checked)
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
                            {loading ? 'Deleting...' : 'Delete'}
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
            <Dialog.Root open={createDialogOpen} onOpenChange={setCreateDialogOpen}>
              <Dialog.Trigger asChild>
                <button
                  className="token-create-button"
                  data-testid="token-create-trigger"
                  disabled={loading}
                >
                  {loading ? 'Creating...' : '+ Create Token'}
                </button>
              </Dialog.Trigger>

              <Dialog.Portal>
                <Dialog.Overlay className="token-panel-overlay" />
                <Dialog.Content className="token-create-dialog">
                  <Dialog.Title>Create New Token</Dialog.Title>
                  <Dialog.Description>Fill in token details. Leave blank for defaults.</Dialog.Description>

                  <div className="token-form">
                    <div className="form-group">
                      <label htmlFor="token-health">Current Health</label>
                      <input
                        id="token-health"
                        type="number"
                        value={newTokenHealth ?? ''}
                        onChange={(e) =>
                          setNewTokenHealth(e.target.value ? parseInt(e.target.value, 10) : undefined)
                        }
                        placeholder="e.g., 50"
                      />
                    </div>

                    <div className="form-group">
                      <label htmlFor="token-max-health">Max Health</label>
                      <input
                        id="token-max-health"
                        type="number"
                        value={newTokenMaxHealth ?? ''}
                        onChange={(e) =>
                          setNewTokenMaxHealth(e.target.value ? parseInt(e.target.value, 10) : undefined)
                        }
                        placeholder="e.g., 100"
                      />
                    </div>
                  </div>

                  <div className="token-dialog-actions">
                    <button className="button-cancel" onClick={() => setCreateDialogOpen(false)} disabled={loading}>
                      Cancel
                    </button>
                    <button
                      className="button-create"
                      data-testid="token-create-submit"
                      onClick={handleCreateToken}
                      disabled={loading}
                    >
                      {loading ? 'Creating...' : 'Create Token'}
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
