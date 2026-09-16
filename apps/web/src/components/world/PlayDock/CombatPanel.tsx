import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  addCombatant,
  addLairCombatant,
  advanceTurn,
  changeHitPoints,
  endCombat,
  getActiveCombat,
  removeCombatant,
  startCombat,
  updateCombatant,
} from "@/api/combat";
import { getWorldActors } from "@/api/actors";
import { setCombatAutoApply } from "@/api/attacks";
import { getTokens } from "@/api/tokens";
import { Button } from "@/components/ui/button/Button";
import {
  subscribeToWorldEvents,
  startPlayPanelEventSync,
} from "@/engine/world/sync";
import { cn } from "@/lib/utils";
import type { CombatRecord, HitPointChange } from "@/types/combat";
import type { WorldActorRecord } from "@/types/actor";
import type { TokenRecord } from "@/types/token";
import {
  buildRosterOffer,
  unattemptedIds,
  type RosterCandidate,
} from "./combatRoster";
import { CombatantHitPoints, CombatantOutMark } from "./CombatantHitPoints";
import { CombatantBudget } from "./CombatantBudget";
import { CombatantAct } from "./CombatantAct";
import { combatantActKind } from "./combatantActions";
import { useSelectedTokenIds } from "./useSelectedTokenIds";
import { LookAtButton } from "./LookAtButton";
import { readFollowTheTurn, writeFollowTheTurn } from "./followTheTurn";
import { FOLLOW_SURROUND_CELLS, lookAtToken, mayLookAt } from "@/engine/lookAt";

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
  const [lairLabel, setLairLabel] = useState("");
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

  /**
   * The scene's tokens, for the selection offer and for the target icons.
   *
   * No longer a Game Master's read alone (owner decision 2026-09-15): a
   * player's tracker offers to look at a creature too, and the token's record
   * is what says where it is and whether this viewer may read its name. The
   * server already sends a player these tokens to draw the board with, so this
   * asks for nothing new — and everything the offer drives is still rendered
   * under `isGm`.
   */
  const loadSceneTokens = useCallback(() => {
    if (!sceneId) return;
    getTokens(sceneId)
      // A failure here costs the offer and the target icons, not the tracker:
      // the roster, the round and every existing control keep working without
      // it.
      .then(setSceneTokens)
      .catch(() => undefined);
  }, [sceneId]);

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

  /**
   * Spec: the camera follows whoever's turn it is (owner decision 2026-09-15).
   *
   * Per person and remembered in this browser — see `followTheTurn.ts` for why
   * it is nobody else's setting and why it starts off.
   */
  // Held with the world it was read for, so a panel reused for another world
  // reads that world's preference during render rather than one frame late
  // from an effect.
  const [followPref, setFollowPref] = useState(() => ({
    worldId,
    enabled: readFollowTheTurn(worldId),
  }));
  const following =
    followPref.worldId === worldId
      ? followPref.enabled
      : readFollowTheTurn(worldId);
  const setFollowing = (enabled: boolean) =>
    setFollowPref({ worldId, enabled });

  /**
   * The turn the camera was last moved for.
   *
   * The rule, stated here: **the camera moves on a turn change, and never
   * between turns.** Moving the camera yourself mid-turn is therefore not a
   * fight — nothing pulls it back, and following simply resumes at the next
   * turn. That is the behaviour the toggle promises: a Game Master who pans
   * away to look at the far end of the corridor keeps their view until the
   * turn passes, and then gets taken to whoever is up.
   *
   * A ref rather than state: it must not cause a render, and it must be
   * written before the next render so one turn change moves the camera once
   * however many times this component re-renders for other reasons.
   */
  const followedTurn = useRef<string | null>(null);

  const activeCombatantId = combat?.activeCombatantId ?? null;
  const activeTokenId =
    combat?.combatants.find((combatant) => combatant.id === activeCombatantId)
      ?.tokenId ?? null;

  useEffect(() => {
    if (!following) {
      // Remembered even while off, so turning it back on mid-fight does not
      // immediately yank the camera to the turn already in progress. The next
      // turn change is what moves it, which is what "follow the turn" means.
      followedTurn.current = activeCombatantId;
      return;
    }
    if (!activeCombatantId || activeCombatantId === followedTurn.current) {
      return;
    }
    followedTurn.current = activeCombatantId;
    // A lair has no token to fly to (spec 046 US6), and neither has a
    // combatant added from an actor that was never placed. Nothing to do.
    if (!activeTokenId) return;
    // Zoomed out a little, so what is around the creature is visible too —
    // the engine decides how far, from the creature's own footprint.
    lookAtToken(activeTokenId, { surroundCells: FOLLOW_SURROUND_CELLS });
  }, [following, activeCombatantId, activeTokenId]);

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
   * Spec 046: the Game Master's Damage and Heal.
   *
   * The answer is hit points, not a combat, so it is not adopted here: the
   * tracker is re-read, which picks up a creature marked out at zero, and the
   * bars move on every board from world event 26.
   */
  const changeHp = async (
    tokenId: string,
    kind: HitPointChange,
    amount: number,
  ) => {
    setBusy(true);
    setError(null);
    try {
      await changeHitPoints(tokenId, kind, amount);
      await refresh();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Changing hit points failed",
      );
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
        {combat.roundLabel ? (
          <span
            className="text-xs font-semibold tracking-widest text-muted-foreground uppercase"
            data-testid="combat-round-counter"
          >
            {combat.roundLabel} {combat.round}
          </span>
        ) : (
          // Deliberately an empty span rather than nothing: the row is a
          // space-between, and dropping the element entirely would slide the
          // GM's controls to the left only for systems without rounds.
          <span />
        )}
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

      {/*
        Everyone's, not the Game Master's: a player waiting for their turn is
        exactly who this was asked for ("I could scroll through the combat and
        see if it's my turn — the viewport should scroll to me"). It changes
        nothing anyone else sees.
      */}
      <label className="flex items-center gap-2 text-xs text-muted-foreground">
        <input
          type="checkbox"
          checked={following}
          data-testid="follow-the-turn"
          className="size-3.5 accent-primary"
          onChange={(event) => {
            const enabled = event.target.checked;
            setFollowing(enabled);
            writeFollowTheTurn(worldId, enabled);
          }}
        />
        Follow the turn
      </label>

      {isGm ? (
        // Spec 046 FR-006: the per-encounter override of the world's
        // auto-apply default. It lives on this encounter and ends with it.
        <label className="grid gap-1 text-xs">
          <span className="text-muted-foreground">
            Damage to your NPCs in this encounter
          </span>
          <select
            value={
              combat.autoApply === null
                ? "world"
                : combat.autoApply
                  ? "on"
                  : "off"
            }
            disabled={busy}
            data-testid="combat-auto-apply"
            className="rounded border border-border bg-background px-2 py-1"
            onChange={(event) => {
              const choice = event.target.value;
              const enabled =
                choice === "world" ? null : choice === "on" ? true : false;
              void (async () => {
                setBusy(true);
                setError(null);
                try {
                  await setCombatAutoApply(combat.id, enabled);
                  await refresh();
                } catch (err) {
                  setError(
                    err instanceof Error
                      ? err.message
                      : "Changing auto-apply failed",
                  );
                } finally {
                  setBusy(false);
                }
              })();
            }}
          >
            <option value="world">
              As the world says (
              {combat.autoApply === null && combat.effectiveAutoApply
                ? "applied"
                : combat.autoApply === null
                  ? "offered"
                  : "world default"}
              )
            </option>
            <option value="on">Applied automatically</option>
            <option value="off">Offered to me first</option>
          </select>
        </label>
      ) : null}

      {combat.combatants.length === 0 ? (
        <p className="text-sm text-muted-foreground">No combatants yet.</p>
      ) : (
        <ul className="grid gap-1" data-testid="combatant-list">
          {combat.combatants.map((combatant) => {
            const isTurn = combatant.id === combat.activeCombatantId;
            const tokenId = combatant.tokenId;
            const actKind = combatantActKind(combatant);
            const token = sceneTokens.find(
              (row) => row.sceneId === sceneId && row.tokenId === tokenId,
            );
            const locatable = mayLookAt(token, isGm) ? token : undefined;
            return (
              <li
                key={combatant.id}
                data-testid="combatant-row"
                data-active-turn={isTurn ? "true" : "false"}
                data-downed-by={combatant.downedBy ?? ""}
                className={cn(
                  "flex flex-wrap items-center gap-2 rounded-lg border px-2 py-1.5",
                  isTurn ? "border-primary bg-primary/10" : "border-border",
                  !combatant.active && "opacity-50",
                )}
              >
                {/*
                  "When I'm in combat with 50 goblins, any one of them should
                  have a little icon I can scroll to." A lair has no token, and
                  a creature this viewer may not name gets none either — the
                  engine would refuse it, and an icon that quietly does nothing
                  is worse than no icon.

                  First in the row, not beside the name: a Game Master's
                  keyboard path through a row runs from initiative straight to
                  the hit-point amount, and a target between them would cost an
                  extra Tab on every creature they damage. A row with no target
                  keeps the space, so fifty rows still line up.
                */}
                {locatable ? (
                  <LookAtButton
                    tokenId={locatable.tokenId}
                    label={combatant.label}
                    testIdPrefix="combatant-look-at"
                  />
                ) : (
                  <span aria-hidden="true" className="w-[22px] shrink-0" />
                )}

                {isGm ? (
                  <input
                    type="number"
                    value={combatant.initiative}
                    aria-label={`Initiative for ${combatant.label}`}
                    className="h-7 w-12 rounded border border-input bg-transparent px-1 text-sm tabular-nums outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
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
                  {combatant.kind === "LAIR" ? (
                    <span
                      className="ml-1 text-xs text-muted-foreground"
                      data-testid="combatant-lair"
                    >
                      Lair
                    </span>
                  ) : combatant.isNpc ? (
                    <span className="ml-1 text-xs text-muted-foreground">
                      NPC
                    </span>
                  ) : null}
                </span>
                <CombatantOutMark combatant={combatant} />

                {isGm && tokenId ? (
                  <CombatantHitPoints
                    combatant={combatant}
                    busy={busy}
                    onChange={(kind, amount) =>
                      void changeHp(tokenId, kind, amount)
                    }
                  />
                ) : null}

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

                {/* Spec 046 US5: every seat sees what each creature has left.
                    A lair has no budget (null), so shows none. */}
                <CombatantBudget
                  label={combatant.label}
                  budget={combatant.budget}
                />
                {isGm && actKind ? (
                  // Spec 046 US6: a legendary action between turns, or the
                  // lair's, made as an attack by the Game Master.
                  <CombatantAct
                    worldId={worldId}
                    combatant={combatant}
                    tokens={sceneTokens.filter(
                      (token) => token.sceneId === sceneId,
                    )}
                    kind={actKind}
                  />
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
              className="h-9 min-w-0 flex-1 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
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
          {/* Spec 046 US6 (FR-053): a lair takes initiative count 20, and
              loses ties; the server places it. */}
          <label
            htmlFor="combat-add-lair"
            className="text-xs font-semibold tracking-widest text-muted-foreground uppercase"
          >
            Add lair
          </label>
          <div className="flex gap-2">
            <input
              id="combat-add-lair"
              type="text"
              value={lairLabel}
              placeholder="The lair's name"
              onChange={(event) => setLairLabel(event.target.value)}
              data-testid="combat-add-lair-name"
              className="h-9 min-w-0 flex-1 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
            />
            <Button
              type="button"
              size="sm"
              disabled={busy || lairLabel.trim() === ""}
              data-testid="combat-add-lair-button"
              onClick={() => {
                const label = lairLabel.trim();
                void run(() => addLairCombatant(combat.id, label)).then(() =>
                  setLairLabel(""),
                );
              }}
            >
              Add lair
            </Button>
          </div>
        </div>
      ) : null}
    </div>
  );
}
