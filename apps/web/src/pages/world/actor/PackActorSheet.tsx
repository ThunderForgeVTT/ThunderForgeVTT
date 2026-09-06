/**
 * An actor's sheet, drawn by the world's interface pack.
 *
 * # What this replaces
 *
 * `systemActorSheets.ts` held a `Record<string, ComponentType>` with one
 * entry — `genie` — and a note explaining that adding a system should not
 * require editing this app's core pages. It required exactly that: a system
 * without a hand-written React container had no sheet at all, and six of the
 * seven bundled systems did not have one.
 *
 * That registry is the shape spec 032 exists to retire (FR-029). Everything
 * needed to replace it was built and connected to nothing: the server
 * publishes what a system declares, `packs/interface/forge` says how to lay it
 * out, and `SheetLayout` renders one against the other. This is the join.
 *
 * # Why the layout comes from the appearance context
 *
 * Because it is the world's binding, resolved once by `AppearanceProvider`
 * with the base pack already overlaid and a missing pack already fallen back
 * to Forge (FR-018). A sheet that fetched its own pack would be a second
 * resolution of the same question, free to answer it differently.
 */
import { useEffect, useState } from "react";

import { fetchActorSheet } from "@/api/actorSheet";
import { useAppearance } from "@/appearance/appearance-context";
import { BASE_PACK_ID } from "@/appearance/appearance-context";
import { PackSurfaceBoundary } from "@/appearance/PackSurfaceBoundary";
import { SheetLayout } from "@/sheet-layout/SheetLayout";
import { declarationsFrom } from "@/sheet-layout/declarations";
import { rendersAnything, resolutionFrom } from "@/sheet-layout/resolve";
import type {
  LayoutDeclaration,
  SheetDeclarations,
} from "@/sheet-layout/types";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";

interface PackActorSheetProps {
  actorId: string;
  /** Shown when the actor's system publishes nothing at all. */
  emptyMessage?: string;
}

export function PackActorSheet({ actorId, emptyMessage }: PackActorSheetProps) {
  const appearance = useAppearance();
  /**
   * Keyed by the actor it describes, and written only from the fetch.
   *
   * Resetting to null at the top of the effect would be a synchronous
   * `setState` in an effect body — a cascading render, and the thing
   * `react-hooks/set-state-in-effect` refuses. Carrying the id instead makes
   * "still loading" a comparison rather than a write, and has the property
   * that matters more: a slow response for a previous actor can never paint
   * itself onto the actor now on screen.
   */
  const [loaded, setLoaded] = useState<{
    actorId: string;
    declarations: Partial<SheetDeclarations> | null;
  } | null>(null);

  useEffect(() => {
    let live = true;

    fetchActorSheet(actorId)
      .then((sheet) => {
        if (live) setLoaded({ actorId, declarations: sheet });
      })
      .catch(() => {
        if (live) setLoaded({ actorId, declarations: null });
      });

    return () => {
      live = false;
    };
  }, [actorId]);

  const current = loaded?.actorId === actorId ? loaded : null;
  const declarations = current?.declarations ?? null;
  const failed = current !== null && current.declarations === null;

  if (failed) {
    // Named plainly rather than rendered as an empty sheet. A sheet that
    // failed to load and a character with nothing on it look identical, and
    // one of them is a character whose numbers are gone.
    return (
      <StatusBadge variant="danger">
        This character's sheet could not be loaded.
      </StatusBadge>
    );
  }

  if (current === null || declarations === null) {
    return <Loader label="Reading the character sheet" />;
  }

  // The base pack's layout while the world's is still resolving. Forge is
  // generic, so it renders any system — showing the sheet in it for a moment
  // is better than showing nothing and then the sheet.
  const layout = (appearance?.layout ?? []) as LayoutDeclaration;

  // Everything the pack drives is inside the boundary, deliberately.
  //
  // It was not, and the gap was real: `rendersAnything` walks the pack's
  // layout to decide whether to draw at all, and a malformed node throws
  // there — one call *above* where the boundary used to start. A boundary
  // that wraps only the render and not the decision to render contains the
  // second half of a surface and lets the first half take the page.
  //
  // The pack named is the one in force, which is the base pack when the
  // world's own could not be applied: naming the absent pack would send a
  // Game Master to look at something that never ran.
  return (
    <PackSurfaceBoundary
      packId={appearance?.packId ?? BASE_PACK_ID}
      surface="character sheet"
    >
      <PackSheet
        layout={layout}
        declarations={declarations}
        emptyMessage={emptyMessage}
      />
    </PackSurfaceBoundary>
  );
}

interface PackSheetProps {
  layout: LayoutDeclaration;
  declarations: Partial<SheetDeclarations>;
  emptyMessage?: string;
}

/**
 * The pack-driven half: what this pack lays out for this character.
 *
 * Split from `PackActorSheet` so that a boundary can sit between them. Fetch
 * failures and loading belong to the component that fetches; everything that
 * reads a pack's layout belongs here, where a failure is contained.
 */
function PackSheet({ layout, declarations, emptyMessage }: PackSheetProps) {
  // Whether this pack draws anything at all for this character.
  //
  // `SheetLayout` returns null when no node in the layout has anything to
  // place, and a null sheet is silently indistinguishable from a sheet that
  // failed — which is exactly what happened: a targeted pack whose layout is
  // specific value nodes (Forged Steel is `pair` and `value` throughout) draws
  // nothing for a character nobody has filled in, and the page showed blank
  // space with no explanation. Asked through the same exported predicate the
  // renderer uses, rather than by guessing from the declaration count.
  const resolved = declarationsFrom(declarations);
  const at = resolutionFrom(resolved);
  const draws = layout.some((node) => rendersAnything(node, at));

  if (!draws) {
    // Two different truths, said differently. A system that publishes nothing
    // is spec 031's "a game system that defines no character sheet"; a system
    // that publishes plenty while the pack lays out none of it is a blank
    // character, and telling someone their system has no sheet would be wrong.
    const publishesNothing = (declarations.all ?? []).length === 0;
    return (
      <p className="text-sm text-muted-foreground" data-slot="sheet-empty">
        {emptyMessage ??
          (publishesNothing
            ? "This character's game system publishes nothing to show on a sheet."
            : "Nothing to show yet — this character has none of the values its sheet lays out.")}
      </p>
    );
  }

  return <SheetLayout layout={layout} declarations={declarations} />;
}
