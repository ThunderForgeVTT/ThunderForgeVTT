import { useState } from "react";
import { addItemEffect, removeItemEffect, updateItemEffect } from "@/api/items";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type {
  ItemEffectRecord,
  ItemEffectTrigger,
  ItemEffectType,
} from "@/types/item";

export interface ItemEffectEditorProps {
  itemId: string;
  effects: ItemEffectRecord[];
  onChanged: (effects: ItemEffectRecord[]) => void;
}

const EFFECT_TYPE_OPTIONS: Array<{ value: ItemEffectType; label: string }> = [
  { value: "HEAL", label: "Heal" },
  { value: "DAMAGE", label: "Damage" },
  { value: "MODIFIER", label: "Modifier (boost/detriment)" },
  { value: "ATTACK_ROLL", label: "Attack Roll" },
];

const TRIGGER_OPTIONS: Array<{ value: "" | ItemEffectTrigger; label: string }> =
  [
    { value: "", label: "Unspecified" },
    { value: "ON_USE", label: "On use (consumable)" },
    { value: "PASSIVE", label: "Passive (always active)" },
  ];

/**
 * Spec 013 (T027): add/edit/remove an Item's structured effects (FR-005).
 * Each effect is authored data only — no dice rolling, no trigger
 * evaluation happens here (FR-004a, Clarifications).
 */
export function ItemEffectEditor({
  itemId,
  effects,
  onChanged,
}: ItemEffectEditorProps) {
  const [draftType, setDraftType] = useState<ItemEffectType>("MODIFIER");
  const [draftFormula, setDraftFormula] = useState("");
  const [draftTarget, setDraftTarget] = useState("");
  const [draftTrigger, setDraftTrigger] = useState<"" | ItemEffectTrigger>("");
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pendingEffectId, setPendingEffectId] = useState<string | null>(null);

  const handleAdd = async () => {
    if (!draftFormula.trim() || !draftTarget.trim()) {
      return;
    }
    setIsSaving(true);
    setError(null);
    try {
      const created = await addItemEffect(itemId, {
        effectType: draftType,
        formula: draftFormula.trim(),
        target: draftTarget.trim(),
        triggerKind: draftTrigger || null,
        sortOrder: effects.length,
      });
      onChanged([...effects, created]);
      setDraftFormula("");
      setDraftTarget("");
      setDraftTrigger("");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to add effect");
    } finally {
      setIsSaving(false);
    }
  };

  const handleRemove = async (effectId: string) => {
    setPendingEffectId(effectId);
    setError(null);
    try {
      await removeItemEffect(effectId);
      onChanged(effects.filter((effect) => effect.id !== effectId));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to remove effect");
    } finally {
      setPendingEffectId(null);
    }
  };

  const handleUpdateField = async (
    effect: ItemEffectRecord,
    patch: Partial<ItemEffectRecord>,
  ) => {
    setPendingEffectId(effect.id);
    setError(null);
    try {
      const updated = await updateItemEffect(effect.id, {
        effectType: patch.effectType ?? effect.effectType,
        formula: patch.formula ?? effect.formula,
        target: patch.target ?? effect.target,
        triggerKind: patch.triggerKind ?? effect.triggerKind,
        sortOrder: effect.sortOrder,
      });
      onChanged(effects.map((e) => (e.id === effect.id ? updated : e)));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to update effect");
    } finally {
      setPendingEffectId(null);
    }
  };

  return (
    <Card className="grid gap-4 p-5" data-testid="item-effect-editor">
      <h3 className="text-lg font-semibold">Effects</h3>

      {error ? <StatusBadge variant="danger">{error}</StatusBadge> : null}

      {effects.length === 0 ? (
        <p className="text-sm text-muted-foreground italic">No effects yet.</p>
      ) : (
        <div className="grid gap-3">
          {effects.map((effect) => (
            <div
              key={effect.id}
              data-testid={`item-effect-row-${effect.id}`}
              className="grid gap-2 rounded-lg border border-border p-3 sm:grid-cols-[1fr_1fr_1fr_1fr_auto]"
            >
              <select
                value={effect.effectType}
                disabled={pendingEffectId === effect.id}
                onChange={(event) =>
                  void handleUpdateField(effect, {
                    effectType: event.target.value as ItemEffectType,
                  })
                }
                className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
                aria-label="Effect type"
              >
                {EFFECT_TYPE_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
              <Input
                defaultValue={effect.formula}
                disabled={pendingEffectId === effect.id}
                onBlur={(event) =>
                  void handleUpdateField(effect, {
                    formula: event.target.value,
                  })
                }
                placeholder="Formula (e.g. 3d6)"
                aria-label="Effect formula"
              />
              <Input
                defaultValue={effect.target}
                disabled={pendingEffectId === effect.id}
                onBlur={(event) =>
                  void handleUpdateField(effect, { target: event.target.value })
                }
                placeholder="Target (e.g. Hit Points)"
                aria-label="Effect target"
              />
              <select
                value={effect.triggerKind ?? ""}
                disabled={pendingEffectId === effect.id}
                onChange={(event) =>
                  void handleUpdateField(effect, {
                    triggerKind: (event.target.value ||
                      null) as ItemEffectTrigger | null,
                  })
                }
                className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
                aria-label="Effect trigger"
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
                data-testid={`item-effect-remove-${effect.id}`}
              >
                Remove
              </Button>
            </div>
          ))}
        </div>
      )}

      <div className="grid gap-2 rounded-lg border border-dashed border-border p-3 sm:grid-cols-[1fr_1fr_1fr_1fr_auto]">
        <select
          value={draftType}
          onChange={(event) =>
            setDraftType(event.target.value as ItemEffectType)
          }
          className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
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
          placeholder="Formula (e.g. 1d20 + STAT + MODIFIERS)"
          disabled={isSaving}
          data-testid="new-item-effect-formula"
        />
        <Input
          value={draftTarget}
          onChange={(event) => setDraftTarget(event.target.value)}
          placeholder="Target (e.g. Hit Points)"
          disabled={isSaving}
          data-testid="new-item-effect-target"
        />
        <select
          value={draftTrigger}
          onChange={(event) =>
            setDraftTrigger(event.target.value as "" | ItemEffectTrigger)
          }
          className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
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
          data-testid="add-item-effect-button"
        >
          Add effect
        </Button>
      </div>
    </Card>
  );
}
