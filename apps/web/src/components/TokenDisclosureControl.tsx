import React, { useState } from "react";
import { setTokenDisclosure } from "@/api/tokenStatus";
import type { DisclosureState } from "@/engine/sdk/DisclosureState";

/**
 * What one token discloses about one resource, as the Game Master sets it.
 *
 * Spec 029 FR-013a. Four states, and they are deliberately **not** presented
 * as four interchangeable appearances.
 *
 * `percentage` leaks materially more than it looks like it does: a player who
 * knows the damage they dealt can divide it by the change, recover the
 * maximum, and read exact values from then on. `chunked` resists that because
 * a quarter index rarely moves on a single hit. A control that listed the four
 * as equals would be inviting a Game Master to pick the leaky one for a reason
 * — readability — that `chunked` serves nearly as well.
 *
 * So the list is ordered by how much it gives away, and the one with a caveat
 * carries the caveat.
 */

export interface TokenDisclosureControlProps {
  tokenId: string;
  resourceId: string;
  resourceLabel: string;
  current: DisclosureState;
  onChanged?: (state: DisclosureState) => void;
}

/** Ordered least to most revealing, with the caveat where it belongs. */
const STATES: {
  value: DisclosureState;
  label: string;
  hint: string;
}[] = [
  {
    value: "greyed",
    label: "Present only",
    hint: "Players see that this exists and nothing about its value.",
  },
  {
    value: "chunked",
    label: "To the quarter",
    hint: "Enough to play on — “nearly dead” — without giving away figures.",
  },
  {
    value: "percentage",
    label: "As a percentage",
    hint: "A player who knows the damage they dealt can work back to the exact numbers.",
  },
  {
    value: "visible",
    label: "Exact",
    hint: "Players see the real current and maximum.",
  },
];

export function TokenDisclosureControl({
  tokenId,
  resourceId,
  resourceLabel,
  current,
  onChanged,
}: TokenDisclosureControlProps): React.ReactElement {
  const [state, setState] = useState<DisclosureState>(current);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selected = STATES.find((s) => s.value === state) ?? STATES[1];

  return (
    <div className="token-disclosure">
      <label htmlFor={`disclosure-${tokenId}-${resourceId}`}>
        {resourceLabel} — what players see
      </label>
      <select
        id={`disclosure-${tokenId}-${resourceId}`}
        data-testid={`token-disclosure-${tokenId}-${resourceId}`}
        value={state}
        disabled={saving}
        onChange={(e) => {
          const next = e.target.value as DisclosureState;
          const previous = state;
          setState(next);
          setSaving(true);
          setError(null);
          void setTokenDisclosure(tokenId, resourceId, next)
            .then(() => onChanged?.(next))
            .catch((err: unknown) => {
              // Put the control back where it was. Leaving it showing a state
              // the server rejected would tell a Game Master their table sees
              // something it does not — the one lie this control must never
              // tell.
              setState(previous);
              setError(
                err instanceof Error ? err.message : "Could not change this",
              );
            })
            .finally(() => setSaving(false));
        }}
      >
        {STATES.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
      <p className="token-disclosure-hint">{selected.hint}</p>
      {error && (
        <p
          className="token-disclosure-error"
          role="alert"
          data-testid={`token-disclosure-error-${tokenId}`}
        >
          {error}
        </p>
      )}
    </div>
  );
}
