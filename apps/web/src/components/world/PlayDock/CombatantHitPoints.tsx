import { useId, useState } from "react";
import type { CombatantRecord, HitPointChange } from "@/types/combat";

/**
 * Spec 046 FR-014: the Game Master's Damage and Heal on one combatant.
 *
 * A number and two buttons. The arithmetic is the server's — temporary hit
 * points first, stopping at zero and at the maximum — so nothing here guesses
 * at what the result will be; the bars move when the world event says they
 * did, on this board and every other.
 *
 * Only for a combatant with a token: `changeHitPoints` addresses the creature
 * on the board, and a combatant added from the actor list has none.
 */
export function CombatantHitPoints({
  combatant,
  busy,
  onChange,
}: {
  combatant: CombatantRecord;
  busy: boolean;
  onChange: (kind: HitPointChange, amount: number) => void;
}) {
  const [amount, setAmount] = useState("");
  const inputId = useId();
  const parsed = Number.parseInt(amount, 10);
  const valid = Number.isFinite(parsed) && parsed >= 0;

  const submit = (kind: HitPointChange) => {
    if (!valid) return;
    onChange(kind, parsed);
    setAmount("");
  };

  return (
    <form
      className="flex items-center gap-1"
      data-testid="combatant-hit-points"
      onSubmit={(event) => {
        // Enter in the number applies damage: the common case at a table.
        event.preventDefault();
        submit("DAMAGE");
      }}
    >
      <label htmlFor={inputId} className="sr-only">
        Hit points to change for {combatant.label}
      </label>
      <input
        id={inputId}
        type="number"
        min={0}
        inputMode="numeric"
        value={amount}
        placeholder="HP"
        data-testid="combatant-hp-amount"
        className="h-7 w-14 rounded border border-input bg-transparent px-1 text-sm tabular-nums outline-none"
        onChange={(event) => setAmount(event.target.value)}
      />
      <button
        type="submit"
        disabled={busy || !valid}
        aria-label={`Damage ${combatant.label}`}
        data-testid="combatant-damage-button"
        className="rounded px-1.5 py-0.5 text-xs text-muted-foreground hover:bg-muted hover:text-destructive disabled:opacity-50"
      >
        Damage
      </button>
      <button
        type="button"
        disabled={busy || !valid}
        aria-label={`Heal ${combatant.label}`}
        data-testid="combatant-heal-button"
        className="rounded px-1.5 py-0.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-50"
        onClick={() => submit("HEALING")}
      >
        Heal
      </button>
    </form>
  );
}

/**
 * Spec 046 T018: why a combatant is out, said in words.
 *
 * A creature that dropped to zero and one the Game Master took out look the
 * same greyed out, and they behave differently: healing brings back the first
 * and not the second. So the row says which.
 */
export function CombatantOutMark({
  combatant,
}: {
  combatant: CombatantRecord;
}) {
  if (combatant.active) return null;
  const text =
    combatant.downedBy === "HIT_POINTS" ? "Out: 0 hit points" : "Down";
  return (
    <span
      className="text-xs font-semibold text-muted-foreground"
      data-testid="combatant-out"
      data-downed-by={combatant.downedBy ?? "UNKNOWN"}
      aria-label={`${combatant.label}: ${text}`}
    >
      {text}
    </span>
  );
}
