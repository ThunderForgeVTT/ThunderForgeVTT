import { useEffect, useState, type ReactNode } from "react";
import { getWorldActors } from "@/api/actors";
import type { WorldActorRecord } from "@/types/actor";
import { InPaneCharacterSheet } from "@/components/world/PlayDock/InPaneCharacterSheet";
import { lookAtToken } from "@/engine/lookAt";
import type { WorldStore } from "@/engine/world/store";
import type { WorldToken } from "@/engine/world/types";

export interface PlayerSeatInput {
  worldId: string;
  worldStore: WorldStore;
  sceneId: string | null;
  /** The character this viewer plays (`useScenePlay`); `null` for none. */
  claimedActorId: string | null;
  tokens: Record<string, WorldToken>;
  selectOne: (tokenId: string) => void;
}

const note = (text: string) => (
  <p className="text-sm text-muted-foreground" data-testid="phone-me-note">
    {text}
  </p>
);

/**
 * Spec 102 R7: a player's Me and Sheet. Me finds their character's token on
 * this scene, selects it and puts it on screen; Sheet is that character.
 */
export function usePlayerSeat({
  worldId,
  worldStore,
  sceneId,
  claimedActorId,
  tokens,
  selectOne,
}: PlayerSeatInput) {
  const [actor, setActor] = useState<WorldActorRecord | null>(null);

  useEffect(() => {
    setActor(null);
    if (!claimedActorId) return;
    let active = true;
    getWorldActors(worldId)
      .then((all) => {
        if (active) setActor(all.find((a) => a.id === claimedActorId) ?? null);
      })
      // No sheet is the same as no character here; the Actors panel says why.
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [worldId, claimedActorId]);

  const go = (tokenId: string) => {
    selectOne(tokenId);
    lookAtToken(tokenId);
  };

  const onMe = (close: () => void): ReactNode | null => {
    if (!claimedActorId)
      return note(
        "You haven't picked a character yet. Choose one under More, then Actors.",
      );
    const mine = Object.values(tokens).filter(
      (t) =>
        t.actorId === claimedActorId && (!sceneId || t.sceneId === sceneId),
    );
    if (mine.length === 0) return note("Your character isn't on this scene.");
    if (mine.length === 1) {
      go(mine[0].id);
      return null;
    }
    return (
      <div className="flex flex-col gap-1" data-testid="phone-me-chooser">
        {mine.map((t) => (
          <button
            key={t.id}
            type="button"
            onClick={() => {
              go(t.id);
              close();
            }}
            className="flex min-h-12 items-center rounded-xl px-3 text-left text-sm active:bg-muted"
          >
            {t.label ?? "Your token"}
          </button>
        ))}
      </div>
    );
  };

  const characterSheet = (): ReactNode =>
    actor ? (
      <InPaneCharacterSheet
        worldId={worldId}
        worldStore={worldStore}
        sceneId={sceneId}
        actor={actor}
      />
    ) : (
      note(
        claimedActorId
          ? "Loading your character…"
          : "You haven't picked a character yet. Choose one under More, then Actors.",
      )
    );

  return { onMe, characterSheet };
}
