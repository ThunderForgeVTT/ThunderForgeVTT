/**
 * Spec 048: the actor's links to pieces the world does not hold yet, for the
 * sheet to mark "awaiting the GM". The store does not hold them, so this is
 * a plain fetch with `refetch()`; the sheet remounts when an import lands.
 */
import { useCallback, useEffect, useState } from "react";
import { getActorStagedLinks, type ActorStagedLink } from "@/api/sheetImport";

export interface UseActorStagedLinksResult {
  links: ActorStagedLink[];
  loading: boolean;
  refetch: () => Promise<void>;
}

export function useActorStagedLinks(
  actorId: string,
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

  return { links, loading, refetch };
}
