import { useState, useEffect } from "react";
import { updateWorldAutoApplyNpcDamage } from "@/api/attacks";
import { Link } from "react-router-dom";
import {
  getWorld,
  updateWorldAllowPlayerActorArt,
  updateWorldAllowPlayerCreatedActors,
} from "@/api/world";
import { Card } from "@/components/ui/card/Card";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { IN_DEMO } from "@/lib/demoBuild";

interface CampaignSettingsPanelProps {
  worldId: string;
}

/**
 * CampaignSettingsPanel holds the world-level campaign settings.
 *
 * Spec 023 (FR-011): the player roster and its controls live on the Players
 * page (`PlayersPage.tsx`). Spec 088 (FR-001): so do the world's links
 * (`WorldLinksPanel.tsx`); this panel only points there.
 */
export function CampaignSettingsPanel({ worldId }: CampaignSettingsPanelProps) {
  const [error, setError] = useState<string | null>(null);
  const [allowPlayerCreatedActors, setAllowPlayerCreatedActors] =
    useState(false);
  const [isUpdatingAllowSetting, setIsUpdatingAllowSetting] = useState(false);
  const [autoApplyNpcDamage, setAutoApplyNpcDamage] = useState(false);
  const [isUpdatingAutoApply, setIsUpdatingAutoApply] = useState(false);
  const [allowPlayerActorArt, setAllowPlayerActorArt] = useState(true);
  const [isUpdatingActorArt, setIsUpdatingActorArt] = useState(false);

  useEffect(() => {
    void getWorld(worldId).then((world) => {
      if (world) {
        setAllowPlayerCreatedActors(world.allowPlayerCreatedActors);
        setAutoApplyNpcDamage(world.autoApplyNpcDamage);
        setAllowPlayerActorArt(world.allowPlayerActorArt);
      }
    });
  }, [worldId]);

  const handleToggleAllowPlayerCreatedActors = async (allow: boolean) => {
    setIsUpdatingAllowSetting(true);
    setError(null);
    try {
      const updated = await updateWorldAllowPlayerCreatedActors(worldId, allow);
      setAllowPlayerCreatedActors(updated.allowPlayerCreatedActors);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to update setting");
    } finally {
      setIsUpdatingAllowSetting(false);
    }
  };

  /** Spec 046 FR-006: the world's default; an encounter may override it. */
  const handleToggleAutoApply = async (enabled: boolean) => {
    setIsUpdatingAutoApply(true);
    setError(null);
    try {
      setAutoApplyNpcDamage(
        await updateWorldAutoApplyNpcDamage(worldId, enabled),
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to update setting");
    } finally {
      setIsUpdatingAutoApply(false);
    }
  };

  /** Spec 044 FR-030a: on by default; off withdraws every player's grant. */
  const handleToggleActorArt = async (allow: boolean) => {
    setIsUpdatingActorArt(true);
    setError(null);
    try {
      const updated = await updateWorldAllowPlayerActorArt(worldId, allow);
      setAllowPlayerActorArt(updated.allowPlayerActorArt);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to update setting");
    } finally {
      setIsUpdatingActorArt(false);
    }
  };

  const displayError = error;

  return (
    <section>
      <Card surface="parchment" className="grid gap-6 p-6">
        <div>
          <h2 className="text-xl font-semibold">Campaign Settings</h2>
          <p className="text-muted-foreground">
            Player-created characters, character art and NPC damage
          </p>
        </div>

        {displayError && (
          <StatusBadge variant="danger">{displayError}</StatusBadge>
        )}

        {/* Spec 088 (FR-001): links are made and revoked on the Players
            page. Not in the demo, which has no one to invite (spec 074). */}
        {IN_DEMO ? null : (
          <p className="text-sm text-muted-foreground">
            To invite players, make a link on the{" "}
            <Link
              to={`/world/${worldId}/players`}
              className="underline underline-offset-2"
              data-testid="campaign-settings-players-link"
            >
              Players page
            </Link>
            .
          </p>
        )}

        {/* Spec 017 (FR-007): player-created character setting */}
        <div className="grid gap-3">
          <h3 className="font-semibold">Player-created characters</h3>
          <p className="text-sm text-muted-foreground">
            When on, a joining player without a GM-designated character can
            create their own on the Actor Selection screen. Off by default.
          </p>
          <label
            className="flex items-center gap-2 text-sm"
            data-testid="allow-player-created-actors-toggle"
          >
            <input
              type="checkbox"
              checked={allowPlayerCreatedActors}
              disabled={isUpdatingAllowSetting}
              onChange={(e) =>
                void handleToggleAllowPlayerCreatedActors(e.target.checked)
              }
            />
            Allow players to create their own actors
          </label>
        </div>

        {/* Spec 044 FR-030a: players and their own character's art */}
        <div className="grid gap-3">
          <h3 className="font-semibold">Players&apos; character art</h3>
          <p className="text-sm text-muted-foreground">
            When on, the player who holds a character may change its portrait
            and token, and build a look for it. Turning this off stops every
            player in this world from doing so; art already set stays, and you
            may still change any character&apos;s art. To stop one player only,
            lock that character&apos;s look on its page. On by default.
          </p>
          <label
            className="flex items-center gap-2 text-sm"
            data-testid="allow-player-actor-art-toggle"
          >
            <input
              type="checkbox"
              checked={allowPlayerActorArt}
              disabled={isUpdatingActorArt}
              onChange={(e) => void handleToggleActorArt(e.target.checked)}
            />
            Players may change their character&apos;s art
          </label>
        </div>

        {/* Spec 046 FR-006: auto-apply for the Game Master's own NPCs */}
        <div className="grid gap-3">
          <h3 className="font-semibold">Damage to your NPCs</h3>
          <p className="text-sm text-muted-foreground">
            When on, a hit on a creature no player controls is applied at once
            instead of waiting for you to take it. Damage to a player&apos;s
            character is always theirs to take. An encounter can override this
            from the combat tracker. Off by default.
          </p>
          <label
            className="flex items-center gap-2 text-sm"
            data-testid="auto-apply-npc-damage-toggle"
          >
            <input
              type="checkbox"
              checked={autoApplyNpcDamage}
              disabled={isUpdatingAutoApply}
              onChange={(e) => void handleToggleAutoApply(e.target.checked)}
            />
            Apply damage to my NPCs automatically
          </label>
        </div>
      </Card>
    </section>
  );
}
