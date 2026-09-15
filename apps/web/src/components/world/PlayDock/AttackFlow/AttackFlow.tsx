import { useEffect, useMemo, useRef, useState } from "react";
import { makeAttack, previewAttack } from "@/api/attacks";
import { Button } from "@/components/ui/button/Button";
import { triggerDiceRollAnimation } from "@/engine/bevy";
import { queueEdit, shouldQueue } from "@/engine/world/sync/offlineQueue";
import type {
  ActionCost,
  AttackInput,
  AttackPreviewRecord,
  AttackRecord,
} from "@/types/attack";
import type { TokenRecord } from "@/types/token";
import { attackSummary, warningTexts } from "../AttackLog/attackText";
import { useSelectedTokenIds } from "../useSelectedTokenIds";

/** The value of the "no target" choice. */
const NO_TARGET = "";

export interface AttackFlowProps {
  worldId: string;
  /** The creature attacking. Exactly one of this and `lairCombatantId`. */
  attackerTokenId?: string | null;
  /**
   * A lair acting on its count (spec 046 US6): the Game Master's, never
   * queued offline, never a reaction.
   */
  lairCombatantId?: string | null;
  /**
   * What the attack costs, fixed by whoever opened the flow — the tracker's
   * "Spend legendary action" sends `LEGENDARY`. Unset, the ability's own cost
   * applies and the attacker may call it a reaction.
   */
  actionCost?: ActionCost | null;
  abilityId: string;
  abilityName: string;
  /** Every token on the scene, as this viewer was sent them. */
  tokens: TokenRecord[];
  /**
   * The creature the flow opens aimed at — the one right-clicked on the
   * board. Unset, the token selected on the board is the target, if any.
   */
  targetTokenId?: string | null;
  /**
   * Move focus to the target list when the flow opens. On for a flow opened
   * by a button (the sheet's attack); off where the flow opens as an ability
   * is chosen from a list (the tracker), since moving focus out of that list
   * while its choice is still being made would take the list away from a
   * keyboard user.
   */
  focusOnOpen?: boolean;
  /**
   * Closed with Cancel, Done or Escape. The caller returns focus to what
   * opened the flow, which this component cannot know (spec 046 T107).
   */
  onClose: () => void;
}

function tokenLabel(token: TokenRecord): string {
  return token.name?.trim() || "Unnamed creature";
}

/**
 * Choose a target, see the warning, roll (spec 046 US1, FR-033).
 *
 * The target comes from the board — the token selected there — or from the
 * list. Before anything is rolled, `previewAttack` says what the server would
 * record and whether the turn would refuse it; a warning never stops the
 * attacker (clarification 1) except the one thing the server refuses, whose
 * reason is shown here in the server's own words.
 *
 * The roll is the server's. This component sends the attacker, the ability
 * and the target, and shows what comes back; it animates dice it played no
 * part in deciding, exactly as the dice roller does.
 *
 * Offline, the attack is queued as an intent (research R16) and resolved when
 * the connection returns, against the state the server then holds.
 */
export function AttackFlow({
  worldId,
  attackerTokenId = null,
  lairCombatantId = null,
  actionCost = null,
  abilityId,
  abilityName,
  tokens,
  targetTokenId = null,
  focusOnOpen = true,
  onClose,
}: AttackFlowProps) {
  const targetRef = useRef<HTMLSelectElement>(null);
  useEffect(() => {
    if (focusOnOpen) targetRef.current?.focus();
  }, [focusOnOpen]);
  const reactionAllowed = actionCost === null && lairCombatantId === null;
  const selected = useSelectedTokenIds();
  const candidates = useMemo(
    () => tokens.filter((token) => token.tokenId !== attackerTokenId),
    [tokens, attackerTokenId],
  );
  const selectedTarget = selected.find((id) =>
    candidates.some((token) => token.tokenId === id),
  );
  const namedTarget = candidates.some(
    (token) => token.tokenId === targetTokenId,
  )
    ? targetTokenId
    : null;
  const [targetId, setTargetId] = useState<string>(
    namedTarget ?? selectedTarget ?? NO_TARGET,
  );
  const [reaction, setReaction] = useState(false);
  const [preview, setPreview] = useState<AttackPreviewRecord | null>(null);
  const [rolling, setRolling] = useState(false);
  const [result, setResult] = useState<AttackRecord[] | null>(null);
  const [queued, setQueued] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const input: AttackInput = useMemo(
    () => ({
      ...(lairCombatantId ? { lairCombatantId } : { attackerTokenId }),
      abilityId,
      targetTokenId: targetId === NO_TARGET ? null : targetId,
      actionCost:
        actionCost ?? (reaction && reactionAllowed ? "REACTION" : null),
    }),
    [
      attackerTokenId,
      lairCombatantId,
      abilityId,
      targetId,
      reaction,
      actionCost,
      reactionAllowed,
    ],
  );

  useEffect(() => {
    let active = true;
    // Offline there is nobody to ask; the attack is queued and judged later.
    if (shouldQueue()) return;
    previewAttack(input)
      .then((answer) => {
        if (active) setPreview(answer);
      })
      .catch(() => {
        if (active) setPreview(null);
      });
    return () => {
      active = false;
    };
  }, [input]);

  const roll = async () => {
    setRolling(true);
    setError(null);
    setResult(null);
    try {
      if (shouldQueue() && !attackerTokenId) {
        setError("A lair acts only while you are connected");
        return;
      }
      if (shouldQueue() && attackerTokenId) {
        const attempt = await queueEdit({
          worldId,
          localId: crypto.randomUUID(),
          kind: "attack",
          command: {
            type: "make_attack",
            token: { id: attackerTokenId },
            attack: {
              abilityId,
              targetTokenId: input.targetTokenId,
              actionCost: input.actionCost,
            },
          },
          isGameMaster: false,
        });
        if (attempt.queued) {
          setQueued(true);
        } else {
          setError(attempt.explanation ?? "The attack could not be queued");
        }
        return;
      }
      const made = await makeAttack(input);
      for (const attack of made) {
        void triggerDiceRollAnimation(
          attack.toHit.dice.map((die) => ({ finalValue: die.finalValue })),
        );
      }
      setResult(made);
    } catch (err) {
      setError(err instanceof Error ? err.message : "The attack failed");
    } finally {
      setRolling(false);
    }
  };

  return (
    <section
      aria-label={`Attack with ${abilityName}`}
      data-testid="attack-flow"
      onKeyDown={(event) => {
        // Escape leaves the flow from anywhere inside it, as a dialog would;
        // an open list takes its own Escape first.
        if (event.key === "Escape" && !rolling) {
          event.preventDefault();
          event.stopPropagation();
          onClose();
        }
      }}
      className="grid gap-2 rounded-lg border border-border p-2 text-xs"
    >
      <h3 className="text-sm font-semibold">Attack with {abilityName}</h3>
      <label className="grid gap-1">
        <span className="text-muted-foreground">Target</span>
        <select
          ref={targetRef}
          value={targetId}
          onChange={(event) => setTargetId(event.target.value)}
          data-testid="attack-flow-target"
          className="rounded border border-border bg-background px-2 py-1"
        >
          <option value={NO_TARGET}>No target (a roll into the air)</option>
          {candidates.map((token) => (
            <option key={token.tokenId} value={token.tokenId}>
              {tokenLabel(token)}
            </option>
          ))}
        </select>
      </label>
      {selectedTarget && selectedTarget !== targetId ? (
        <Button
          type="button"
          size="sm"
          variant="ghost"
          onClick={() => setTargetId(selectedTarget)}
          data-testid="attack-flow-use-selected"
        >
          Use the token selected on the board
        </Button>
      ) : null}
      {reactionAllowed ? (
        <label className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={reaction}
            onChange={(event) => setReaction(event.target.checked)}
            data-testid="attack-flow-reaction"
          />
          This is a reaction
        </label>
      ) : null}

      {preview && !preview.turn.allowed ? (
        <p
          role="alert"
          className="text-destructive"
          data-testid="attack-flow-warning"
        >
          It is {preview.turn.activeLabel ?? "Unknown"}&apos;s turn, so this
          attack will be refused unless it is a reaction.
        </p>
      ) : null}
      {/* After the turn: reach and range warn, and never stand in front of
          the one thing that refuses (spec 046 C1, FR-033). */}
      {preview && preview.flags.length > 0 ? (
        // The live region wraps the list rather than being it: a list given
        // another role stops being a list, and its items lose their parent.
        <div role="alert">
          <ul
            className="grid gap-0.5 text-amber-700 dark:text-amber-400"
            data-testid="attack-flow-flags"
          >
            {warningTexts(preview).map((warning) => (
              <li key={warning} data-testid="attack-flow-flag">
                {warning}
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      <div className="flex gap-2">
        <Button
          type="button"
          size="sm"
          // Not `disabled` while rolling: disabling the focused button takes
          // focus from it, and a keyboard user is left on the page.
          aria-disabled={rolling}
          onClick={() => {
            if (!rolling) void roll();
          }}
          data-testid="attack-flow-confirm"
        >
          {rolling ? "Rolling…" : "Roll attack"}
        </Button>
        <Button
          type="button"
          size="sm"
          variant="secondary"
          onClick={onClose}
          data-testid="attack-flow-close"
        >
          {result || queued ? "Done" : "Cancel"}
        </Button>
      </div>

      {result ? (
        <div role="status">
          <ul className="grid gap-1" data-testid="attack-flow-result">
            {result.map((attack) => (
              <li key={attack.id}>{attackSummary(attack)}</li>
            ))}
          </ul>
        </div>
      ) : null}
      {queued ? (
        <p role="status" data-testid="attack-flow-queued">
          You are offline. The attack is queued and will be made when you
          reconnect, if it is still your turn.
        </p>
      ) : null}
      {error ? (
        <p
          role="alert"
          className="text-destructive"
          data-testid="attack-flow-error"
        >
          {error}
        </p>
      ) : null}
    </section>
  );
}
