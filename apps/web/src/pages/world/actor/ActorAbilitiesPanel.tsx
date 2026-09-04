import { useCallback, useEffect, useState } from "react";
import { getWorldAbilities } from "@/api/abilities";
import {
  attachAbilityToActor,
  detachAbilityFromActor,
  getActorAbilities,
} from "@/api/actorAbilities";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type { WorldAbilityRecord } from "@/types/ability";
import type { ActorAbilityEntryRecord } from "@/types/actorAbility";
import {
  DEFAULT_VOCABULARY,
  getAbilityVocabulary,
  labelFor,
  type AbilityVocabulary,
} from "@/abilities/vocabulary";

export interface ActorAbilitiesPanelProps {
  actorId: string;
  worldId: string;
  /** The world's game system, for facet labels. */
  gameSystemId?: string | null;
  /**
   * FR-022: Editor/Owner on the **ACTOR** gates attach/detach — NOT the
   * caller's permission on any given ability. A GM can hand an NPC a spell the
   * players can't read; a player who owns an ability still can't put it on an
   * actor they don't control.
   */
  canManage: boolean;
}

/**
 * Spec 025 (T054): an actor's known abilities. Mirrors `ActorInventoryPanel`,
 * minus quantity — an actor either knows an ability or does not (Non-Goals
 * exclude slots, charges, and preparation).
 *
 * GM-only abilities never reach a non-DM here: they are filtered server-side
 * and silently, so this component does no visibility filtering of its own and
 * must not start doing any.
 */
export function ActorAbilitiesPanel({
  actorId,
  worldId,
  gameSystemId,
  canManage,
}: ActorAbilitiesPanelProps) {
  const [entries, setEntries] = useState<ActorAbilityEntryRecord[] | null>(
    null,
  );
  const [catalog, setCatalog] = useState<WorldAbilityRecord[] | null>(null);
  // Spec 033: the world's assembled vocabulary, so this panel names a type
  // the same way the compendium does (FR-006).
  const [vocabulary, setVocabulary] =
    useState<AbilityVocabulary>(DEFAULT_VOCABULARY);
  const [error, setError] = useState<string | null>(null);
  const [selectedAbilityId, setSelectedAbilityId] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const [pendingEntryId, setPendingEntryId] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setEntries(await getActorAbilities(actorId));
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load known abilities",
      );
    }
  }, [actorId]);

  useEffect(() => {
    let active = true;
    getActorAbilities(actorId)
      .then((result) => {
        if (active) {
          setEntries(result);
        }
      })
      .catch((err) => {
        if (active) {
          setError(
            err instanceof Error
              ? err.message
              : "Failed to load known abilities",
          );
        }
      });
    return () => {
      active = false;
    };
  }, [actorId]);

  // A viewer never fetches the catalog — mirrors ActorInventoryPanel.
  useEffect(() => {
    if (!canManage) {
      return;
    }
    let active = true;
    getWorldAbilities(worldId)
      .then((result) => {
        if (active) {
          setCatalog(result);
        }
      })
      .catch(() => {
        if (active) {
          setCatalog([]);
        }
      });
    return () => {
      active = false;
    };
  }, [worldId, canManage]);

  useEffect(() => {
    let active = true;
    getAbilityVocabulary(worldId)
      .then((assembled) => {
        if (active) setVocabulary(assembled);
      })
      .catch(() => {
        if (active) setVocabulary(DEFAULT_VOCABULARY);
      });
    return () => {
      active = false;
    };
  }, [worldId, gameSystemId]);

  const handleAttach = async () => {
    if (!selectedAbilityId) {
      return;
    }
    setIsSaving(true);
    setError(null);
    try {
      await attachAbilityToActor(actorId, selectedAbilityId);
      setSelectedAbilityId("");
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to attach ability");
    } finally {
      setIsSaving(false);
    }
  };

  const handleDetach = async (entryId: string) => {
    setPendingEntryId(entryId);
    setError(null);
    try {
      await detachAbilityFromActor(entryId);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to detach ability");
    } finally {
      setPendingEntryId(null);
    }
  };

  if (entries === null) {
    return (
      <Card className="p-5" data-testid="actor-abilities-panel">
        <p className="text-sm text-muted-foreground">
          Loading known abilities…
        </p>
      </Card>
    );
  }

  return (
    <Card className="grid gap-3 p-5" data-testid="actor-abilities-panel">
      <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
        Known abilities
      </p>

      {entries.length === 0 ? (
        <p className="text-sm text-muted-foreground italic">
          No known abilities.
        </p>
      ) : (
        <ul className="grid gap-1">
          {entries.map((entry) => (
            <li
              key={entry.id}
              className="flex items-center justify-between gap-3 text-sm"
              data-testid={`actor-ability-${entry.id}`}
            >
              <span>
                {/* A tombstoned ability reads REDACTED for non-DMs — the
                    server withholds the name, since a deleted row carries no
                    gm_only flag to check (fail closed). */}
                <span
                  className={
                    entry.abilityId === null
                      ? "font-medium text-muted-foreground"
                      : "font-medium"
                  }
                >
                  {entry.abilityName}
                </span>
                {entry.classification ? (
                  <span className="ml-2 text-muted-foreground">
                    {labelFor(vocabulary, entry.classification)}
                  </span>
                ) : (
                  // Tombstone: the ability was deleted, but the entry and its
                  // name snapshot survive (FR-023).
                  <span className="ml-2 text-muted-foreground italic">
                    (deleted ability)
                  </span>
                )}
                {entry.gmOnly ? (
                  <span
                    className="ml-2 rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground"
                    title="Hidden from players"
                  >
                    GM-only
                  </span>
                ) : null}
              </span>
              {canManage ? (
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={pendingEntryId === entry.id}
                  onClick={() => void handleDetach(entry.id)}
                  data-testid={`actor-ability-detach-${entry.id}`}
                >
                  Remove
                </Button>
              ) : null}
            </li>
          ))}
        </ul>
      )}

      {canManage ? (
        <div className="grid gap-2 sm:grid-cols-[1fr_auto]">
          <select
            className="rounded-md border border-border bg-background px-2 py-1 text-sm"
            value={selectedAbilityId}
            onChange={(event) => setSelectedAbilityId(event.target.value)}
            disabled={isSaving || catalog === null}
            aria-label="Ability to attach"
            data-testid="actor-ability-select"
          >
            <option value="">Select an ability…</option>
            {(catalog ?? []).map((ability) => (
              <option key={ability.id} value={ability.id}>
                {ability.name}
                {ability.gmOnly ? " (GM-only)" : ""}
              </option>
            ))}
          </select>
          <Button
            type="button"
            size="sm"
            onClick={() => void handleAttach()}
            disabled={isSaving || !selectedAbilityId}
            data-testid="actor-ability-attach-button"
          >
            Attach
          </Button>
        </div>
      ) : null}

      {error ? <StatusBadge variant="danger">{error}</StatusBadge> : null}
    </Card>
  );
}
