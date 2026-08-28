import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { readEngineStats } from "@/engine/bevy/stats";
import { getHeartbeatLatencyMs } from "@/engine/world/sync/heartbeat";
import { subscribeToEngineMonitor } from "@/services/engineMonitor";
import {
  subscribeToPeerTransfer,
  type PeerTransferState,
} from "@/services/peerTransfer";

/**
 * Frames, latency and peers, along the bottom of the canvas.
 *
 * # An indicator, not an instrument panel
 *
 * Three numbers, one line, low contrast, no chrome. The temptation with a
 * readout like this is to keep adding to it — sprite counts, draw calls,
 * shadow quads, all of which `engine_stats` already reports — until it is a
 * panel sitting on top of the map. These three earn their place because
 * each one answers a question a player actually asks during play: *is my
 * machine struggling*, *is the connection bad*, *am I connected to anyone*.
 * Anything that only a developer would ask belongs in the frame trace,
 * which is already there and already better at it.
 *
 * Off by default and remembered once turned on — see `engineMonitor.ts`.
 *
 * # Why the numbers are allowed to be absent
 *
 * Each of the three can be genuinely unknown: the engine may not be
 * mounted, no heartbeat may have completed yet, peer transfer may be
 * switched off. Every one of those renders an em dash rather than a zero. A
 * confident `0 fps` over a canvas that is animating, or `0 ms` on a dead
 * connection, is worse than an obvious blank — it answers the question
 * wrongly instead of admitting it has no answer.
 *
 * # The peer count carries a requirement, not just a statistic
 *
 * FR-049 obliges the app to tell people peer transfer is in use, because a
 * direct connection reveals their network address. `PeerIndicator` is that
 * disclosure. When this readout is on it would sit directly above it saying
 * the same thing twice, so `PeerIndicator` stands down and the count here
 * takes over the duty — which is why this one is a link to the setting and
 * carries the same explanation in its title, rather than being a bare
 * number.
 */

/** How often the readout refreshes. */
const SAMPLE_INTERVAL_MS = 1_000;

export function EngineMonitor() {
  const [visible, setVisible] = useState(false);
  const [fps, setFps] = useState<number | null>(null);
  const [latency, setLatency] = useState<number | null>(null);
  const [peers, setPeers] = useState<PeerTransferState | null>(null);

  useEffect(() => subscribeToEngineMonitor(setVisible), []);
  useEffect(() => subscribeToPeerTransfer(setPeers), []);

  useEffect(() => {
    if (!visible) return;

    // Sampled on a timer rather than per frame. A readout that re-rendered
    // React every frame would be a measurable part of the frame time it
    // claims to be reporting, and a number that changes 60 times a second
    // is unreadable anyway.
    let cancelled = false;
    const sample = () => {
      void readEngineStats().then((stats) => {
        if (!cancelled) setFps(stats ? Math.round(stats.fps) : null);
      });
      if (!cancelled) setLatency(getHeartbeatLatencyMs());
    };
    sample();
    const timer = setInterval(sample, SAMPLE_INTERVAL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [visible]);

  if (!visible) return null;

  const peerCount = peers?.enabled ? peers.connectedPeers : null;

  return (
    <div
      data-testid="engine-monitor"
      className="pointer-events-none absolute bottom-3 left-1/2 z-50 -translate-x-1/2"
    >
      <div className="flex items-center gap-3 rounded-full border border-border/40 bg-background/70 px-3 py-1 font-mono text-[11px] text-muted-foreground backdrop-blur-sm">
        <span data-testid="engine-monitor-fps" title="Frames per second">
          {fps === null ? "—" : fps} fps
        </span>
        <span aria-hidden className="text-border">
          ·
        </span>
        <span
          data-testid="engine-monitor-latency"
          title="Round trip to the server, from the session heartbeat"
        >
          {latency === null ? "—" : latency} ms
        </span>
        <span aria-hidden className="text-border">
          ·
        </span>
        {peerCount === null ? (
          <span data-testid="engine-monitor-peers">— peers</span>
        ) : (
          <Link
            to="/settings/storage"
            data-testid="engine-monitor-peers"
            data-peer-count={peerCount}
            title={
              peerCount > 0
                ? "Sharing map files directly with other players — they can see your network address. Change this in storage settings."
                : "No direct connections to other players right now."
            }
            className="pointer-events-auto underline-offset-2 hover:underline"
          >
            {peerCount} {peerCount === 1 ? "peer" : "peers"}
          </Link>
        )}
      </div>
    </div>
  );
}
