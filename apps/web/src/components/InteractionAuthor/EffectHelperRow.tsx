import type { ComponentType } from "react";
import {
  BookOpen,
  CircleDot,
  DoorOpen,
  Lightbulb,
  Package,
  Signpost,
  Sparkles,
} from "lucide-react";
import { Button } from "@/components/ui/button/Button";
import type { EffectHelper } from "./effectHelpers";

/**
 * One button per thing this build can actually make happen (FR-028).
 *
 * # Why buttons as well as the dropdown, rather than instead of it
 *
 * The dropdown is correct and unusable for the person it is for. A Game
 * Master authoring their first interactive has to know that "what happens" is
 * a list worth opening, then read six labels written in the vocabulary of the
 * subsystems that contributed them. The playtest note is blunt about it: a
 * placed lore page should be recognisable as a book you can open, not as a
 * row in a select.
 *
 * So the kinds this build supports are shown *at rest*, with their icons, and
 * the dropdown stays as the thing they both drive. Removing the dropdown was
 * the tempting simplification and is wrong twice: it is the only control that
 * survives a build contributing more effects than fit in a rail panel, and it
 * is what `interactive-authoring-ui.spec.ts` reaches for when it proves the
 * form is registry-driven.
 *
 * # Why the icon is keyed by namespace and has a fallback
 *
 * Exactly the reasoning `interaction_marker.rs` gives for colouring badges by
 * namespace: `door.set_state` and `door.set_lock` are both doors to the
 * person looking at them, and a subsystem contributing a second effect should
 * not need a new decision made about it here. A namespace this build has no
 * icon for still gets a button — a generic glyph, never a missing control,
 * because a helper that silently disappears is indistinguishable from an
 * effect that does not exist.
 */

type Glyph = ComponentType<{ className?: string; "aria-hidden"?: boolean }>;

const NAMESPACE_GLYPH: Record<string, Glyph> = {
  lore: BookOpen,
  item: Package,
  door: DoorOpen,
  light: Lightbulb,
  nav: Signpost,
};

/** What a namespace nobody drew an icon for looks like. */
const UNKNOWN_GLYPH: Glyph = Sparkles;

export interface EffectHelperRowProps {
  helpers: EffectHelper[];
  /** The chosen effect, or `null` for scenery. */
  selectedId: string | null;
  onChoose: (effectId: string | null) => void;
}

export function EffectHelperRow({
  helpers,
  selectedId,
  onChoose,
}: EffectHelperRowProps) {
  if (helpers.length === 0) {
    // Nothing to offer is said by the panel itself, in one place. A second
    // empty state here would be two messages about one situation.
    return null;
  }

  return (
    <div
      className="flex flex-wrap gap-2"
      data-testid="interaction-helpers"
      role="group"
      aria-label="What this does"
    >
      {helpers.map((helper) => {
        const Glyph = NAMESPACE_GLYPH[helper.namespace] ?? UNKNOWN_GLYPH;
        const chosen = helper.id === selectedId;
        return (
          <Button
            key={helper.id}
            type="button"
            size="sm"
            variant={chosen ? "primary" : "secondary"}
            aria-pressed={chosen}
            title={helper.description}
            onClick={() => onChoose(helper.id)}
            data-testid={`interaction-helper-${helper.id}`}
          >
            <Glyph className="size-4" aria-hidden />
            {helper.label}
          </Button>
        );
      })}

      {/* Last, not first: the dropdown puts scenery first because it is the
          commonest thing a GM places, but a row is read left to right and
          leading with "nothing" buries every option this row exists to
          show. It is here so a choice can be taken back without the GM
          having to work out that the dropdown also does that. */}
      <Button
        type="button"
        size="sm"
        variant={selectedId === null ? "primary" : "ghost"}
        aria-pressed={selectedId === null}
        title="Scenery: it responds to nothing."
        onClick={() => onChoose(null)}
        data-testid="interaction-helper-none"
      >
        <CircleDot className="size-4" aria-hidden />
        Nothing
      </Button>
    </div>
  );
}
