import { useEffect, useState } from "react";
import {
  readCacheOrigins,
  type CacheOriginTally,
} from "@/engine/bevy/cacheStats";
import {
  subscribeToWorldCacheSync,
  type WorldCacheSyncSummary,
} from "@/services/worldCacheDiagnostics";
import { formatBytes } from "@/services/worldCacheStorage";
import { detectCacheSupport } from "@/engine/bevy/cacheSupport";

/**
 * What the local cache did for this session, in plain numbers
 * (spec 028 FR-051, SC-017, T122).
 *
 * # Why this lives on the world dock and not on `/settings/storage`
 *
 * `/settings/storage` is the right home for the *durable* facts — how many
 * bytes each world occupies on this disk, and the button that takes them
 * back — and `StoragePanel` reads those straight off OPFS precisely so a
 * settings screen never has to mount the engine.
 *
 * Everything here is the opposite kind of fact. The origin tally lives in the
 * running engine's memory and describes *this world open*: how much of what
 * was asked for came off the disk, how much crossed the wire, what the sync
 * had to repair. There is no engine mounted on a settings page, so the same
 * panel there would report nothing at all for the case it exists to explain —
 * decorative, which SC-017 will not accept. And SC-017's own wording settles
 * where the numbers belong: the outcomes must be confirmable *during an
 * ordinary session*, and the ordinary session is a world being played.
 *
 * So the panel sits behind the dock's Settings tab, two clicks from the map,
 * with the world still open behind it. That is also what makes SC-001 and
 * SC-003 checkable by a person rather than a test: open a world, look; close
 * it, open it again, look again. The second look is the whole criterion.
 *
 * # Zeroes that are real, and absences that are not
 *
 * A mounted engine that has loaded nothing has genuinely loaded nothing, so
 * zeroes are printed. A *missing* tally — no engine, an older bundle — is not
 * zero and is never drawn as one; the panel says it has nothing to report.
 * The distinction matters here more than in most readouts, because "0
 * downloaded from the server" is the headline good news, and a panel that
 * says it when it does not know would be congratulating the user on a cache
 * that never ran.
 *
 * # Nothing here is sent anywhere
 *
 * FR-052/FR-054. These are numbers the running client already holds, printed
 * for the person whose machine holds them. There is no reporting endpoint,
 * and the engine deliberately never assembles anything richer than counts —
 * see `canvas_asset_origins` — so there is nothing here worth transmitting
 * even if something wanted to.
 */

/** How often the tally is re-read while the panel is on screen. */
const SAMPLE_INTERVAL_MS = 1_000;

function Row({
  label,
  value,
  detail,
  testId,
}: {
  label: string;
  value: string;
  detail?: string;
  testId: string;
}) {
  return (
    <div className="grid gap-0.5" data-testid={testId}>
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-sm text-muted-foreground">{label}</span>
        <span className="text-sm tabular-nums">{value}</span>
      </div>
      {detail ? (
        <span className="text-xs text-muted-foreground">{detail}</span>
      ) : null}
    </div>
  );
}

function items(count: number): string {
  return `${count} ${count === 1 ? "item" : "items"}`;
}

export function CachePanel() {
  const [tally, setTally] = useState<CacheOriginTally | null>(null);
  const [sync, setSync] = useState<WorldCacheSyncSummary | null>(null);
  /**
   * Distinguishes "not read yet" from "read, and there is nothing there".
   * Without it the first frame of every mount claims the cache is
   * unavailable, which is the one message on this panel a person might act
   * on.
   */
  const [read, setRead] = useState(false);

  useEffect(() => subscribeToWorldCacheSync(setSync), []);

  // Computed once: whether a browser has these APIs does not change while the
  // page is open, and re-probing on every sample would be noise.
  const [support] = useState(detectCacheSupport);

  useEffect(() => {
    let cancelled = false;
    const sample = () => {
      void readCacheOrigins().then((next) => {
        if (cancelled) return;
        setTally(next);
        setRead(true);
      });
    };
    sample();
    const timer = window.setInterval(sample, SAMPLE_INTERVAL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, []);

  const served = tally ? tally.cacheItems : 0;
  const fetched = tally
    ? tally.networkItems + tally.peerItems + tally.unverifiedItems
    : 0;
  /**
   * The proportion is over items *served* only. A prefetch served nobody —
   * it filled the store for next time — so counting it here would push the
   * denominator up on a warm visit and make a cache that worked perfectly
   * look like it had missed.
   */
  const total = served + fetched;
  /**
   * Bytes that crossed the wire from the server, whether somebody was waiting
   * for them or not. SC-003 is about what a change *cost to bring down*, and
   * on a revisited world the prefetch is usually what brings it — so a
   * "downloaded" figure that omitted it would read zero on exactly the visit
   * the criterion is about.
   */
  const downloadedBytes = tally
    ? tally.networkBytes + tally.unverifiedBytes + tally.prefetchedBytes
    : 0;
  const downloadedItems = tally
    ? tally.networkItems + tally.unverifiedItems + tally.prefetchedItems
    : 0;
  const peerBytes = tally ? tally.peerBytes + tally.prefetchedPeerBytes : 0;
  const peerItems = tally ? tally.peerItems + tally.prefetchedPeerItems : 0;
  const repairs = (sync?.rowsRepaired ?? 0) + (sync?.blobsReclaimed ?? 0);
  const evicted = sync?.evicted ?? 0;

  return (
    <section className="grid gap-2" data-testid="cache-panel">
      <h3 className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
        Content on this device
      </h3>

      {!support.supported ? (
        <p
          className="text-xs text-muted-foreground"
          /*
            A third absence, distinct from the two below and settled in a
            different way. "Not read yet" resolves on its own; "no cache
            running" might change on a reload; *this browser cannot* will not
            change until the person uses a different browser, and telling them
            so is the whole point of FR-042.

            A playtest found this panel reporting nothing served and 0 B
            downloaded against a world that genuinely had content to cache.
            The figures were true and the impression — "nothing has happened
            yet" — was false.
          */
          data-testid="cache-unsupported"
        >
          {`This browser cannot keep world content on your device, so everything is downloaded each time. Missing: ${support.missing.join(", ")}.`}
        </p>
      ) : !tally ? (
        <p
          className="text-xs text-muted-foreground"
          /*
            The two absences are told apart in the DOM as well as in the
            prose. "Not read yet" lasts a single sample and resolves on its
            own; "read, and there is no cache running" is a settled answer.
            Anything reading this panel — a person or a test — needs to be
            able to wait for the first and act on the second, and a shared
            testid would make that impossible.
          */
          data-testid={read ? "cache-absent" : "cache-reading"}
        >
          {read
            ? "The local cache is not running in this session, so everything is coming from the server."
            : "Reading…"}
        </p>
      ) : (
        <div
          className="grid gap-2"
          data-testid="cache-figures"
          /*
            The same numbers as the text, machine-readable. The rendered
            strings are for people and are free to be reworded; these are what
            a check of SC-001/SC-003 reads, so rewording a sentence can never
            silently change what is being verified.
          */
          data-cache-items={tally.cacheItems}
          data-cache-bytes={tally.cacheBytes}
          data-network-items={downloadedItems}
          data-network-bytes={downloadedBytes}
          data-peer-items={peerItems}
          data-peer-bytes={peerBytes}
        >
          <Row
            testId="cache-served-locally"
            label="Loaded from this device"
            value={total === 0 ? "—" : `${served} of ${total}`}
            detail={
              tally.cacheBytes > 0
                ? `${formatBytes(tally.cacheBytes)} that did not have to be downloaded.`
                : "Nothing has been served from the cache in this session yet."
            }
          />
          <Row
            testId="cache-from-server"
            label="Downloaded from the server"
            value={formatBytes(downloadedBytes)}
            detail={`${items(downloadedItems)} this session${
              tally.prefetchedItems > 0
                ? `, ${tally.prefetchedItems} of them fetched ahead of time so they are ready next visit`
                : ""
            }.`}
          />
          {/*
            Shown only when a peer actually supplied something. A permanent
            "0 from other players" line invites worry about a number that is
            supposed to sit still, and peer transfer is a supported thing to
            have switched off entirely (FR-048).
          */}
          {peerItems > 0 ? (
            <Row
              testId="cache-from-peers"
              label="From other players"
              value={formatBytes(peerBytes)}
              detail={`${items(peerItems)}, checked against the server's fingerprint before use.`}
            />
          ) : null}
          {/*
            Likewise: only when it happened. Nothing was lost — the bytes were
            fetched again from the server — so this is worth seeing, not worth
            acting on (FR-046).
          */}
          {tally.unverifiedItems > 0 ? (
            <Row
              testId="cache-unverified"
              label="Did not match and was not stored"
              value={items(tally.unverifiedItems)}
              detail="Those files were shown but not kept, because they did not match what the server promised."
            />
          ) : null}
        </div>
      )}

      {/*
        FR-051's fourth question, and it comes from the sync rather than the
        tally: only the open's integrity pass knows what it had to repair.
        Silent until something happened, because a healthy store repairing
        nothing is the normal case and does not need a line.
      */}
      {repairs > 0 ? (
        <p
          className="text-xs text-muted-foreground"
          data-testid="cache-repairs"
        >
          {items(repairs)} in the local store were unreadable and have been
          cleaned up. They will be downloaded again when needed; nothing was
          lost.
        </p>
      ) : null}
      {evicted > 0 ? (
        <p
          className="text-xs text-muted-foreground"
          data-testid="cache-evicted"
        >
          {items(evicted)} were released to make room or because they are no
          longer part of this world.
        </p>
      ) : null}
      {sync?.budgetInsufficient ? (
        <p className="text-xs text-muted-foreground" data-testid="cache-full">
          There is not enough room on this device to store this world, so
          content is being loaded from the server each time. Clearing other
          worlds in Settings → Storage would help.
        </p>
      ) : null}

      <p className="text-xs text-muted-foreground">
        These figures stay on this device and are never sent anywhere.
      </p>
    </section>
  );
}

export default CachePanel;
