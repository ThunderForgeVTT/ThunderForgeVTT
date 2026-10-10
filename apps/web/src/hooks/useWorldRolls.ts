/**
 * useWorldRolls.ts — spec 081: the table's rolls as this viewer may see them.
 *
 * Usage:
 *   const { rolls, loading, error, hasOlder, loadOlder, upsert, refetch } =
 *     useWorldRolls(worldId);
 *
 * The first page comes from `worldRolls`; after that the roll sync keeps it
 * live, adding a new roll or replacing a revealed one by id. Each entry is
 * whatever the server answered for this viewer — whole, masked, or absent —
 * so nothing here decides visibility.
 *
 * Spec 084: a reroll arriving asks again for the roll it replaced, so the
 * old one is struck through and loses its Reroll buttons on every screen.
 *
 * Spec 088: when the GM clears the rolls, every entry made at or before the
 * clear leaves the feed on every screen, and nothing that arrives later
 * (a page, a reveal, a fetch in flight) brings one back.
 *
 * `rolls` is oldest first, the order a feed reads in.
 */

import { useCallback, useEffect, useRef, useState } from "react";

import { fetchWorldRoll, fetchWorldRolls } from "@/api/roll";
import { startRollSync, subscribeToWorldEvents } from "@/engine/world/sync";
import { wasCleared } from "@/engine/world/sync/rolls";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import type { WorldRollEntry } from "@/types/roll";

/** The server's own default page, asked for by name so `hasOlder` is exact. */
export const ROLL_PAGE_SIZE = 50;

export interface UseWorldRollsResult {
  rolls: WorldRollEntry[];
  loading: boolean;
  error: Error | null;
  /** Whether the last page asked for came back full. */
  hasOlder: boolean;
  loadOlder: () => Promise<void>;
  /** Add an entry, or replace the one with its id. */
  upsert: (entry: WorldRollEntry) => void;
  refetch: () => Promise<void>;
}

/** `entries` with `entry` in place of the one sharing its id, or added. */
export function upsertRoll(
  entries: WorldRollEntry[],
  entry: WorldRollEntry,
): WorldRollEntry[] {
  const at = entries.findIndex((existing) => existing.id === entry.id);
  if (at === -1) {
    return sortRolls([...entries, entry]);
  }
  const next = entries.slice();
  next[at] = entry;
  return next;
}

/** Oldest first. A reveal keeps its roll's time, so it never moves. */
function sortRolls(entries: WorldRollEntry[]): WorldRollEntry[] {
  return entries
    .slice()
    .sort((a, b) => Date.parse(a.createdAt) - Date.parse(b.createdAt));
}

/** `entries` without those made at or before `clearedAt`. */
export function dropCleared(
  entries: WorldRollEntry[],
  clearedAt: string | null,
): WorldRollEntry[] {
  if (clearedAt === null) return entries;
  return entries.filter((entry) => !wasCleared(entry.createdAt, clearedAt));
}

/** A page from the server (newest first) merged into what is held. */
function mergePage(
  held: WorldRollEntry[],
  page: WorldRollEntry[],
): WorldRollEntry[] {
  return page.reduce(upsertRoll, held);
}

const asError = (err: unknown) =>
  err instanceof Error ? err : new Error(String(err));

export function useWorldRolls(worldId: string): UseWorldRollsResult {
  const [rolls, setRolls] = useState<WorldRollEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const [hasOlder, setHasOlder] = useState(false);
  const clearedAt = useRef<string | null>(null);
  /** Hold `next(held)`, less whatever the last clear took. */
  const hold = useCallback(
    (next: (held: WorldRollEntry[]) => WorldRollEntry[]) =>
      setRolls((held) => dropCleared(next(held), clearedAt.current)),
    [],
  );

  useResetOnChange(worldId, () => {
    clearedAt.current = null;
    setRolls([]);
    setLoading(true);
    setError(null);
    setHasOlder(false);
  });

  const upsert = useCallback(
    (entry: WorldRollEntry) => {
      hold((held) => upsertRoll(held, entry));
      if (entry.__typename === "WorldRoll" && entry.rerollOf !== null) {
        fetchWorldRoll(worldId, entry.rerollOf)
          .then((original) => {
            if (original) hold((held) => upsertRoll(held, original));
          })
          .catch(() => {
            // The original stays as it was; the next page fetch mends it.
          });
      }
    },
    [worldId, hold],
  );

  useEffect(() => {
    let active = true;
    fetchWorldRolls(worldId, { limit: ROLL_PAGE_SIZE })
      .then((page) => {
        if (!active) return;
        // Merged rather than set: a roll the sync delivered while this page
        // was in flight is kept.
        hold((held) => mergePage(held, page));
        setHasOlder(page.length === ROLL_PAGE_SIZE);
      })
      .catch((err) => {
        if (active) setError(asError(err));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    const stop = startRollSync({
      worldId,
      events: subscribeToWorldEvents(worldId),
      onRoll: (entry) => {
        if (active) upsert(entry);
      },
      onCleared: (at) => {
        if (!active) return;
        clearedAt.current = at;
        hold((held) => held);
        // Nothing older than a clear is left to page back to.
        setHasOlder(false);
      },
    });
    return () => {
      active = false;
      stop();
    };
  }, [worldId, upsert, hold]);

  const loadOlder = useCallback(async () => {
    const oldest = rolls[0];
    if (!oldest) return;
    try {
      const page = await fetchWorldRolls(worldId, {
        before: oldest.createdAt,
        limit: ROLL_PAGE_SIZE,
      });
      hold((held) => mergePage(held, page));
      setHasOlder(page.length === ROLL_PAGE_SIZE);
    } catch (err) {
      setError(asError(err));
    }
  }, [rolls, worldId, hold]);

  const refetch = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const page = await fetchWorldRolls(worldId, { limit: ROLL_PAGE_SIZE });
      hold(() => mergePage([], page));
      setHasOlder(page.length === ROLL_PAGE_SIZE);
    } catch (err) {
      setError(asError(err));
    } finally {
      setLoading(false);
    }
  }, [worldId, hold]);

  return { rolls, loading, error, hasOlder, loadOlder, upsert, refetch };
}
