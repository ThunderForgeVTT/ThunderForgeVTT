import { useEffect, useState } from "react";
import { setAbilityAttack, setItemAttack } from "@/api/attacks";
import { postGraphQL } from "@/api/graphqlClient";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import type { ActionCost, AttackFields } from "@/types/attack";

const ATTACK_FIELD_SELECTION = `
  reach
  rangeNormal
  rangeLong
  needsLineOfSight
  actionCost
  legendaryCost
  multiattack
`;

const ACTION_COSTS: { value: ActionCost; label: string }[] = [
  { value: "ACTION", label: "Action" },
  { value: "BONUS_ACTION", label: "Bonus action" },
  { value: "REACTION", label: "Reaction" },
  { value: "LEGENDARY", label: "Legendary action" },
  { value: "FREE", label: "Free" },
];

export type AttackFieldsOwner =
  | { kind: "ability"; id: string }
  | { kind: "item"; id: string };

function loadFields(owner: AttackFieldsOwner): Promise<AttackFields> {
  if (owner.kind === "ability") {
    return postGraphQL<{ ability: AttackFields }>(
      `
        query AbilityAttackFields($abilityId: UUID!) {
          ability(abilityId: $abilityId) {
            ${ATTACK_FIELD_SELECTION}
          }
        }
      `,
      { abilityId: owner.id },
    ).then((data) => data.ability);
  }
  return postGraphQL<{ item: AttackFields }>(
    `
      query ItemAttackFields($itemId: UUID!) {
        item(itemId: $itemId) {
          ${ATTACK_FIELD_SELECTION}
        }
      }
    `,
    { itemId: owner.id },
  ).then((data) => data.item);
}

/** A number field's text as a nullable number: blank clears it. */
function parseDistance(text: string): number | null {
  const trimmed = text.trim();
  if (trimmed === "") return null;
  const value = Number(trimmed);
  return Number.isFinite(value) ? value : null;
}

export interface AttackFieldsEditorProps {
  owner: AttackFieldsOwner;
  canEdit: boolean;
}

/**
 * What an ability or item is as an attack (spec 046 research R2, T061).
 *
 * Reach and range belong to the attack, never to the creature: an ogre is
 * Large and its greatclub reaches five feet. Distances are in the world's game
 * system's units (feet for 5e). Nothing here refuses an attack at the table —
 * reach, range and sight are flagged when an attack is made (Phase 7), and
 * only turn order is ever refused.
 *
 * Beside the effect editor rather than inside it: an effect is one roll, and
 * these describe the whole ability.
 */
export function AttackFieldsEditor({
  owner,
  canEdit,
}: AttackFieldsEditorProps) {
  const [fields, setFields] = useState<AttackFields | null>(null);
  const [reach, setReach] = useState("");
  const [rangeNormal, setRangeNormal] = useState("");
  const [rangeLong, setRangeLong] = useState("");
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    loadFields(owner)
      .then((loaded) => {
        if (!active) return;
        setFields(loaded);
        setReach(loaded.reach === null ? "" : String(loaded.reach));
        setRangeNormal(
          loaded.rangeNormal === null ? "" : String(loaded.rangeNormal),
        );
        setRangeLong(loaded.rangeLong === null ? "" : String(loaded.rangeLong));
      })
      .catch(() => {
        if (active) setError("Could not load this attack's details");
      });
    return () => {
      active = false;
    };
    // `owner` is a fresh object each render; its identity is kind and id.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [owner.kind, owner.id]);

  if (!fields) {
    return error ? (
      <Card className="p-5 text-sm text-destructive">{error}</Card>
    ) : null;
  }

  const save = async () => {
    setSaving(true);
    setError(null);
    setMessage(null);
    const next: AttackFields = {
      ...fields,
      reach: parseDistance(reach),
      rangeNormal: parseDistance(rangeNormal),
      rangeLong: parseDistance(rangeLong),
    };
    try {
      if (owner.kind === "ability") {
        await setAbilityAttack(owner.id, next);
      } else {
        await setItemAttack(owner.id, next);
      }
      setFields(next);
      setMessage("Saved");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Saving failed");
    } finally {
      setSaving(false);
    }
  };

  return (
    <Card className="grid gap-3 p-5" data-testid="attack-fields-editor">
      <h2 className="text-lg font-semibold">As an attack</h2>
      <p className="text-sm text-muted-foreground">
        Distances are in the game system&apos;s units. Leave reach and range
        blank for an attack that declares none.
      </p>
      <div className="grid gap-3 sm:grid-cols-3">
        <label className="grid gap-1 text-sm">
          Reach
          <input
            type="number"
            min={0}
            inputMode="decimal"
            value={reach}
            disabled={!canEdit}
            onChange={(e) => setReach(e.target.value)}
            data-testid="attack-fields-reach"
            className="rounded border border-border bg-background px-2 py-1"
          />
        </label>
        <label className="grid gap-1 text-sm">
          Normal range
          <input
            type="number"
            min={0}
            inputMode="decimal"
            value={rangeNormal}
            disabled={!canEdit}
            onChange={(e) => setRangeNormal(e.target.value)}
            data-testid="attack-fields-range-normal"
            className="rounded border border-border bg-background px-2 py-1"
          />
        </label>
        <label className="grid gap-1 text-sm">
          Long range
          <input
            type="number"
            min={0}
            inputMode="decimal"
            value={rangeLong}
            disabled={!canEdit}
            onChange={(e) => setRangeLong(e.target.value)}
            data-testid="attack-fields-range-long"
            className="rounded border border-border bg-background px-2 py-1"
          />
        </label>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        <label className="grid gap-1 text-sm">
          Costs
          <select
            value={fields.actionCost}
            disabled={!canEdit}
            onChange={(e) =>
              setFields({ ...fields, actionCost: e.target.value as ActionCost })
            }
            data-testid="attack-fields-action-cost"
            className="rounded border border-border bg-background px-2 py-1"
          >
            {ACTION_COSTS.map((cost) => (
              <option key={cost.value} value={cost.value}>
                {cost.label}
              </option>
            ))}
          </select>
        </label>
        <label className="grid gap-1 text-sm">
          Legendary actions spent
          <input
            type="number"
            min={0}
            step={1}
            value={fields.legendaryCost}
            disabled={!canEdit}
            onChange={(e) =>
              setFields({
                ...fields,
                legendaryCost: Math.max(0, Math.trunc(Number(e.target.value))),
              })
            }
            data-testid="attack-fields-legendary-cost"
            className="rounded border border-border bg-background px-2 py-1"
          />
        </label>
      </div>
      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={fields.needsLineOfSight}
          disabled={!canEdit}
          onChange={(e) =>
            setFields({ ...fields, needsLineOfSight: e.target.checked })
          }
          data-testid="attack-fields-line-of-sight"
        />
        Needs to see its target
      </label>
      <label className="grid gap-1 text-sm">
        Multiattack: the ids of the abilities one use makes, in order, one per
        line
        <textarea
          rows={2}
          value={fields.multiattack.join("\n")}
          disabled={!canEdit}
          onChange={(e) =>
            setFields({
              ...fields,
              multiattack: e.target.value
                .split(/\s+/)
                .map((part) => part.trim())
                .filter(Boolean),
            })
          }
          data-testid="attack-fields-multiattack"
          className="rounded border border-border bg-background px-2 py-1 font-mono text-xs"
        />
      </label>
      {canEdit ? (
        <div className="flex items-center gap-3">
          <Button
            type="button"
            size="sm"
            disabled={saving}
            onClick={() => void save()}
            data-testid="attack-fields-save"
          >
            {saving ? "Saving…" : "Save attack details"}
          </Button>
          {message ? (
            <span className="text-sm text-muted-foreground" role="status">
              {message}
            </span>
          ) : null}
        </div>
      ) : null}
      {error ? (
        <p className="text-sm text-destructive" role="alert">
          {error}
        </p>
      ) : null}
    </Card>
  );
}
