import { useEffect, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { Link } from "react-router-dom";
import {
  createAbility,
  getWorldAbilities,
  suggestAbilityName,
} from "@/api/abilities";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import type {
  AbilityClassification,
  WorldAbilityRecord,
} from "@/types/ability";
import { labelFor, type AbilityVocabulary } from "@/abilities/vocabulary";

export interface AbilityCompendiumTabProps {
  worldId: string;
  onSelect: (abilityId: string) => void;
  selectedAbilityId: string | null;
  /** DM/GM-only — gates the create control (FR-002). */
  isGm: boolean;
  /** What this world calls its abilities, assembled by the server (spec 033 FR-006). */
  vocabulary: AbilityVocabulary;
  onCatalogLoaded?: (abilities: WorldAbilityRecord[]) => void;
}

/**
 * Spec 025 (T025): the Compendium's Abilities tab, replacing spec 011's
 * placeholder — the last one in the Compendium (SC-001).
 *
 * Mirrors `ItemCompendiumTab` (server-side `search`, debounced "did you mean?",
 * GM-gated create), with two additions:
 *
 *   * classifications render through the active system's facet labels
 *     (FR-012), so a 5E-style pack shows "Spell" where Genie might show
 *     "Scroll";
 *   * GM-only abilities carry a visible marker (FR-024d). A player never
 *     receives one — they are filtered server-side (FR-024b) — so this badge
 *     only ever renders for a DM, and is deliberately NOT a client-side
 *     visibility gate.
 *
 * Note the item version's dead `refreshKey` prop is not carried over: it was
 * declared but never passed by the parent (research.md §3, defect 6).
 */
export function AbilityCompendiumTab({
  worldId,
  onSelect,
  selectedAbilityId,
  isGm,
  vocabulary,
  onCatalogLoaded,
}: AbilityCompendiumTabProps) {
  const [abilities, setAbilities] = useState<WorldAbilityRecord[] | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [query, setQuery] = useState("");
  const [refreshTick, setRefreshTick] = useState(0);
  const [newName, setNewName] = useState("");
  const [newDescription, setNewDescription] = useState("");
  const [newClassification, setNewClassification] =
    useState<AbilityClassification>("SPELL");
  const [isCreating, setIsCreating] = useState(false);
  // Stored against the name it was looked up for. Whether a suggestion is
  // showing right now is then a render-time question ("is this still the
  // name we asked about, and is it still long enough to ask?"), so the
  // effect below never has to null it back out.
  const [loadedSuggestion, setLoadedSuggestion] = useState<{
    worldId: string;
    query: string;
    match: WorldAbilityRecord | null;
  } | null>(null);

  const handleAdd = async () => {
    const name = newName.trim();
    if (!name) {
      return;
    }
    setIsCreating(true);
    try {
      await createAbility({
        worldId,
        name,
        description: newDescription.trim() || undefined,
        classification: newClassification,
      });
      setNewName("");
      setNewDescription("");
      setLoadedSuggestion(null);
      setRefreshTick((current) => current + 1);
    } finally {
      setIsCreating(false);
    }
  };

  // Reset during render rather than at the top of the effect below: this
  // is state derived from the arguments, and doing it in the effect commits
  // one render pairing the new key with the previous key's data.
  useResetOnChange(`${worldId}|${query}|${refreshTick}`, () => {
    setAbilities(null);
    setError(null);
  });

  useEffect(() => {
    let active = true;

    getWorldAbilities(worldId, query || undefined)
      .then((result) => {
        if (!active) {
          return;
        }
        setAbilities(result);
        onCatalogLoaded?.(result);
      })
      .catch((err) => {
        if (active) {
          setError(err instanceof Error ? err : new Error(String(err)));
        }
      });

    return () => {
      active = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [worldId, query, refreshTick]);

  // FR-007: non-blocking "did you mean?" as the DM types — debounced, and it
  // never blocks handleAdd. Duplicate names are explicitly allowed (FR-006).
  const suggestionQuery = newName.trim();
  const suggestion =
    isGm &&
    suggestionQuery.length >= 2 &&
    loadedSuggestion?.worldId === worldId &&
    loadedSuggestion.query === suggestionQuery
      ? loadedSuggestion.match
      : null;

  useEffect(() => {
    const name = newName.trim();
    if (!isGm || name.length < 2) {
      return;
    }
    let active = true;
    const timer = setTimeout(() => {
      suggestAbilityName(worldId, name)
        .then((matches) => {
          if (active) {
            setLoadedSuggestion({
              worldId,
              query: name,
              match: matches[0] ?? null,
            });
          }
        })
        .catch(() => {
          if (active) {
            setLoadedSuggestion({ worldId, query: name, match: null });
          }
        });
    }, 300);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [worldId, newName, isGm]);

  if (error) {
    return (
      <p className="text-sm text-destructive">
        Failed to load abilities: {error.message}
      </p>
    );
  }

  if (abilities === null) {
    return <p className="text-sm text-muted-foreground">Loading abilities…</p>;
  }

  return (
    <div className="grid gap-3">
      <Input
        type="search"
        placeholder="Search abilities by name or description…"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        data-testid="ability-catalog-search-input"
        aria-label="Search abilities"
      />

      {abilities.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {query ? `No abilities match "${query}".` : "No Abilities yet."}
        </p>
      ) : (
        <div className="overflow-x-auto rounded-lg border border-border">
          <table className="w-full text-sm" data-testid="ability-catalog-table">
            <thead>
              <tr className="border-b border-border bg-muted/50 text-left text-xs tracking-wide text-muted-foreground uppercase">
                <th className="p-2 font-semibold">Name</th>
                <th className="p-2 font-semibold">Type</th>
                <th className="p-2 font-semibold">Description</th>
                <th className="p-2 font-semibold">Actions</th>
              </tr>
            </thead>
            <tbody>
              {abilities.map((ability) => (
                <tr
                  key={ability.id}
                  className={cn(
                    "cursor-pointer border-b border-border last:border-0 hover:bg-muted/40",
                    selectedAbilityId === ability.id && "bg-muted",
                  )}
                  data-testid={`ability-catalog-row-${ability.id}`}
                  onClick={() => onSelect(ability.id)}
                  aria-selected={selectedAbilityId === ability.id}
                >
                  <td className="p-2 font-medium">
                    {ability.name}
                    {ability.gmOnly ? (
                      <span
                        className="ml-2 rounded bg-muted px-1.5 py-0.5 text-xs font-normal text-muted-foreground"
                        data-testid={`ability-gm-only-badge-${ability.id}`}
                        title="Hidden from players"
                      >
                        GM-only
                      </span>
                    ) : null}
                  </td>
                  <td className="p-2 text-muted-foreground">
                    {labelFor(vocabulary, ability.classification)}
                  </td>
                  <td className="max-w-xs truncate p-2 text-muted-foreground">
                    {ability.description || (
                      <span className="italic">No description</span>
                    )}
                  </td>
                  <td className="p-2">
                    <div
                      className="flex gap-2"
                      onClick={(event) => event.stopPropagation()}
                    >
                      <Button
                        asChild
                        variant="ghost"
                        size="sm"
                        data-testid={`ability-catalog-view-${ability.id}`}
                      >
                        <Link
                          to={`/world/${worldId}/ability/${ability.id}/view`}
                        >
                          View
                        </Link>
                      </Button>
                      {ability.myPermissionLevel !== "VIEWER" ? (
                        <Button
                          asChild
                          variant="ghost"
                          size="sm"
                          data-testid={`ability-catalog-edit-${ability.id}`}
                        >
                          <Link
                            to={`/world/${worldId}/ability/${ability.id}/edit`}
                          >
                            Edit
                          </Link>
                        </Button>
                      ) : null}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {isGm ? (
        <div className="grid gap-2">
          <div className="grid gap-2 sm:grid-cols-[1fr_auto_1fr_auto]">
            <Input
              value={newName}
              onChange={(event) => setNewName(event.target.value)}
              placeholder="New ability name"
              disabled={isCreating}
              data-testid="new-ability-name-input"
            />
            <select
              className="rounded-md border border-border bg-background px-2 text-sm"
              value={newClassification}
              onChange={(event) =>
                setNewClassification(
                  event.target.value as AbilityClassification,
                )
              }
              disabled={isCreating}
              aria-label="Ability type"
              data-testid="new-ability-classification-select"
            >
              {/*
               * The world's own types, in the system's words and the
               * system's order (FR-004, FR-006), rather than a fixed list of
               * four in ours.
               *
               * Filtered to built-ins for now because the wire type is still
               * a GraphQL enum and only those four are storable. Increment D
               * retires the enum and drops the CHECK constraint, and this
               * filter goes with them — at which point a system's own type
               * becomes authorable here and nothing else about this control
               * changes.
               */}
              {vocabulary.types
                .filter((kind) => kind.builtin)
                .map((kind) => (
                  <option key={kind.id} value={kind.id.toUpperCase()}>
                    {kind.label}
                  </option>
                ))}
            </select>
            <Input
              value={newDescription}
              onChange={(event) => setNewDescription(event.target.value)}
              placeholder="Description (optional)"
              disabled={isCreating}
              data-testid="new-ability-description-input"
            />
            <Button
              type="button"
              size="sm"
              icon="spells"
              onClick={() => void handleAdd()}
              disabled={isCreating || !newName.trim()}
              data-testid="add-ability-button"
            >
              Add Ability
            </Button>
          </div>
          {suggestion ? (
            <p
              className="text-xs text-muted-foreground"
              data-testid="ability-name-suggestion"
            >
              Did you mean{" "}
              <span className="font-medium">{suggestion.name}</span>? Names can
              be reused if that&apos;s intentional.
            </p>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
