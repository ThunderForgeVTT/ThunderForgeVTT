import { useState } from "react";
import { COINS, type CoinKey, type Coins } from "./data";
import { fieldClass, sectionHeadingClass } from "../components/styles";

/**
 * Spec 048: the character's purse, one count per coin. Counts are whole and
 * never negative; each saves when it loses focus.
 */
export default function CoinsSection({
  coins,
  canEdit,
  disabled,
  onSave,
}: {
  coins: Coins;
  canEdit: boolean;
  disabled: boolean;
  onSave: (coins: Coins) => void;
}) {
  return (
    <dl
      className="grid grid-cols-5 gap-2 text-center"
      data-testid="dnd5e-coins"
    >
      {COINS.map(({ key, label }) => (
        <div key={key} className="grid gap-1">
          <dt>
            <label
              htmlFor={canEdit ? `dnd5e-coins-${key}` : undefined}
              className={sectionHeadingClass}
              title={label}
            >
              {key.toUpperCase()}
            </label>
          </dt>
          <dd className="grid">
            {canEdit ? (
              <CoinDraft
                key={coins[key]}
                coin={key}
                label={label}
                value={coins[key]}
                disabled={disabled}
                onCommit={(count) => onSave({ ...coins, [key]: count })}
              />
            ) : (
              <span
                className="font-medium tabular-nums"
                data-testid={`dnd5e-coins-${key}`}
                aria-label={`${label}: ${coins[key]}`}
              >
                {coins[key]}
              </span>
            )}
          </dd>
        </div>
      ))}
    </dl>
  );
}

function CoinDraft({
  coin,
  label,
  value,
  disabled,
  onCommit,
}: {
  coin: CoinKey;
  label: string;
  value: number;
  disabled: boolean;
  onCommit: (count: number) => void;
}) {
  const [draft, setDraft] = useState(String(value));
  const commit = () => {
    const count = Math.max(0, Math.trunc(Number(draft)));
    if (!Number.isFinite(count)) {
      setDraft(String(value));
    } else if (count !== value) {
      onCommit(count);
    }
  };
  return (
    <input
      id={`dnd5e-coins-${coin}`}
      type="number"
      min={0}
      step={1}
      inputMode="numeric"
      aria-label={label}
      className={`${fieldClass} text-center tabular-nums`}
      value={draft}
      disabled={disabled}
      data-testid={`dnd5e-coins-${coin}-input`}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
    />
  );
}
