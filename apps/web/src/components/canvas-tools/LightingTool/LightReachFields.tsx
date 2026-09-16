import { useState } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { SceneUnits } from "@/types/light";
import {
  afterCommit,
  currentReaches,
  reachChanges,
  type PendingReaches,
  type Reaches,
} from "./reachCommit";
import { toSystemUnits, toWorldUnits } from "./sceneUnits";

export interface LightReachFieldsProps {
  /** Which light, so a newly selected one starts from its own values. */
  lightId: string;
  /** The dim reach, in world units. */
  radius: number;
  /** The bright reach, in world units. */
  brightRadius: number;
  units: SceneUnits;
  /** Both reaches, in world units, once one of them is committed. */
  onChange: (changes: { radius?: number; brightRadius?: number }) => void;
}

/**
 * What has been typed and not yet committed — only the fields actually typed
 * in. A field left alone reads the light's current value, so a reach committed
 * a moment ago is not overwritten by the figure that was showing when the
 * other field was started.
 */
type Draft = { lightId: string; bright?: string; dim?: string } | null;

/** Reaches committed for a light and not yet back from the server. */
type Pending = { lightId: string; reaches: PendingReaches } | null;

/**
 * A placed light's bright and dim reach, in the game system's units
 * (spec 045 FR-061) — "20 ft bright, 40 ft dim", as a Game Master reads a
 * torch off a rulebook, not the world units the board draws in.
 *
 * Committed on Enter or on leaving the field, not per keystroke. Typing "40"
 * over a dim reach passes through "4" first, and sent at once that "4" would
 * pull the bright reach in to meet it; the server keeps a bright reach within
 * the dim one, so the half-typed number would have changed the light.
 *
 * A bright reach typed past the dim reach is refused here, with the reason
 * next to the field, rather than sent for the server to refuse. It is judged
 * against the dim reach just committed, not the one the server last sent: a
 * Game Master who sets 40 ft dim and then 20 ft bright, faster than the server
 * answers, has not typed a bright reach past a 10 ft dim one (see
 * `reachCommit.ts`).
 */
export function LightReachFields({
  lightId,
  radius,
  brightRadius,
  units,
  onChange,
}: LightReachFieldsProps) {
  const [draft, setDraft] = useState<Draft>(null);
  const [pendingState, setPending] = useState<Pending>(null);
  const [problem, setProblem] = useState<string | null>(null);

  const stored: Reaches = { radius, brightRadius };
  const pending =
    pendingState && pendingState.lightId === lightId
      ? pendingState.reaches
      : {};
  // Worked out as it is read, not remembered: a reach stops standing in the
  // moment the stored one moves, or once it is too old to be an answer still
  // to come (`reachCommit.ts`), and another light has none of its own.
  const current = currentReaches(stored, pending);
  const showing = {
    bright: String(toSystemUnits(current.brightRadius, units)),
    dim: String(toSystemUnits(current.radius, units)),
  };
  // A draft belongs to the light it was typed for; selecting another light
  // shows that light's own values.
  const shown = draft && draft.lightId === lightId ? draft : null;
  const bright = shown?.bright ?? showing.bright;
  const dim = shown?.dim ?? showing.dim;

  const commit = () => {
    if (!shown) return;
    const brightValue = Number(bright);
    const dimValue = Number(dim);
    if (
      bright.trim() === "" ||
      dim.trim() === "" ||
      !Number.isFinite(brightValue) ||
      !Number.isFinite(dimValue) ||
      brightValue < 0 ||
      dimValue <= 0
    ) {
      setProblem(
        "Reaches are distances: a dim reach above zero, and a bright reach of zero or more.",
      );
      return;
    }
    if (brightValue > dimValue) {
      setProblem(
        "A light cannot be bright further out than it reaches at all.",
      );
      return;
    }
    setProblem(null);
    setDraft(null);
    const typed: Partial<Reaches> = {};
    if (shown.dim !== undefined && shown.dim !== showing.dim) {
      typed.radius = toWorldUnits(dimValue, units);
    }
    if (shown.bright !== undefined && shown.bright !== showing.bright) {
      typed.brightRadius = toWorldUnits(brightValue, units);
    }
    // Both, when both moved or the other is still on its way, so the server
    // never judges one against the other's old value.
    const changes = reachChanges(typed, stored, pending);
    if (changes.radius !== undefined || changes.brightRadius !== undefined) {
      setPending({ lightId, reaches: afterCommit(changes, stored, pending) });
      onChange(changes);
    }
  };

  const edit = (field: "bright" | "dim", value: string) => {
    setDraft({ ...(shown ?? { lightId }), lightId, [field]: value });
  };

  const onKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      event.preventDefault();
      commit();
    }
  };

  return (
    <div className="grid gap-2" data-testid="light-reach">
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label htmlFor="light-bright-reach">
            Bright reach ({units.label})
          </Label>
          <Input
            id="light-bright-reach"
            data-testid="light-bright-reach"
            type="number"
            inputMode="decimal"
            min={0}
            step={units.perCell}
            value={bright}
            aria-invalid={problem !== null}
            aria-describedby={problem ? "light-reach-problem" : undefined}
            onChange={(event) => edit("bright", event.target.value)}
            onBlur={commit}
            onKeyDown={onKeyDown}
          />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="light-dim-reach">Dim reach ({units.label})</Label>
          <Input
            id="light-dim-reach"
            data-testid="light-dim-reach"
            type="number"
            inputMode="decimal"
            min={0}
            step={units.perCell}
            value={dim}
            aria-invalid={problem !== null}
            aria-describedby={problem ? "light-reach-problem" : undefined}
            onChange={(event) => edit("dim", event.target.value)}
            onBlur={commit}
            onKeyDown={onKeyDown}
          />
        </div>
      </div>
      {problem ? (
        <p
          id="light-reach-problem"
          role="alert"
          className="text-xs text-destructive"
          data-testid="light-reach-problem"
        >
          {problem}
        </p>
      ) : null}
    </div>
  );
}
