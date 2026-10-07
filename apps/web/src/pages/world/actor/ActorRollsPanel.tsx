import { CharacterRollButtons } from "@/components/world/PlayDock/CharacterRollButtons";
import { useCharacterRolls } from "@/components/world/PlayDock/useCharacterRolls";
import type { WorldActorRecord } from "@/types/actor";

export interface ActorRollsPanelProps {
  worldId: string;
  actor: WorldActorRecord;
  isGm: boolean;
}

/**
 * Spec 081 US2: the same rolls the dock's sheet offers, on the sheet page.
 *
 * A player with this page open in a second tab rolls here and the play view
 * animates it, because a roll reaches every board as a world event; nothing
 * passes between the tabs. An attack needs a target picked on the board, so
 * an attack entry here says so instead of rolling (FR-014).
 */
export function ActorRollsPanel({
  worldId,
  actor,
  isGm,
}: ActorRollsPanelProps) {
  const rolls = useCharacterRolls(worldId, actor);
  if (rolls.length === 0) return null;
  return (
    <section className="grid gap-2" data-testid="sheet-rolls">
      <h3 className="font-semibold">Rolls</h3>
      <CharacterRollButtons
        worldId={worldId}
        rolls={rolls}
        isGm={isGm}
        testIdPrefix="sheet"
      />
    </section>
  );
}
