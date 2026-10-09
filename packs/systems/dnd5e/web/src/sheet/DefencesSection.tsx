import { Button } from "@thunderforge/host";
import { DEFENCES, label, type DefenceKey } from "./data";
import {
  fieldClass,
  hintClass,
  sectionHeadingClass,
} from "../components/styles";

/**
 * Spec 048: the damage a character shrugs off, ignores or suffers double
 * from, and the conditions that cannot take hold of it. Each list names only
 * the damage types and conditions this system declares.
 */
export default function DefencesSection({
  defences,
  canEdit,
  disabled,
  onSave,
}: {
  defences: Record<DefenceKey, string[]>;
  canEdit: boolean;
  disabled: boolean;
  onSave: (key: DefenceKey, ids: string[]) => void;
}) {
  return (
    <div className="grid gap-3" data-testid="dnd5e-defences">
      {DEFENCES.map(({ key, label: title, choices }) => {
        const chosen = defences[key];
        const left = choices.filter((id) => !chosen.includes(id));
        if (!canEdit && chosen.length === 0) return null;
        return (
          <div key={key} className="grid gap-1" data-testid={`dnd5e-${key}`}>
            <div className={sectionHeadingClass}>{title}</div>
            <div className="flex flex-wrap items-center gap-1.5">
              {chosen.length === 0 ? (
                <span className={hintClass}>None.</span>
              ) : null}
              {chosen.map((id) =>
                canEdit ? (
                  <Button
                    key={id}
                    type="button"
                    variant="secondary"
                    size="sm"
                    disabled={disabled}
                    aria-label={`Remove ${label(id)} from ${title}`}
                    onClick={() =>
                      onSave(
                        key,
                        chosen.filter((other) => other !== id),
                      )
                    }
                  >
                    {label(id)} ×
                  </Button>
                ) : (
                  <span
                    key={id}
                    className="rounded-md border border-border px-2 py-0.5 text-sm"
                  >
                    {label(id)}
                  </span>
                ),
              )}
              {canEdit && left.length > 0 ? (
                <select
                  aria-label={`Add to ${title}`}
                  className={`${fieldClass} w-auto`}
                  value=""
                  disabled={disabled}
                  onChange={(event) => {
                    const id = event.target.value;
                    if (id) {
                      onSave(
                        key,
                        choices.filter((c) => c === id || chosen.includes(c)),
                      );
                    }
                  }}
                >
                  <option value="">Add…</option>
                  {left.map((id) => (
                    <option key={id} value={id}>
                      {label(id)}
                    </option>
                  ))}
                </select>
              ) : null}
            </div>
          </div>
        );
      })}
      {!canEdit && DEFENCES.every(({ key }) => defences[key].length === 0) ? (
        <span className={hintClass}>
          No resistances, immunities or vulnerabilities.
        </span>
      ) : null}
    </div>
  );
}
