import { useState } from "react";
import {
  addAbilityEffect,
  removeAbilityEffect,
  updateAbilityEffect,
  type AbilityEffectInput,
} from "@/api/abilities";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type {
  AbilityEffectRecord,
  AbilityEffectTrigger,
  AbilityEffectType,
} from "@/types/ability";

export interface AbilityEffectEditorProps {
  abilityId: string;
  /** Controlled by the parent, mirroring `ItemEffectEditor`. */
  effects: AbilityEffectRecord[];
  onChanged: (effects: AbilityEffectRecord[]) => void;
  /**
   * FR-017: effect edits require Editor on the parent ability.
   *
   * The item version has no such prop and renders its editor for VIEWERs in
   * view mode (research.md §3, defect 5). This one is gated — a read-only
   * viewer gets a plain list, not disabled-looking controls that would fail
   * server-side anyway.
   */
  canEdit: boolean;
}

const EFFECT_TYPE_OPTIONS: { value: AbilityEffectType; label: string }[] = [
  { value: "HEAL", label: "Heal" },
  { value: "DAMAGE", label: "Damage" },
  { value: "MODIFIER", label: "Modifier" },
  { value: "ATTACK_ROLL", label: "Attack Roll" },
];

const TRIGGER_OPTIONS: { value: "" | AbilityEffectTrigger; label: string }[] = [
  { value: "", label: "Unspecified" },
  { value: "ON_USE", label: "On use" },
  { value: "PASSIVE", label: "Passive" },
];

/**
 * Spec 025 (T041): add/edit/remove an ability's structured effects.
 *
 * Effects are inert authored data (FR-019) — nothing here rolls or resolves a
 * formula. `triggerKind` is scaffolded per FR-020 and evaluated by nothing;
 * it is offered so authors can record intent for a future resolution spec.
 */
export function AbilityEffectEditor({
  abilityId,
  effects,
  onChanged,
  canEdit,
}: AbilityEffectEditorProps) {
  const [draftType, setDraftType] = useState<AbilityEffectType>("MODIFIER");
  const [draftFormula, setDraftFormula] = useState("");
  const [draftTarget, setDraftTarget] = useState("");
  const [draftTrigger, setDraftTrigger] = useState<"" | AbilityEffectTrigger>("");
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pendingEffectId, setPendingEffectId] = useState<string | null>(null);

  const handleAdd = async () => {
    setIsSaving(true);
    setError(null);
    try {
      const created = await addAbilityEffect(abilityId, {
        effectType: draftType,
        formula: draftFormula,
        target: draftTarget,
        triggerKind: draftTrigger || null,
        sortOrder: effects.length,
      });
      onChanged([...effects, created]);
      setDraftFormula("");
      setDraftTarget("");
      setDraftTrigger("");
    } catch (err) {
      // FR-018's validation errors surface here verbatim.
      setError(err instanceof Error ? err.message : "Failed to add effect");
    } finally {
      setIsSaving(false);
    }
  };

  const handleUpdateField = async (
    effect: AbilityEffectRecord,
    patch: Partial<AbilityEffectInput>,
  ) => {
    setPendingEffectId(effect.id);
    setError(null);
    try {
      const updated = await updateAbilityEffect(effect.id, {
        effectType: patch.effectType ?? effect.effectType,
        formula: patch.formula ?? effect.formula,
        target: patch.target ?? effect.target,
        triggerKind: patch.triggerKind !== undefined ? patch.triggerKind : effect.triggerKind,
        sortOrder: effect.sortOrder,
      });
      onChanged(effects.map((e) => (e.id === updated.id ? updated : e)));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to update effect");
    } finally {
      setPendingEffectId(null);
    }
  };

  const handleRemove = async (effectId: string) => {
    setPendingEffectId(effectId);
    setError(null);
    try {
      await removeAbilityEffect(effectId);
      onChanged(effects.filter((e) => e.id !== effectId));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to remove effect");
    } finally {
      setPendingEffectId(null);
    }
  };

  return (
    <Card className="grid gap-3 p-5" data-testid="ability-effect-editor">
      <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
        Effects
      </p>

      {effects.length === 0 ? (
        <p className="text-sm text-muted-foreground italic">No effects yet.</p>
      ) : (
        <ul className="grid gap-2">
          {effects.map((effect) => (
            <li
              key={effect.id}
              className="grid items-center gap-2 sm:grid-cols-[auto_1fr_1fr_auto_auto]"
              data-testid={`ability-effect-row-${effect.id}`}
            >
              {canEdit ? (
                <>
                  <select
                    className="rounded-md border border-border bg-background px-2 py-1 text-sm"
                    value={effect.effectType}
                    disabled={pendingEffectId === effect.id}
                    aria-label="Effect type"
                    onChange={(event) =>
                      void handleUpdateField(effect, {
                        effectType: event.target.value as AbilityEffectType,
                      })
                    }
                  >
                    {EFFECT_TYPE_OPTIONS.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                  {/* Uncontrolled + blur-to-save, matching ItemEffectEditor —
                      avoids a round-trip per keystroke. */}
                  <Input
                    defaultValue={effect.formula}
                    disabled={pendingEffectId === effect.id}
                    aria-label="Formula"
                    onBlur={(event) => {
                      if (event.target.value !== effect.formula) {
                        void handleUpdateField(effect, { formula: event.target.value });
                      }
                    }}
                  />
                  <Input
                    defaultValue={effect.target}
                    disabled={pendingEffectId === effect.id}
                    aria-label="Target"
                    onBlur={(event) => {
                      if (event.target.value !== effect.target) {
                        void handleUpdateField(effect, { target: event.target.value });
                      }
                    }}
                  />
                  <select
                    className="rounded-md border border-border bg-background px-2 py-1 text-sm"
                    value={effect.triggerKind ?? ""}
                    disabled={pendingEffectId === effect.id}
                    aria-label="Trigger"
                    onChange={(event) =>
                      void handleUpdateField(effect, {
                        triggerKind: (event.target.value || null) as AbilityEffectTrigger | null,
                      })
                    }
                  >
                    {TRIGGER_OPTIONS.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={pendingEffectId === effect.id}
                    onClick={() => void handleRemove(effect.id)}
                    data-testid={`ability-effect-remove-${effect.id}`}
                  >
                    Remove
                  </Button>
                </>
              ) : (
                <span className="col-span-full text-sm text-muted-foreground">
                  <span className="font-medium text-foreground">
                    {EFFECT_TYPE_OPTIONS.find((o) => o.value === effect.effectType)?.label ??
                      effect.effectType}
                  </span>{" "}
                  — {effect.formula} → {effect.target}
                </span>
              )}
            </li>
          ))}
        </ul>
      )}

      {canEdit ? (
        <div className="grid gap-2 sm:grid-cols-[auto_1fr_1fr_auto_auto]">
          <select
            className="rounded-md border border-border bg-background px-2 py-1 text-sm"
            value={draftType}
            onChange={(event) => setDraftType(event.target.value as AbilityEffectType)}
            disabled={isSaving}
            aria-label="New effect type"
          >
            {EFFECT_TYPE_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
          <Input
            value={draftFormula}
            onChange={(event) => setDraftFormula(event.target.value)}
            placeholder="Formula (e.g. 3d6)"
            disabled={isSaving}
            data-testid="new-ability-effect-formula"
          />
          <Input
            value={draftTarget}
            onChange={(event) => setDraftTarget(event.target.value)}
            placeholder="Target (e.g. Hit Points)"
            disabled={isSaving}
            data-testid="new-ability-effect-target"
          />
          <select
            className="rounded-md border border-border bg-background px-2 py-1 text-sm"
            value={draftTrigger}
            onChange={(event) =>
              setDraftTrigger(event.target.value as "" | AbilityEffectTrigger)
            }
            disabled={isSaving}
            aria-label="New effect trigger"
          >
            {TRIGGER_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
          <Button
            type="button"
            size="sm"
            onClick={() => void handleAdd()}
            disabled={isSaving || !draftFormula.trim() || !draftTarget.trim()}
            data-testid="add-ability-effect-button"
          >
            Add Effect
          </Button>
        </div>
      ) : null}

      {error ? <StatusBadge variant="danger">{error}</StatusBadge> : null}
    </Card>
  );
}
