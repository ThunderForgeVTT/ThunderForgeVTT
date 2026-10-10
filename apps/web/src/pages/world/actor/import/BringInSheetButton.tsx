/**
 * Spec 048: the way in to bringing a character in from a sheet. Shown only
 * where it can work: the person may edit the actor, the server has the
 * feature on, and the actor's system declares a `sheetImport` mapping
 * (FR-012). The server checks all three again.
 */
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { getGameSystemManifest } from "@/api/gameSystems";
import { Button } from "@/components/ui/button/Button";
import { useFeatureFlag } from "@/hooks/useFeatureFlag";
import { mayEditActor } from "@/pages/world/actor/actorEditRight";
import type { WorldActorRecord } from "@/types/actor";

/** Whether a system declares a sheet mapping; manifests are read once. */
const declared = new Map<string, Promise<boolean>>();

export function systemReadsSheets(gameSystemId: string): Promise<boolean> {
  let pending = declared.get(gameSystemId);
  if (!pending) {
    pending = getGameSystemManifest(gameSystemId).then(
      (manifest) =>
        typeof manifest.sheetImport === "object" &&
        manifest.sheetImport !== null,
    );
    pending.catch(() => declared.delete(gameSystemId));
    declared.set(gameSystemId, pending);
  }
  return pending;
}

export function useSystemReadsSheets(
  gameSystemId: string | null | undefined,
): boolean {
  const [answer, setAnswer] = useState<{ id: string; reads: boolean } | null>(
    null,
  );
  useEffect(() => {
    if (!gameSystemId) return;
    let active = true;
    systemReadsSheets(gameSystemId)
      .then((reads) => {
        if (active) setAnswer({ id: gameSystemId, reads });
      })
      .catch(() => {
        if (active) setAnswer({ id: gameSystemId, reads: false });
      });
    return () => {
      active = false;
    };
  }, [gameSystemId]);
  return !!gameSystemId && answer?.id === gameSystemId && answer.reads;
}

export function BringInSheetButton({
  worldId,
  actor,
  testId = "actor-bring-in-sheet",
  size = "md",
}: {
  worldId: string;
  actor: Pick<WorldActorRecord, "id" | "gameSystemId" | "myPermissionLevel">;
  testId?: string;
  size?: "sm" | "md";
}) {
  const navigate = useNavigate();
  const flagOn = useFeatureFlag("feature.sheet_import");
  const reads = useSystemReadsSheets(flagOn ? actor.gameSystemId : null);
  if (!flagOn || !reads || !mayEditActor(actor)) return null;
  return (
    <Button
      variant="secondary"
      size={size}
      onClick={() => navigate(`/world/${worldId}/actor/${actor.id}/import`)}
      data-testid={testId}
    >
      Bring in a sheet
    </Button>
  );
}
