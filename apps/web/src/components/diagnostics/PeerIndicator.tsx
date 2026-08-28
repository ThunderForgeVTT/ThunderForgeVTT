import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { subscribeToEngineMonitor } from "@/services/engineMonitor";
import { subscribeToPeerTransfer, type PeerTransferState } from "@/services/peerTransfer";

/**
 * The visible half of FR-049, on the world canvas (spec 028, T092).
 *
 * # Why here, and why not always
 *
 * FR-049 requires "a visible indicator, not a buried setting". A settings
 * page cannot satisfy that on its own: peer transfer is on by default, so the
 * people most in need of knowing are precisely the ones who never went
 * looking. The exposure it discloses — other participants' devices seeing
 * this one's network address — happens while someone is at the table, so the
 * disclosure belongs at the table, beside the connection status this page
 * already shows for the same reason.
 *
 * It renders only while peers are actually connected. A notice that is always
 * present is one that has stopped being read by the second session, and it
 * would also be false most of the time: with no peers connected, nothing is
 * being revealed to anybody. This deliberately makes the indicator's presence
 * mean something — a direct connection exists right now.
 *
 * It is non-blocking and it is a link, so the disclosure and the control that
 * answers it are one click apart rather than in unrelated places.
 */
export function PeerIndicator() {
  const [state, setState] = useState<PeerTransferState | null>(null);
  const [monitorShowing, setMonitorShowing] = useState(false);

  useEffect(() => subscribeToPeerTransfer(setState), []);
  // The canvas readout reports a peer count of its own, as a link carrying
  // this same explanation. Two overlays saying the same thing at once is
  // the clutter the readout was asked not to be, so this one stands down
  // while that one is showing — and comes back the moment it is switched
  // off, because FR-049's disclosure is not the user's to turn off by
  // hiding a diagnostics panel.
  useEffect(() => subscribeToEngineMonitor(setMonitorShowing), []);

  if (monitorShowing) return null;
  if (!state || !state.enabled || state.connectedPeers < 1) return null;

  return (
    <Link
      to="/settings/storage"
      data-testid="peer-transfer-indicator"
      data-peer-count={state.connectedPeers}
      title="Content is being shared directly with other players, which lets their devices see your network address. Click to manage or turn it off."
      style={{
        position: "absolute",
        // Below the live-sync indicator's row so the two never overlap when
        // both are showing, and clear of the dock's icon rail on the right.
        top: "3.25rem",
        right: "4rem",
        zIndex: 1000,
        padding: "0.4rem 0.75rem",
        borderRadius: "0.375rem",
        background: "rgba(0, 0, 0, 0.75)",
        color: "white",
        fontSize: "0.8rem",
        textDecoration: "none",
      }}
    >
      Sharing directly with {state.connectedPeers}{" "}
      {state.connectedPeers === 1 ? "player" : "players"} — they can see your
      network address
    </Link>
  );
}

export default PeerIndicator;
