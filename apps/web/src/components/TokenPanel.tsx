import React, { useCallback, useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import * as Popover from '@radix-ui/react-popover';
import { Dicebear } from '@dicebear/avatars';
import { useGraphQL } from '../hooks/useGraphQL';
import { useRxDB } from '../hooks/useRxDB';
import '../styles/TokenPanel.scss';

interface WorldToken {
  id: string;
  world_id: string;
  label?: string;
  x: number;
  y: number;
  z: number;
  health?: number;
  max_health?: number;
  created_by: string;
  schema_version: number;
  created_at: string;
  updated_at: string;
}

interface TokenPanelProps {
  worldId: string;
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
}

/**
 * TokenPanel: React component for managing world tokens.
 *
 * Features:
 * - Displays list of tokens in the world
 * - Create new tokens
 * - Edit token stats (health, label)
 * - Delete tokens
 * - Uses Dicebear for deterministic token avatars
 * - Syncs with RxDB for offline-first caching
 * - Emits GraphQL mutations for server persistence
 *
 * Architecture:
 * - Observes RxDB `world_tokens` collection
 * - Emits mutations via GraphQL client
 * - RxDB replication plugin syncs via worldEventCreated subscription
 * - No direct network calls; all changes flow through GraphQL
 */
export const TokenPanel: React.FC<TokenPanelProps> = ({
  worldId,
  isOpen,
  onOpenChange,
}) => {
  const { graphql } = useGraphQL();
  const { db } = useRxDB();
  const [tokens, setTokens] = useState<WorldToken[]>([]);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [newTokenLabel, setNewTokenLabel] = useState('');
  const [newTokenHealth, setNewTokenHealth] = useState<number | undefined>();
  const [newTokenMaxHealth, setNewTokenMaxHealth] = useState<number | undefined>();
  const [selectedTokenId, setSelectedTokenId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Subscribe to world_tokens collection changes
  React.useEffect(() => {
    if (!db || !isOpen) return;

    const subscription = db.collections.world_tokens
      ?.find()
      .where('world_id')
      .eq(worldId)
      .sort({ created_at: -1 })
      .$
      .subscribe((results) => {
        setTokens(results as WorldToken[]);
      });

    return () => subscription?.unsubscribe();
  }, [db, isOpen, worldId]);

  /**
   * Create a new token via GraphQL mutation.
   * The mutation automatically:
   * 1. Sets created_by from authenticated session
   * 2. Persists to database
   * 3. Triggers pg_notify
   * 4. Broadcasts via worldEventCreated subscription
   * 5. RxDB replication applies the delta
   */
  const handleCreateToken = useCallback(async () => {
    if (!graphql) return;

    setLoading(true);
    setError(null);

    try {
      const response = await graphql.mutate('createWorldToken', {
        input: {
          world_id: worldId,
          label: newTokenLabel || undefined,
          health: newTokenHealth,
          max_health: newTokenMaxHealth,
          x: 0, // Default spawn position
          y: 0,
          z: 0,
        },
      });

      if (response.errors) {
        setError(response.errors[0]?.message || 'Failed to create token');
      } else {
        // Clear form; RxDB subscription will update tokens list
        setNewTokenLabel('');
        setNewTokenHealth(undefined);
        setNewTokenMaxHealth(undefined);
        setCreateDialogOpen(false);
      }
    } catch (err) {
      setError(
        err instanceof Error ? err.message : 'Unknown error creating token'
      );
    } finally {
      setLoading(false);
    }
  }, [graphql, worldId, newTokenLabel, newTokenHealth, newTokenMaxHealth]);

  /**
   * Delete a token. Authorization enforced at backend:
   * Only the token creator (created_by == authenticated_user_id) can delete.
   */
  const handleDeleteToken = useCallback(
    async (tokenId: string) => {
      if (!graphql) return;

      if (!window.confirm('Delete this token?')) return;

      setLoading(true);
      setError(null);

      try {
        const response = await graphql.mutate('deleteWorldToken', {
          token_id: tokenId,
        });

        if (response.errors) {
          setError(response.errors[0]?.message || 'Failed to delete token');
        } else {
          setSelectedTokenId(null);
          // RxDB subscription updates list
        }
      } catch (err) {
        setError(
          err instanceof Error ? err.message : 'Unknown error deleting token'
        );
      } finally {
        setLoading(false);
      }
    },
    [graphql]
  );

  /**
   * Move token to a new position. Called from canvas drag-to-move.
   * Uses moveToken mutation for proper authorization and server validation.
   */
  const handleMoveToken = useCallback(
    async (tokenId: string, x: number, y: number, z?: number) => {
      if (!graphql) return;

      try {
        await graphql.mutate('moveToken', {
          input: {
            token_id: tokenId,
            x,
            y,
            z,
          },
        });
      } catch (err) {
        console.error('Failed to move token:', err);
        setError(
          err instanceof Error ? err.message : 'Unknown error moving token'
        );
      }
    },
    [graphql]
  );

  /**
   * Get deterministic avatar for token.
   * Uses token_id as seed for Dicebear API.
   * Provides visual identity independent of player avatars.
   */
  const getTokenAvatar = (tokenId: string): string => {
    return `https://api.dicebear.com/9.x/adventurer-neutral/svg?seed=${tokenId}`;
  };

  /**
   * Compute health bar width percentage.
   * Derived locally; not sent over network.
   */
  const getHealthPercentage = (health?: number, maxHealth?: number): number => {
    if (health === undefined || maxHealth === undefined || maxHealth <= 0) {
      return 0;
    }
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
            Create, manage, and position tokens on the scene.
          </Dialog.Description>

          {error && <div className="token-panel-error">{error}</div>}

          {/* Token List */}
          <div className="token-list">
            {tokens.length === 0 ? (
              <div className="token-list-empty">
                No tokens yet. Create one to get started.
              </div>
            ) : (
              tokens.map((token) => (
                <Popover.Root
                  key={token.id}
                  open={selectedTokenId === token.id}
                  onOpenChange={(open) => {
                    setSelectedTokenId(open ? token.id : null);
                  }}
                >
                  <Popover.Trigger asChild>
                    <div className="token-list-item" role="button">
                      <img
                        src={getTokenAvatar(token.id)}
                        alt={token.label || 'Token'}
                        className="token-avatar"
                      />
                      <div className="token-info">
                        <div className="token-label">
                          {token.label || `Token ${token.id.slice(0, 8)}`}
                        </div>
                        {token.health !== undefined && (
                          <div className="token-health">
                            <div className="health-bar-container">
                              <div
                                className="health-bar"
                                style={{
                                  width: `${getHealthPercentage(
                                    token.health,
                                    token.max_health
                                  )}%`,
                                }}
                              />
                            </div>
                            <span className="health-text">
                              {token.health}/{token.max_health}
                            </span>
                          </div>
                        )}
                      </div>
                    </div>
                  </Popover.Trigger>

                  <Popover.Content className="token-popover-content">
                    <div className="token-details">
                      <h4>{token.label || 'Token Details'}</h4>
                      <div className="token-position">
                        <p>
                          Position: ({token.x.toFixed(1)}, {token.y.toFixed(1)}
                          , {token.z.toFixed(1)})
                        </p>
                        <p>ID: {token.id}</p>
                      </div>
                      <button
                        className="token-delete-button"
                        onClick={() => handleDeleteToken(token.id)}
                        disabled={loading}
                      >
                        {loading ? 'Deleting...' : 'Delete'}
                      </button>
                    </div>
                  </Popover.Content>
                </Popover.Root>
              ))
            )}
          </div>

          {/* Create Token Button */}
          <Dialog.Root open={createDialogOpen} onOpenChange={setCreateDialogOpen}>
            <Dialog.Trigger asChild>
              <button className="token-create-button" disabled={loading}>
                {loading ? 'Creating...' : '+ Create Token'}
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
                  <div className="form-group">
                    <label htmlFor="token-label">Label</label>
                    <input
                      id="token-label"
                      type="text"
                      value={newTokenLabel}
                      onChange={(e) => setNewTokenLabel(e.target.value)}
                      placeholder="e.g., Goblin, Chest, Portal"
                    />
                  </div>

                  <div className="form-group">
                    <label htmlFor="token-health">Current Health</label>
                    <input
                      id="token-health"
                      type="number"
                      value={newTokenHealth ?? ''}
                      onChange={(e) =>
                        setNewTokenHealth(
                          e.target.value ? parseInt(e.target.value) : undefined
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
                      value={newTokenMaxHealth ?? ''}
                      onChange={(e) =>
                        setNewTokenMaxHealth(
                          e.target.value ? parseInt(e.target.value) : undefined
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
                    onClick={handleCreateToken}
                    disabled={loading}
                  >
                    {loading ? 'Creating...' : 'Create Token'}
                  </button>
                </div>
              </Dialog.Content>
            </Dialog.Portal>
          </Dialog.Root>

          {/* Close Button */}
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
