import { useEffect, useId, useRef, useState } from "react";
import { postGraphQL } from "@/api/graphqlClient";
import { Button } from "@/components/ui/button/Button";

/** One ability a multiattack may name: the world's abilities, by name. */
export interface MultiattackChoice {
  id: string;
  name: string;
}

/**
 * The world's abilities, for the picker. Only the two fields it shows: the
 * compendium's own read carries effects, permissions and lore links for every
 * ability, none of which a list of names needs. GM-only abilities are
 * withheld for anyone who may not see them, by the server.
 */
function loadChoices(worldId: string): Promise<MultiattackChoice[]> {
  return postGraphQL<{ worldAbilities: MultiattackChoice[] }>(
    `
      query MultiattackChoices($worldId: UUID!) {
        worldAbilities(worldId: $worldId) {
          id
          name
        }
      }
    `,
    { worldId },
  ).then((data) => data.worldAbilities);
}

export interface MultiattackPickerProps {
  worldId: string;
  /** The parts, in the order one use makes them. */
  value: string[];
  onChange: (next: string[]) => void;
  disabled: boolean;
  /** The ability being edited: a multiattack never names itself. */
  selfId?: string;
}

/** What a button that just moved or removed a part should hand focus to. */
type FocusTarget =
  | { kind: "up" | "down" | "remove"; index: number }
  | { kind: "add" };

/**
 * A multiattack's parts, picked from the world's abilities and put in order
 * (spec 046 FR-044; replaces the ability-id text box T061 left).
 *
 * A native `<select>` to choose and plain buttons to add, move and remove, so
 * every step works from the keyboard with nothing to learn, and every button
 * says which part it acts on ("Move Greatclub, part 2, up"). The same ability
 * may be named twice: an ogre that swings its greatclub twice has two parts.
 *
 * Focus stays with the part it acted on: moving a part keeps its button under
 * focus in the part's new place, and removing one moves focus to the part
 * that took its place, or to the add control when the list is empty. Each
 * change is also said in words through a status region.
 */
export function MultiattackPicker({
  worldId,
  value,
  onChange,
  disabled,
  selfId,
}: MultiattackPickerProps) {
  const [choices, setChoices] = useState<MultiattackChoice[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [adding, setAdding] = useState("");
  const [announcement, setAnnouncement] = useState("");
  const pendingFocus = useRef<FocusTarget | null>(null);
  const listRef = useRef<HTMLOListElement>(null);
  const addRef = useRef<HTMLSelectElement>(null);
  const baseId = useId();

  useEffect(() => {
    let active = true;
    loadChoices(worldId)
      .then((loaded) => {
        if (active) setChoices(loaded);
      })
      .catch(() => {
        if (active) setLoadError("Could not load this world's abilities");
      });
    return () => {
      active = false;
    };
  }, [worldId]);

  useEffect(() => {
    const target = pendingFocus.current;
    if (!target) return;
    pendingFocus.current = null;
    if (target.kind === "add") {
      addRef.current?.focus();
      return;
    }
    const button = listRef.current?.querySelector<HTMLButtonElement>(
      `[data-part-index="${target.index}"][data-part-action="${target.kind}"]`,
    );
    // A move to the top or bottom disables the button that did it; its
    // sibling in the same row is the next useful place for focus.
    if (button && !button.disabled) {
      button.focus();
    } else {
      listRef.current
        ?.querySelector<HTMLButtonElement>(
          `[data-part-index="${target.index}"][data-part-action="remove"]`,
        )
        ?.focus();
    }
  }, [value]);

  const nameOf = (id: string) =>
    choices?.find((choice) => choice.id === id)?.name ??
    "An ability you cannot see";

  const selectable = (choices ?? []).filter((choice) => choice.id !== selfId);

  const add = () => {
    if (!adding) return;
    onChange([...value, adding]);
    setAnnouncement(
      `Added ${nameOf(adding)} as part ${value.length + 1} of ${value.length + 1}`,
    );
  };

  const move = (index: number, by: -1 | 1) => {
    const to = index + by;
    if (to < 0 || to >= value.length) return;
    const next = [...value];
    [next[index], next[to]] = [next[to], next[index]];
    pendingFocus.current = { kind: by < 0 ? "up" : "down", index: to };
    onChange(next);
    setAnnouncement(`Moved ${nameOf(value[index])} to part ${to + 1}`);
  };

  const remove = (index: number) => {
    const next = value.filter((_, i) => i !== index);
    pendingFocus.current =
      next.length === 0
        ? { kind: "add" }
        : { kind: "remove", index: Math.min(index, next.length - 1) };
    onChange(next);
    setAnnouncement(`Removed ${nameOf(value[index])}`);
  };

  const legendId = `${baseId}-legend`;
  const addId = `${baseId}-add`;

  return (
    <fieldset
      className="grid gap-2 rounded border border-border p-3"
      data-testid="attack-fields-multiattack"
      aria-describedby={`${baseId}-hint`}
    >
      <legend id={legendId} className="px-1 text-sm font-medium">
        Multiattack
      </legend>
      <p id={`${baseId}-hint`} className="text-sm text-muted-foreground">
        The attacks one use makes, in order. Leave it empty for a single attack.
      </p>

      {value.length === 0 ? (
        <p className="text-sm text-muted-foreground">No parts.</p>
      ) : (
        <ol
          ref={listRef}
          aria-labelledby={legendId}
          className="grid gap-1"
          data-testid="multiattack-parts"
        >
          {value.map((id, index) => {
            const name = nameOf(id);
            const part = `${name}, part ${index + 1}`;
            return (
              <li
                // Parts may repeat an ability, so the index is part of the key.
                key={`${id}-${index}`}
                className="flex items-center gap-2 rounded border border-border px-2 py-1 text-sm"
                data-testid="multiattack-part"
                data-ability-id={id}
              >
                <span className="w-6 text-right tabular-nums text-muted-foreground">
                  {index + 1}.
                </span>
                <span className="min-w-0 flex-1 truncate">{name}</span>
                {disabled ? null : (
                  <>
                    <Button
                      type="button"
                      size="sm"
                      variant="secondary"
                      aria-label={`Move ${part}, up`}
                      disabled={index === 0}
                      data-part-index={index}
                      data-part-action="up"
                      onClick={() => move(index, -1)}
                    >
                      Up
                    </Button>
                    <Button
                      type="button"
                      size="sm"
                      variant="secondary"
                      aria-label={`Move ${part}, down`}
                      disabled={index === value.length - 1}
                      data-part-index={index}
                      data-part-action="down"
                      onClick={() => move(index, 1)}
                    >
                      Down
                    </Button>
                    <Button
                      type="button"
                      size="sm"
                      variant="secondary"
                      aria-label={`Remove ${part}`}
                      data-part-index={index}
                      data-part-action="remove"
                      onClick={() => remove(index)}
                    >
                      Remove
                    </Button>
                  </>
                )}
              </li>
            );
          })}
        </ol>
      )}

      {disabled ? null : (
        <div className="flex flex-wrap items-end gap-2">
          <label htmlFor={addId} className="grid gap-1 text-sm">
            Add an attack
            <select
              id={addId}
              ref={addRef}
              value={adding}
              onChange={(e) => setAdding(e.target.value)}
              disabled={choices === null}
              data-testid="multiattack-add-choice"
              className="rounded border border-border bg-background px-2 py-1"
            >
              <option value="">
                {choices === null ? "Loading abilities…" : "Choose an ability"}
              </option>
              {selectable.map((choice) => (
                <option key={choice.id} value={choice.id}>
                  {choice.name}
                </option>
              ))}
            </select>
          </label>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            disabled={!adding}
            onClick={add}
            data-testid="multiattack-add"
          >
            Add part
          </Button>
        </div>
      )}

      {loadError ? (
        <p className="text-sm text-destructive" role="alert">
          {loadError}
        </p>
      ) : null}
      <p className="sr-only" role="status" aria-live="polite">
        {announcement}
      </p>
    </fieldset>
  );
}
