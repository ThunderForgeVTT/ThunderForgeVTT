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
