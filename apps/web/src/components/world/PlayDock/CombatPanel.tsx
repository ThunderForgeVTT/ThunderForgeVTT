import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  addCombatant,
  advanceTurn,
  endCombat,
  getActiveCombat,
  removeCombatant,
  startCombat,
  updateCombatant,
} from "@/api/combat";
import { getWorldActors } from "@/api/actors";
import { getTokens } from "@/api/tokens";
import { Button } from "@/components/ui/button/Button";
import {
  subscribeToWorldEvents,
  startPlayPanelEventSync,
} from "@/engine/world/sync";
import { cn } from "@/lib/utils";
import type { CombatRecord } from "@/types/combat";
import type { WorldActorRecord } from "@/types/actor";
import type { TokenRecord } from "@/types/token";
import {
  buildRosterOffer,
  unattemptedIds,
  type RosterCandidate,
} from "./combatRoster";
import { useSelectedTokenIds } from "./useSelectedTokenIds";

export interface CombatPanelProps {
  worldId: string;
  sceneId: string | null;
  isGm: boolean;
}

/**
 * The shared initiative tracker.
 *
 * Shared is the whole point, so this component holds no turn-order state of
 * its own: every mutation returns the authoritative combat and the
 * `world_events` subscription (code 18) refetches it whenever anyone else
 * changes it. The list is rendered in exactly the order the server sends —
 * sorting here would be a second ordering rule that could disagree with the
 * server's, which is the bug a shared tracker exists to prevent.
 *
 * Players get a read-only view; every control is GM-gated both here and,
 * authoritatively, in `mutations_combat.rs`.
 *
 * # Offering the selection (spec 031 FR-030)
 *
 * A GM who has just selected the tokens they mean to fight with is offered
 * them, one press, instead of picking each out of the actor list. The offer is
 * additive and explicit: see `combatRoster.ts` for why replacing the roster
 * was rejected. Nothing about the round and turn presentation below changed
 * with it — the same `combat.round`, the same server-given order, the same
 * "Next turn" (FR-031's turn structure is the game system's to define, which
 * is spec 032's work, not this one's).
 */
export function CombatPanel({ worldId, sceneId, isGm }: CombatPanelProps) {
  const [combat, setCombat] = useState<CombatRecord | null>(null);
  const [loading, setLoading] = useState(true);
  const [actors, setActors] = useState<WorldActorRecord[]>([]);
  const [addActorId, setAddActorId] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const selectedTokenIds = useSelectedTokenIds();
  const [sceneTokens, setSceneTokens] = useState<TokenRecord[]>([]);
  // Ids already asked about, so a selected token that will never be persisted
  // (the engine's demo tokens) cannot drive a refetch on every render.
  const lookedUp = useRef<Set<string>>(new Set());

  const refresh = useCallback(() => {
    return getActiveCombat(worldId)
      .then(setCombat)
      .catch((err) =>
        setError(err instanceof Error ? err.message : "Failed to load combat"),
      )
      .finally(() => setLoading(false));
  }, [worldId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const stop = startPlayPanelEventSync(
      { onCombatChanged: () => void refresh() },
      subscribeToWorldEvents(worldId),
    );
    return stop;
  }, [worldId, refresh]);

  useEffect(() => {
    if (!isGm) return;
    getWorldActors(worldId)
      .then(setActors)
      .catch(() => setActors([]));
  }, [worldId, isGm]);

  const loadSceneTokens = useCallback(() => {
    if (!isGm || !sceneId) return;
    getTokens(sceneId)
      // A failure here costs the offer, not the tracker: the roster, the round
      // and every existing control keep working without it.
      .then(setSceneTokens)
      .catch(() => undefined);
  }, [isGm, sceneId]);

  useEffect(() => {
    lookedUp.current = new Set();
    loadSceneTokens();
  }, [loadSceneTokens]);

  const offer = useMemo(
    () =>
      buildRosterOffer({
        selectedTokenIds,
        // Filtered rather than cleared on a scene change: the previous
        // scene's tokens are still in hand until the new fetch answers, and
        // labelling this scene's selection from them would name the wrong
        // character for as long as the request takes.
        tokens: sceneTokens.filter((token) => token.sceneId === sceneId),
        actors,
        combatants: combat?.combatants ?? [],
      }),
    [selectedTokenIds, sceneTokens, sceneId, actors, combat],
  );

  useEffect(() => {
    // A token placed moments ago is selected before this panel has heard of
    // it. One look per id is enough to catch that without polling.
    const pending = unattemptedIds(offer.unresolvedTokenIds, lookedUp.current);
    if (pending.length === 0) return;
    for (const id of pending) lookedUp.current.add(id);
    loadSceneTokens();
  }, [offer.unresolvedTokenIds, loadSceneTokens]);

  /** Runs a mutation, adopts its authoritative result, and surfaces failures. */
  const run = async (action: () => Promise<CombatRecord>) => {
    setBusy(true);
    setError(null);
    try {
      setCombat(await action());
    } catch (err) {
      setError(err instanceof Error ? err.message : "Combat action failed");
    } finally {
      setBusy(false);
    }
  };

  /**
   * Files the offered tokens into `combat`, one at a time.
   *
   * Sequential on purpose: the server decides each combatant's place in the
   * order, and firing the adds together would let the party land in whatever
   * order the requests happened to finish in rather than the order the GM
   * selected them. Each call answers with the whole combat, so the last answer
   * is the authoritative one; a failure part-way leaves the combatants already
   * accepted in place, which the `world_events` refetch then reconciles.
   */
  const withCandidates = async (
    target: CombatRecord,
    candidates: RosterCandidate[],
  ): Promise<CombatRecord> => {
    let latest = target;
    for (const candidate of candidates) {
      latest = await addCombatant({
        combatId: target.id,
        label: candidate.label,
        actorId: candidate.actorId,
        tokenId: candidate.tokenId,
        isNpc: candidate.isNpc,
      });
    }
    return latest;
  };

  if (loading) {
    return <p className="text-sm text-muted-foreground">Loading combat…</p>;
  }

  if (!combat || combat.endedAt) {
    return (
      <div className="grid gap-3" data-testid="combat-panel">
        <p className="text-sm text-muted-foreground">No combat in progress.</p>
        {error ? <p className="text-sm text-destructive">{error}</p> : null}
        {isGm ? (
          <Button
            type="button"
            size="sm"
            disabled={busy}
            data-testid="start-combat-button"
            onClick={() => void run(() => startCombat(worldId, sceneId))}
          >
            Start combat
          </Button>
        ) : null}
        {isGm && offer.additions.length > 0 ? (
          // Offered beside the plain start, never instead of it: a GM with
          // something selected for an unrelated reason must still be able to
          // open an empty encounter.
          <Button
            type="button"
            size="sm"
            variant="secondary"
            disabled={busy}
            data-testid="start-combat-with-selection-button"
            onClick={() =>
              void run(async () =>
                withCandidates(
                  await startCombat(worldId, sceneId),
                  offer.additions,
                ),
              )
            }
          >
            Start with {offer.additions.length} selected
          </Button>
        ) : null}
      </div>
    );
  }

  const addableActors = actors.filter(
    (actor) =>
      !combat.combatants.some((combatant) => combatant.actorId === actor.id),
  );

  return (
    <div className="grid gap-3" data-testid="combat-panel">
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
          Round {combat.round}
        </span>
        {isGm ? (
          <div className="flex gap-2">
            <Button
              type="button"
              size="sm"
              disabled={busy || combat.combatants.length === 0}
              data-testid="advance-turn-button"
              onClick={() => void run(() => advanceTurn(combat.id))}
            >
              Next turn
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              disabled={busy}
              data-testid="end-combat-button"
              onClick={() => void run(() => endCombat(combat.id))}
            >
              End
            </Button>
          </div>
        ) : null}
      </div>

      {error ? <p className="text-sm text-destructive">{error}</p> : null}

      {combat.combatants.length === 0 ? (
        <p className="text-sm text-muted-foreground">No combatants yet.</p>
      ) : (
        <ul className="grid gap-1" data-testid="combatant-list">
          {combat.combatants.map((combatant) => {
            const isTurn = combatant.id === combat.activeCombatantId;
            return (
              <li
                key={combatant.id}
                data-testid="combatant-row"
                data-active-turn={isTurn ? "true" : "false"}
                className={cn(
                  "flex items-center gap-2 rounded-lg border px-2 py-1.5",
                  isTurn ? "border-primary bg-primary/10" : "border-border",
                  !combatant.active && "opacity-50",
                )}
              >
                {isGm ? (
                  <input
                    type="number"
                    value={combatant.initiative}
                    aria-label={`Initiative for ${combatant.label}`}
                    className="h-7 w-12 rounded border border-input bg-transparent px-1 text-sm tabular-nums outline-none"
                    onChange={(event) => {
                      const initiative = Number.parseInt(
                        event.target.value,
                        10,
                      );
                      if (!Number.isFinite(initiative)) return;
                      void run(() =>
                        updateCombatant({
                          combatantId: combatant.id,
                          initiative,
                        }),
                      );
                    }}
                  />
                ) : (
                  <span className="w-8 text-sm tabular-nums">
                    {combatant.initiative}
                  </span>
                )}

                <span className="min-w-0 flex-1 truncate text-sm">
                  {combatant.label}
                  {combatant.isNpc ? (
                    <span className="ml-1 text-xs text-muted-foreground">
                      NPC
                    </span>
                  ) : null}
                </span>

                {isGm ? (
                  <>
                    <button
                      type="button"
                      title={combatant.active ? "Mark down" : "Revive"}
                      aria-label={
                        combatant.active
                          ? `Mark ${combatant.label} down`
                          : `Revive ${combatant.label}`
                      }
                      disabled={busy}
                      className="rounded px-1.5 py-0.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground"
                      onClick={() =>
                        void run(() =>
                          updateCombatant({
                            combatantId: combatant.id,
                            active: !combatant.active,
                          }),
                        )
                      }
                    >
                      {combatant.active ? "Down" : "Up"}
                    </button>
                    <button
                      type="button"
                      aria-label={`Remove ${combatant.label}`}
                      disabled={busy}
                      className="rounded px-1.5 py-0.5 text-xs text-muted-foreground hover:bg-muted hover:text-destructive"
                      onClick={() =>
                        void run(() => removeCombatant(combatant.id))
                      }
                    >
                      ✕
                    </button>
                  </>
                ) : null}
              </li>
            );
          })}
        </ul>
      )}

      {isGm && offer.additions.length + offer.alreadyPresent.length > 0 ? (
        <div
          className="grid gap-2 border-t border-border pt-3"
          data-testid="combat-selection-offer"
        >
          <span className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Selected on the map
          </span>
          <ul className="grid gap-1" data-testid="combat-selection-list">
            {[
              ...offer.additions.map((candidate) => ({
                candidate,
                present: false,
              })),
              // Shown rather than hidden: selecting the party twice should say
              // why the count is smaller than the selection, not silently
              // drop names the GM can see highlighted on the map.
              ...offer.alreadyPresent.map((candidate) => ({
                candidate,
                present: true,
              })),
            ].map(({ candidate, present }) => {
              return (
                <li
                  key={candidate.tokenId}
                  data-testid="combat-selection-row"
                  data-already-in-combat={present ? "true" : "false"}
                  className={cn(
                    "truncate text-sm",
                    present && "text-muted-foreground",
                  )}
                >
                  {candidate.label}
                  {present ? " — already in combat" : null}
                </li>
              );
            })}
          </ul>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            disabled={busy || offer.additions.length === 0}
            data-testid="combat-add-selected-button"
            onClick={() =>
              void run(() => withCandidates(combat, offer.additions))
            }
          >
            Add {offer.additions.length} selected
          </Button>
        </div>
      ) : null}

      {isGm ? (
        <div className="grid gap-2 border-t border-border pt-3">
          <label
            htmlFor="combat-add-actor"
            className="text-xs font-semibold tracking-widest text-muted-foreground uppercase"
          >
            Add combatant
          </label>
          <div className="flex gap-2">
            <select
              id="combat-add-actor"
              value={addActorId}
              onChange={(event) => setAddActorId(event.target.value)}
              data-testid="combat-add-actor-select"
              className="h-9 min-w-0 flex-1 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
            >
              <option value="">Select an actor…</option>
              {addableActors.map((actor) => (
                <option key={actor.id} value={actor.id}>
                  {actor.label}
                </option>
              ))}
            </select>
            <Button
              type="button"
              size="sm"
              disabled={busy || !addActorId}
              data-testid="combat-add-button"
              onClick={() => {
                const actor = actors.find(
                  (candidate) => candidate.id === addActorId,
                );
                if (!actor) return;
                void run(() =>
                  addCombatant({
                    combatId: combat.id,
                    label: actor.label,
                    actorId: actor.id,
                    isNpc: actor.isNpc,
                  }),
                ).then(() => setAddActorId(""));
              }}
            >
              Add
            </Button>
          </div>
        </div>
      ) : null}
    </div>
  );
}
