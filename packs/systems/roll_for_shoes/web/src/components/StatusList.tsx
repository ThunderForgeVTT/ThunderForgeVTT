import { useState } from "react";
import { Button, Input } from "@thunderforge/host";

import { statusModifier, type Status } from "../game.ts";
import { cardTitleClass, fieldClass, hintClass } from "./styles.ts";

export interface StatusListProps {
  statuses: Status[];
  canEdit: boolean;
  busy: boolean;
  onAdd: (name: string, modifier: number) => void;
  onRemove: (id: string) => void;
}

/** A signed number written the way a modifier is read: +2, −2, 0. */
function signed(modifier: number): string {
  if (modifier > 0) {
    return `+${modifier}`;
  }
  // A true minus sign, not a hyphen — these sit beside dice and are read fast.
  return modifier < 0 ? `−${Math.abs(modifier)}` : "0";
}

/**
 * What a character is currently carrying.
 *
 * The system ships no list of statuses and judges none of them, exactly as it
 * judges no skill name: what counts as a condition, and what it is worth, is
 * the table's call. So this is a list the table writes, not a menu it picks
 * from.
 *
 * The modifier is applied flat to the roll's total — never to a die, and never
 * to the count of dice rolled. That is why a status can neither create nor
 * destroy an advancement, and why nothing here is drawn as a die.
 */
export function StatusList({
  statuses,
  canEdit,
  busy,
  onAdd,
  onRemove,
}: StatusListProps) {
  const [name, setName] = useState("");
  const [modifier, setModifier] = useState("-1");

  const total = statusModifier(statuses);

  return (
    <section className="grid gap-2" data-testid="rfs-statuses">
      <div className="flex items-baseline justify-between gap-3">
        <h3 className={cardTitleClass}>Statuses</h3>
        {statuses.length > 0 ? (
          <p className="text-sm">
            <span className={hintClass}>on every roll </span>
            <span
              data-testid="rfs-statuses-total"
              className="font-semibold tabular-nums"
            >
              {signed(total)}
            </span>
          </p>
        ) : null}
      </div>

      {statuses.length === 0 ? (
        <p className={hintClass} data-testid="rfs-statuses-none">
          Nothing is on them right now.
        </p>
      ) : (
        <ul className="grid gap-1">
          {statuses.map((status) => (
            <li
              key={status.id}
              data-testid={`rfs-status-${status.id}`}
              className="flex items-center justify-between gap-2 text-sm"
            >
              <span>
                {status.name}{" "}
                <span className="font-semibold tabular-nums">
                  {signed(status.modifier)}
                </span>
              </span>
              {canEdit ? (
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  data-testid={`rfs-status-remove-${status.id}`}
                  onClick={() => onRemove(status.id)}
                >
                  Remove
                </Button>
              ) : null}
            </li>
          ))}
        </ul>
      )}

      {canEdit ? (
        <div className="flex flex-wrap items-end gap-2">
          <label className="grid gap-1">
            <span className={hintClass}>What is on them</span>
            <Input
              className={fieldClass}
              placeholder="Wounded, cornered, on fire&hellip;"
              data-testid="rfs-status-name"
              value={name}
              onChange={(event) => setName(event.target.value)}
            />
          </label>
          <label className="grid gap-1">
            <span className={hintClass}>Worth</span>
            <Input
              type="number"
              inputMode="numeric"
              className={`${fieldClass} w-20`}
              data-testid="rfs-status-modifier"
              value={modifier}
              onChange={(event) => setModifier(event.target.value)}
            />
          </label>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            disabled={busy}
            data-testid="rfs-status-add"
            onClick={() => {
              // An unreadable number is a nameless status' cousin: refuse it
              // here rather than storing a NaN the sheet cannot show. `addStatus`
              // refuses it too, and says so — this only keeps the field honest.
              const worth = Math.trunc(Number(modifier));
              onAdd(name, Number.isFinite(worth) ? worth : 0);
              setName("");
              setModifier("-1");
            }}
          >
            Add
          </Button>
        </div>
      ) : null}

      <p className={hintClass}>
        Applied to the total, never to the dice. Every die still has to come up
        a six on its own to earn a new skill.
      </p>
    </section>
  );
}

export default StatusList;
