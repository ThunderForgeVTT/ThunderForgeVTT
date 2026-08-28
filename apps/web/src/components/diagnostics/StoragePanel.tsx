import { useCallback, useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { useAuth } from "@/hooks/useAuth";
import { getMyWorlds } from "@/api/world";
import {
  clearAllCache,
  clearWorldCache,
  formatBytes,
  readCacheUsage,
  type CacheUsage,
} from "@/services/worldCacheStorage";

/**
 * What ThunderForge is storing on this machine, and how to take it back
 * (spec 028, US5, FR-025/FR-026, T065/T066).
 *
 * # What the numbers are
 *
 * Bytes on disk in OPFS, per world, read directly rather than asked of the
 * engine — a settings screen has not mounted a canvas and should not download
 * a wasm module to print a number (see `worldCacheStorage.ts`). They are
 * ciphertext sizes, so they are what the disk is actually giving up, and they
 * run slightly above the plaintext figures the budget accounts in.
 *
 * # Clearing is safe, and the panel says so
 *
 * Nothing here touches the server. A cleared world is exactly a world that
 * has not been visited yet: it loads on the next visit, a little slower, and
 * nothing about the account or the campaign changes. That is worth stating in
 * the UI rather than only in a docstring, because "clear" next to a number is
 * a word people reasonably hesitate over — and hesitation is what leaves
 * people stuck with a full disk and a feature they were afraid to use.
 *
 * # Worlds that are cached but not listed
 *
 * The breakdown is keyed by what is on disk, and world *names* come from the
 * account's world list. Those can disagree: a world the user has left, or one
 * deleted server-side, leaves bytes behind and appears here with no name.
 * Showing it as its bare id is deliberate — it is precisely the content a
 * storage screen exists to let someone reclaim, and hiding rows without names
 * would make the rows fail to add up to the total.
 */
export function StoragePanel() {
  const { user } = useAuth();
  const userId = user?.id ?? null;

  const [usage, setUsage] = useState<CacheUsage | null>(null);
  const [names, setNames] = useState<Map<string, string>>(new Map());
  const [busy, setBusy] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  /**
   * Bumped to ask for a re-read, rather than calling a `refresh()` that sets
   * state from an effect body.
   *
   * The read is genuinely a subscription to something outside React — a
   * filesystem that other tabs, the eviction pass and the sync are all
   * writing to — so the effect below owns it and the handlers only signal
   * that the answer has changed. It also gives cancellation somewhere to
   * live: a clear followed by a quick unmount would otherwise resolve into a
   * component that is gone.
   */
  const [reloadToken, setReloadToken] = useState(0);

  useEffect(() => {
    if (!userId) return;
    let cancelled = false;
    void readCacheUsage(userId).then((next) => {
      if (!cancelled) setUsage(next);
    });
    return () => {
      cancelled = true;
    };
  }, [userId, reloadToken]);

  useEffect(() => {
    // Names are a nicety; the panel is fully usable without them, so a failed
    // world list must not empty the breakdown or surface an error over a
    // storage figure that is perfectly readable.
    void getMyWorlds()
      .then((worlds) => setNames(new Map(worlds.map((w) => [w.id, w.name]))))
      .catch(() => setNames(new Map()));
  }, []);

  const clearOne = useCallback(
    async (worldId: string) => {
      if (!userId) return;
      setBusy(worldId);
      const outcome = await clearWorldCache(userId, worldId);
      setReloadToken((token) => token + 1);
      setBusy(null);
      setNote(
        outcome.ok
          ? `Cleared ${formatBytes(outcome.freedBytes)}. That world will load from the server next time.`
          : "Some of that world's files could not be removed. Nothing was lost — try again.",
      );
    },
    [userId],
  );

  const clearEverything = useCallback(async () => {
    if (!userId) return;
    setBusy("all");
    const outcome = await clearAllCache(userId);
    setReloadToken((token) => token + 1);
    setBusy(null);
    setNote(
      outcome.ok
        ? `Cleared ${formatBytes(outcome.freedBytes)}. Your worlds are untouched on the server.`
        : "Some files could not be removed. Nothing was lost — try again.",
    );
  }, [userId]);

  const rows = useMemo(
    () =>
      (usage?.worlds ?? []).map((world) => ({
        ...world,
        name: names.get(world.worldId) ?? null,
      })),
    [usage, names],
  );

  if (!userId) return null;

  if (usage?.unavailable) {
    return (
      <section className="grid gap-2" data-testid="storage-panel">
        <h2 className="text-lg font-semibold">Offline storage</h2>
        <p className="text-sm text-muted-foreground">
          This browser does not provide the storage ThunderForge caches worlds
          in, so nothing is being kept on this machine. Everything still works;
          worlds simply load from the server each time.
        </p>
      </section>
    );
  }

  return (
    <section className="grid gap-4" data-testid="storage-panel">
      <div className="grid gap-1">
        <h2 className="text-lg font-semibold">Offline storage</h2>
        <p className="text-sm text-muted-foreground">
          Worlds you have opened are kept on this machine so they load quickly
          and keep working when the connection does not. Clearing any of it
          only affects this browser — nothing on the server changes, and a
          cleared world loads again the next time you open it.
        </p>
      </div>

      <p className="text-sm" data-testid="storage-total">
        <span className="font-medium">{formatBytes(usage?.totalBytes ?? 0)}</span>{" "}
        <span className="text-muted-foreground">
          in use across {rows.length} {rows.length === 1 ? "world" : "worlds"}
        </span>
      </p>

      {rows.length === 0 ? (
        <p className="text-sm text-muted-foreground" data-testid="storage-empty">
          Nothing is stored yet. Open a world and it will be kept here.
        </p>
      ) : (
        <ul className="grid gap-2">
          {rows.map((world) => (
            <li
              key={world.worldId}
              className="flex items-center justify-between gap-4 rounded-md border px-3 py-2"
              data-testid="storage-world-row"
              data-world-id={world.worldId}
            >
              <span className="grid">
                <span className="text-sm font-medium">
                  {world.name ?? "A world no longer in your list"}
                </span>
                <span className="text-xs text-muted-foreground">
                  {world.name ? world.worldId : "Cached content you can reclaim"}
                </span>
              </span>
              <span className="flex items-center gap-3">
                <span className="text-sm tabular-nums" data-testid="storage-world-bytes">
                  {formatBytes(world.bytes)}
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={busy !== null}
                  onClick={() => void clearOne(world.worldId)}
                  data-testid="storage-clear-world"
                >
                  {busy === world.worldId ? "Clearing…" : "Clear"}
                </Button>
              </span>
            </li>
          ))}
        </ul>
      )}

      {rows.length > 0 && (
        <div>
          <Button
            variant="outline"
            size="sm"
            disabled={busy !== null}
            onClick={() => void clearEverything()}
            data-testid="storage-clear-all"
          >
            {busy === "all" ? "Clearing…" : "Clear all stored worlds"}
          </Button>
        </div>
      )}

      {note && (
        <p className="text-sm text-muted-foreground" role="status" data-testid="storage-note">
          {note}
        </p>
      )}
    </section>
  );
}

export default StoragePanel;
