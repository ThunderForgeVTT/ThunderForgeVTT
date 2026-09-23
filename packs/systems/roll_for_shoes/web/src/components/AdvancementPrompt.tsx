import { useState } from "react";
import { Button, Input, StatusBadge } from "@thunderforge/host";

import type { Skill } from "../game.ts";
import { cardTitleClass, hintClass } from "./styles.ts";

export interface AdvancementPromptProps {
  /** The skill that was rolled; the new one grows out of it. */
  parent: Skill;
  busy: boolean;
  onConfirm: (name: string) => void;
  onDecline: () => void;
  /**
   * Room left at the level this new skill would sit at: `null` where the level
   * is uncapped, which is the case in every world that has not turned skill
   * slots on. Zero or less means the advancement happened and has nowhere to go.
   */
  room?: number | null;
  /** The character's experience, and what a slot at that level costs. */
  xp?: number;
  slotCost?: number;
  onBuySlot?: () => void;
}

/**
 * Every die showed a six, so the character has learnt something.
 *
 * The prompt says what the table is looking for — something narrower than the
 * skill just rolled, and true to what was actually attempted — and then
 * accepts whatever the player types. Judging the answer would be this product
 * arbitrating a call the people at the table are there to make.
 *
 * An empty name is the one thing refused, and it is refused here rather than
 * by the server, because there is nothing to send.
 */
export function AdvancementPrompt({
  parent,
  busy,
  onConfirm,
  onDecline,
  room = null,
  xp = 0,
  slotCost = 0,
  onBuySlot,
}: AdvancementPromptProps) {
  const [name, setName] = useState("");
  const [refusal, setRefusal] = useState<string | null>(null);

  const confirm = () => {
    if (name.trim().length === 0) {
      setRefusal("Give the new skill a name.");
      return;
    }
    setRefusal(null);
    onConfirm(name);
  };

  // The level the new skill would sit at is full.
  //
  // The character is **told**, rather than silently denied (FR-030). The
  // advancement happened — every die came up a six and that is not taken back —
  // and what is missing is somewhere to put it. Saying so is the difference
  // between a rule and a bug.
  if (room !== null && room <= 0) {
    return (
      <section data-testid="rfs-advancement-no-room" className="grid gap-2">
        <h3 className={cardTitleClass}>Every die showed a six</h3>
        <p className={hintClass}>
          There is no room at level {parent.level + 1} — this table limits how
          many skills sit at each level, and that one is full. The roll still
          earned this; it has nowhere to go until room is made.
        </p>

        <div className="flex flex-wrap items-center gap-2">
          {onBuySlot ? (
            <Button
              type="button"
              size="sm"
              disabled={busy || xp < slotCost}
              data-testid="rfs-buy-slot"
              onClick={onBuySlot}
            >
              Buy room for {slotCost} XP
            </Button>
          ) : null}
          <Button
            type="button"
            size="sm"
            variant="secondary"
            disabled={busy}
            data-testid="rfs-advancement-decline"
            onClick={onDecline}
          >
            Not this time
          </Button>
        </div>

        {xp < slotCost ? (
          <p className={hintClass} data-testid="rfs-no-room-short">
            A level {parent.level + 1} slot costs {slotCost} XP, and they have{" "}
            {xp}.
          </p>
        ) : null}
      </section>
    );
  }

  return (
    <section data-testid="rfs-advancement" className="grid gap-2">
      <h3 className={cardTitleClass}>Every die showed a six</h3>
      <p className={hintClass}>
        Name the skill this earns. It sits at level {parent.level + 1}, one
        above {parent.name}, and the table expects something narrower than{" "}
        {parent.name} and true to what was just attempted.
      </p>

      <Input
        value={name}
        disabled={busy}
        placeholder="What did they just learn?"
        data-testid="rfs-advancement-name"
        onChange={(event) => setName(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            confirm();
          }
        }}
      />

      {refusal ? <StatusBadge variant="warning">{refusal}</StatusBadge> : null}

      <div className="flex gap-2">
        <Button
          type="button"
          size="sm"
          disabled={busy}
          data-testid="rfs-advancement-confirm"
          onClick={confirm}
        >
          Learn it
        </Button>
        <Button
          type="button"
          size="sm"
          variant="secondary"
          disabled={busy}
          data-testid="rfs-advancement-decline"
          onClick={onDecline}
        >
          Not this time
        </Button>
      </div>
    </section>
  );
}

export default AdvancementPrompt;
