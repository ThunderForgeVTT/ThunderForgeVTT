import { Suspense, useState } from "react";
import { Link } from "react-router-dom";
import { getWorldActorImages } from "@/api/actors";
import { Button } from "@/components/ui/button/Button";
import { BringInSheetButton } from "@/pages/world/actor/import/BringInSheetButton";
import { LazyHeroBuilderDialog } from "@/pages/world/actor/heroBuilderLazy";
import { heroControlsFor } from "@/pages/world/players/heroControls";
import type { WorldActorRecord } from "@/types/actor";

export interface PlayerHeroControlsProps {
  worldId: string;
  /** The card's claimed character, as the server answered for the viewer. */
  actor: WorldActorRecord | undefined;
}

/**
 * Hotfix (owner, 2026-10-08): a player's way from the Players screen to their
 * own hero's sheet and builder.
 *
 * A player who had claimed a character held Editor on it, but this screen
 * only linked its name to the read-only page, so nothing told them the sheet
 * or the builder was theirs to use. These are the same two editors the Game
 * Master uses (the actor's edit page and `HeroBuilderDialog`). Each card asks
 * the server's answer for the person looking (see `heroControlsFor`): the
 * holder sees their hero's controls, the Game Master sees every card's, and
 * another player sees none.
 */
export function PlayerHeroControls({
  worldId,
  actor,
}: PlayerHeroControlsProps) {
  const [existingRoles, setExistingRoles] = useState<readonly string[] | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const controls = heroControlsFor(actor);

  if (!actor || (!controls.sheet && !controls.builder)) {
    return null;
  }

  // The builder asks before replacing art the hero already has, so it needs
  // to know which roles are filled; asked for when opened, not for every card.
  const openBuilder = async () => {
    setError(null);
    try {
      const images = await getWorldActorImages(worldId);
      setExistingRoles((images[actor.id] ?? []).map((image) => image.role));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  // Spec 088 FR-060: on a phone the two controls are full width and at
  // least 44 px tall, the size a thumb can hit; from `sm` they are the small
  // buttons they were.
  const control = "min-h-11 w-full sm:min-h-0 sm:w-auto";
  return (
    <div className="grid gap-1">
      <div className="grid gap-2 sm:flex sm:flex-wrap">
        {controls.sheet ? (
          <Button asChild variant="secondary" size="sm" className={control}>
            {/* FR-062: the sheet's back control returns here. */}
            <Link
              to={`/world/${worldId}/actor/${actor.id}/edit?from=players`}
              data-testid={`player-hero-sheet-${actor.id}`}
            >
              Open sheet
            </Link>
          </Button>
        ) : null}
        {controls.builder ? (
          <Button
            variant="secondary"
            size="sm"
            className={control}
            onClick={() => void openBuilder()}
            data-testid={`player-hero-build-${actor.id}`}
          >
            Edit look
          </Button>
        ) : null}
        {controls.sheet ? (
          <BringInSheetButton
            worldId={worldId}
            actor={actor}
            size="sm"
            testId={`player-hero-import-${actor.id}`}
          />
        ) : null}
      </div>
      {error ? (
        <p role="status" className="text-sm text-destructive">
          {error}
        </p>
      ) : null}
      {existingRoles ? (
        <Suspense fallback={null}>
          <LazyHeroBuilderDialog
            open
            worldId={worldId}
            onOpenChange={(open) => {
              if (!open) {
                setExistingRoles(null);
              }
            }}
            actorId={actor.id}
            actorLabel={actor.label}
            existingRoles={existingRoles}
          />
        </Suspense>
      ) : null}
    </div>
  );
}
