import { useId, useState } from "react";
import {
  applyActorCondition,
  clearActorCondition,
  type WorldSystemCondition,
} from "@/api/actorConditions";
import { Button } from "@/components/ui/button/Button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";

/**
 * A Game Master puts a character under the conditions its world's system
 * declares, or lifts them (spec 067 Story 4).
 *
 * The list is the system's, by its own labels: nothing here knows what any
 * condition is. Each box is one call, and the answer — every condition the
 * character is under afterwards — is what the boxes then show. The board is
 * not touched from here: the markers arrive the way any token change does.
 */
export function ConditionsDialog({
  name,
  actorId,
  declared,
  held: heldAtOpen,
  onClose,
  onBack,
}: {
  name: string;
  actorId: string;
  declared: WorldSystemCondition[];
  /** The ids the character was under when the menu opened. */
  held: string[];
  onClose: () => void;
  onBack: () => void;
}) {
  const [held, setHeld] = useState<ReadonlySet<string>>(
    () => new Set(heldAtOpen),
  );
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const idPrefix = useId();

  const toggle = (conditionId: string, on: boolean) => {
    setBusy(conditionId);
    setError(null);
    (on ? applyActorCondition : clearActorCondition)(actorId, conditionId)
      .then((now) => setHeld(new Set(now.map((condition) => condition.id))))
      .catch((err: unknown) => {
        setError(
          err instanceof Error ? err.message : "Changing the condition failed",
        );
      })
      .finally(() => setBusy(null));
  };

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent
        data-testid="canvas-menu-conditions-dialog"
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          onBack();
        }}
      >
        <DialogTitle>Conditions on {name}</DialogTitle>
        <DialogDescription>
          They stay with the character, on every token of theirs.
        </DialogDescription>
        <ul className="grid max-h-80 gap-2 overflow-y-auto">
          {declared.map((condition) => {
            const inputId = `${idPrefix}-${condition.id}`;
            return (
              <li key={condition.id} className="flex items-start gap-2">
                <Checkbox
                  id={inputId}
                  checked={held.has(condition.id)}
                  disabled={busy !== null}
                  onCheckedChange={(checked) =>
                    toggle(condition.id, checked === true)
                  }
                  data-testid={`canvas-menu-condition-${condition.id}`}
                />
                <div className="grid gap-0.5">
                  <Label htmlFor={inputId}>{condition.label}</Label>
                  {condition.description ? (
                    <p className="text-xs text-muted-foreground">
                      {condition.description}
                    </p>
                  ) : null}
                </div>
              </li>
            );
          })}
        </ul>
        {error ? (
          <p role="alert" className="text-xs text-destructive">
            {error}
          </p>
        ) : null}
        <div className="flex justify-end">
          <Button
            type="button"
            onClick={onClose}
            data-testid="canvas-menu-conditions-done"
          >
            Done
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
