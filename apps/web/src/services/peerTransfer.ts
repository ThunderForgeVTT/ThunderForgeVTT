/**
 * The user's say over peer transfer, and what it is currently doing
 * (spec 028 FR-049, contracts/peer-protocol.md § Privacy).
 *
 * # Why this is a service and not component state
 *
 * Two very different callers need it. The engine's wasm asks *may I* before
 * opening a peer connection at all, on a page that may never render a
 * settings panel; the panel asks *what is happening* so it can show the
 * indicator FR-049 requires. Putting the answer in a component would mean
 * the engine could only ask when that component happened to be mounted —
 * which is the shape that produces "the setting is off and it connected
 * anyway".
 *
 * # Enabled by default, and said out loud
 *
 * FR-049 decides the default: peer-to-peer with server adjudication is the
 * intended model, not an opt-in extra. The cost is that a direct connection
 * reveals IP addresses between participants, so the requirement pairs the
 * default with disclosure — a *visible indicator*, not a buried setting.
 * Turning it off also forfeits server-isolated play (FR-057), because that
 * rests on the same connections, and the user has to be told that rather
 * than discovering later that a capability quietly went away.
 */

/** Persisted per user, per browser profile. */
const STORAGE_KEY = "thunderforge:peer-transfer-enabled";

/** What the indicator renders, and what the engine reports into. */
export interface PeerTransferState {
  /** The user's setting. Everything else is meaningless when false. */
  enabled: boolean;
  /** Peer data channels currently open for the world in view. */
  connectedPeers: number;
  /** Bytes taken from peers this session, for the indicator. */
  bytesFromPeers: number;
  /**
   * Peer responses that failed the fingerprint check and were discarded
   * (FR-046). Surfaced because a peer that cannot be trusted to send the
   * bytes it was asked for is worth seeing, not because the user must act:
   * the fallback already happened.
   */
  verificationFailures: number;
}

type Listener = (state: PeerTransferState) => void;

const listeners = new Set<Listener>();

function readEnabled(): boolean {
  try {
    // Absent means never set, which is the default: on (FR-049).
    return window.localStorage.getItem(STORAGE_KEY) !== "false";
  } catch {
    // A profile with site data blocked still gets the default rather than a
    // thrown error on a page that merely wanted to draw a canvas.
    return true;
  }
}

let state: PeerTransferState = {
  enabled: typeof window === "undefined" ? true : readEnabled(),
  connectedPeers: 0,
  bytesFromPeers: 0,
  verificationFailures: 0,
};

function publish(): void {
  for (const listener of listeners) listener(state);
}

/** The current state, for a caller that does not want to subscribe. */
export function getPeerTransferState(): Readonly<PeerTransferState> {
  return state;
}

/**
 * Whether a peer connection may be opened at all.
 *
 * Asked by the engine before signaling, so that disabling it means no
 * connection is ever made — not that one is made and then ignored. The IP
 * exposure the setting exists to prevent happens at connection time.
 */
export function isPeerTransferEnabled(): boolean {
  return state.enabled;
}

/**
 * Turn peer transfer on or off.
 *
 * Disabling zeroes the activity counters as well, because leaving "3 peers"
 * on screen next to a disabled setting says the setting did not take.
 */
export function setPeerTransferEnabled(enabled: boolean): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, enabled ? "true" : "false");
  } catch {
    // The setting still applies to this session; it just will not persist.
  }
  state = enabled
    ? { ...state, enabled: true }
    : {
        enabled: false,
        connectedPeers: 0,
        bytesFromPeers: 0,
        verificationFailures: 0,
      };
  publish();
}

/**
 * Report what peer transfer is doing. Called by the engine bridge.
 *
 * Ignored entirely while disabled: a late report from a connection that was
 * closing as the user turned the setting off must not resurrect the
 * indicator.
 */
export function reportPeerTransferActivity(
  activity: Partial<Omit<PeerTransferState, "enabled">>,
): void {
  if (!state.enabled) return;
  state = { ...state, ...activity };
  publish();
}

/** Observe the setting and the activity. Returns an unsubscribe. */
export function subscribeToPeerTransfer(listener: Listener): () => void {
  listeners.add(listener);
  listener(state);
  return () => {
    listeners.delete(listener);
  };
}

/** Reset module state. Tests only. */
export function resetPeerTransferForTests(): void {
  listeners.clear();
  state = {
    enabled: true,
    connectedPeers: 0,
    bytesFromPeers: 0,
    verificationFailures: 0,
  };
}
