/**
 * Spec 048: the actor's links to pieces the world does not hold yet, for the
 * sheet to mark "awaiting the GM". The store does not hold them, so this is
 * a plain fetch with `refetch()`.
 *
 * Given the world, the marks follow its events: an import, a rollback, or a
 * decision on a piece this actor carries (codes 40-42) reads the links
 * again, so "awaiting the GM" turns to the world's piece, or to "declined",
 * wherever the sheet is open — the play dock included, which does not
 * remount on those events as the actor page does.
 */
import { useCallback, useEffect, useState } from "react";
import { getActorStagedLinks, type ActorStagedLink } from "@/api/sheetImport";
import { subscribeToWorldEvents } from "@/engine/world/sync";
import { startSheetImportEventSync } from "@/engine/world/sync/sheetImport";

export interface UseActorStagedLinksResult {
  links: ActorStagedLink[];
  loading: boolean;
  refetch: () => Promise<void>;
}

export function useActorStagedLinks(
  actorId: string,
  worldId?: string,
): UseActorStagedLinksResult {
  const [links, setLinks] = useState<ActorStagedLink[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let active = true;
    getActorStagedLinks(actorId)
      .then((found) => {
        if (active) setLinks(found);
      })
      // A sheet without its marks is still a sheet; nothing to show.
      .catch(() => {
        if (active) setLinks([]);
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [actorId]);

  const refetch = useCallback(async () => {
    try {
      setLinks(await getActorStagedLinks(actorId));
    } catch {
      setLinks([]);
    }
  }, [actorId]);

  useEffect(() => {
    if (!worldId) return;
    return startSheetImportEventSync(
      {
        onActorSheetChanged: (changedActorId) => {
          if (changedActorId === actorId) void refetch();
        },
      },
      subscribeToWorldEvents(worldId, { announcePause: false }),
    );
  }, [worldId, actorId, refetch]);

  return { links, loading, refetch };
}
