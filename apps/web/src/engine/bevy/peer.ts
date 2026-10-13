import type { Client } from "graphql-ws";

import { postGraphQL } from "../../api/graphqlClient";
import { reportPlayPausedIn } from "../../api/playPauseSignal";
import {
  isPeerTransferEnabled,
  reportPeerTransferActivity,
} from "../../services/peerTransfer";
import { createSocketClient } from "../world/sync/socketClient";
import { getWasmModule } from "./wasmModule";

// ---------------------------------------------------------------------------
// Peer-assisted content distribution (spec 028 T088-T091, FR-044 to FR-050)
// ---------------------------------------------------------------------------

/**
 * The signaling half, and only the signaling half.
 *
 * The protocol — framing, what may be requested, verification, what may be
 * served, rate limits — lives in `crates/thunderforge-cache-browser/src/peer.rs`
 * and never crosses this boundary. What is here is the part TypeScript already
 * owns: the `graphql-ws` connection the SDP and ICE payloads ride (ADR-048),
 * and the user's setting. The engine hands out opaque strings and takes opaque
 * strings back; nothing in this file interprets one.
 */
const PEER_SIGNAL_MUTATION = `
mutation SendPeerSignal($input: PeerSignalInput!) {
  sendPeerSignal(input: $input)
}`;

const PEER_SESSIONS_QUERY = `
query PeerSessions($worldId: UUID!) {
  peerSessions(worldId: $worldId)
}`;

const PEER_SIGNALS_SUBSCRIPTION = `
subscription PeerSignals($worldId: UUID!, $sessionId: String!) {
  peerSignals(worldId: $worldId, sessionId: $sessionId) {
    fromSessionId
    payload
  }
}`;

/** How often the indicator is refreshed while peer transfer is running. */
const PEER_ACTIVITY_POLL_MS = 1_000;

interface PeerTransferSession {
  worldId: string;
  sessionId: string;
  client: Client;
  dispose: () => void;
  poll: ReturnType<typeof setInterval>;
}

let peerSession: PeerTransferSession | null = null;

/**
 * Start peer transfer for one world.
 *
 * **The setting is consulted first, before anything opens.** That ordering is
 * the whole point of the check: a direct peer connection is what reveals an IP
 * address to another participant, so "disabled" has to mean no connection was
 * ever attempted, not that one was attempted and its bytes ignored
 * (`services/peerTransfer.ts`).
 *
 * Never throws and never reports failure. Every way this can not happen — the
 * setting is off, the bundle has no peer entry points, the signaling server
 * does not answer, no other session is live — leaves the client fetching from
 * the server, which is what it would have done anyway (FR-048, SC-013).
 */
export async function startPeerTransfer(worldId: string): Promise<boolean> {
  if (!isPeerTransferEnabled()) return false;
  if (peerSession?.worldId === worldId) return true;
  stopPeerTransfer();

  try {
    const module = await getWasmModule();
    if (
      !module.start_peer_transfer ||
      !module.receive_peer_signal ||
      !module.offer_to_peer
    ) {
      return false;
    }

    // One session id per page load, generated here and meaningful to nobody
    // else. It is an address for signals and not an identity: it is not the
    // user id, it is not stored, and it does not survive a reload (FR-050).
    const sessionId = crypto.randomUUID();

    const started = module.start_peer_transfer(
      worldId,
      sessionId,
      (to, payload) => {
        // Fire and forget. A signal that does not arrive costs one peer
        // connection, and the contract already says the server does not
        // promise reachability.
        // `fromSessionId` is mandatory and **verified, not trusted**: the
        // server only relays as a session that is registered, in this world,
        // to this user. So it must be the same id the subscription registered
        // with, and a mismatch is silently `false` rather than an error.
        //
        // A `false` reply is "that peer is gone" — the session ended, or lost
        // membership — and is not a transport failure. There is nothing to
        // retry: an unanswered offer simply never becomes a channel, and the
        // fetch it would have served falls to the server like every other
        // peer failure.
        void postGraphQL(PEER_SIGNAL_MUTATION, {
          input: {
            worldId,
            fromSessionId: sessionId,
            toSessionId: to,
            payload,
          },
        }).catch(() => undefined);
      },
    );
    if (!started) return false;

    // A connection of this module's own rather than the world-event socket's
    // singleton, which `engine/world/sync/subscriptionClient.ts` keeps private.
    // It is opened only for a world page with peer transfer enabled and closed
    // with it, so the cost is one socket for as long as the feature is on —
    // and sharing the other one would mean reaching into a module this change
    // does not own.
    // Through the shared factory so it pings like the other socket: the
    // server closes a socket that says nothing for a minute.
    const client = createSocketClient({ retryAttempts: Infinity });

    const dispose = client.subscribe<{
      peerSignals: { fromSessionId: string; payload: string };
    }>(
      { query: PEER_SIGNALS_SUBSCRIPTION, variables: { worldId, sessionId } },
      {
        next: (result) => {
          // Spec 051: signalling ends with the world's play. The page that
          // owns play closes the channels; this only says why.
          reportPlayPausedIn(result.errors);
          const signal = result.data?.peerSignals;
          if (!signal) return;
          void module.receive_peer_signal?.(
            signal.fromSessionId,
            signal.payload,
          );
        },
        // Signaling unavailable means peer transfer is off for the session and
        // everything else works (peer-protocol.md, "Failure modes"). There is
        // nothing to tell the user and nothing for them to do.
        error: () => undefined,
        complete: () => undefined,
      },
    );

    const poll = setInterval(() => {
      try {
        const raw = module.peer_transfer_activity?.();
        if (raw) {
          reportPeerTransferActivity(
            JSON.parse(raw) as {
              connectedPeers: number;
              bytesFromPeers: number;
              verificationFailures: number;
            },
          );
        }
      } catch {
        // A poll that fails is one missed indicator refresh.
      }
    }, PEER_ACTIVITY_POLL_MS);

    peerSession = { worldId, sessionId, client, dispose, poll };

    // The newcomer always initiates: query the roster and offer to each. That
    // is why there is no join/leave push in the contract — arrivals offer, and
    // departures are noticed when a channel closes.
    //
    // Deliberately not awaited. This function resolves once the engine is
    // *able* to talk to peers, and the caller's next act is the world-cache
    // sync that decides what may be asked for; making it wait on a roster
    // round trip would put a network hop in front of the thing the user is
    // actually watching, for a benefit that lands seconds later anyway.
    void (async () => {
      try {
        const roster = await postGraphQL<{ peerSessions: string[] }>(
          PEER_SESSIONS_QUERY,
          { worldId },
        );
        // The roster already excludes every session belonging to this user,
        // not merely this one: two tabs of one browser share an origin and
        // therefore share the cache a transfer would be filling, so there is
        // nothing to gain between them.
        for (const peer of roster.peerSessions ?? []) {
          // Stop if peer transfer was torn down while the roster was in
          // flight; offering from a session that no longer exists would be
          // relayed as nobody.
          if (peerSession?.sessionId !== sessionId) return;
          void module.offer_to_peer?.(peer);
        }
      } catch {
        // No roster is "no peers available": server fetch, no user-visible
        // difference. The subscription stays open, so a later arrival
        // offering to us still connects.
      }
    })();

    return true;
  } catch {
    stopPeerTransfer();
    return false;
  }
}

// ---------------------------------------------------------------------------
// Peer adjudication (spec 028 US7, T098/T100/T101, FR-057 to FR-062)
// ---------------------------------------------------------------------------

/**
 * Ask the peer fabric to begin adjudicated play.
 *
 * **The caller decides the server is gone**, from the heartbeat and nothing
 * else — this is signalling, not a second opinion about connectivity. What
 * the fabric decides is the rest: whether there is a table to play with at
 * all, and, on a player's client, whether the Game Master is among them,
 * which is a thing no count on this side can answer (FR-059).
 *
 * Never throws. Every way it can fail — no bundle, no peers, peer transfer
 * off — is plain offline, which is a path that already works.
 */
export async function beginPeerAdjudication(
  selfUserId: string,
  gmUserId: string,
  onApplied: (changeJson: string) => void,
): Promise<boolean> {
  try {
    const module = await getWasmModule();
    return (
      module.begin_peer_adjudication?.(selfUserId, gmUserId, onApplied) ?? false
    );
  } catch {
    return false;
  }
}

/** Whether peer-adjudicated play is running right now. */
export async function peerAdjudicationActive(): Promise<boolean> {
  try {
    const module = await getWasmModule();
    return module.peer_adjudication_active?.() ?? false;
  } catch {
    return false;
  }
}

/**
 * The server is reachable again: stop adjudicating, and keep what was applied
 * for submission (FR-062).
 */
export async function endPeerAdjudication(
  serverReturned: boolean,
): Promise<void> {
  try {
    const module = await getWasmModule();
    if (serverReturned) {
      module.peer_adjudication_server_returned?.();
    } else {
      module.end_peer_adjudication?.();
    }
  } catch {
    // Nothing to stop, or no bundle to stop it in.
  }
}

/**
 * Everything applied while server-isolated, as JSON.
 *
 * Provisional, all of it. The Game Master's client submits these over its own
 * authenticated session and the server confirms or rejects each one; its
 * decision is final (FR-062).
 */
export async function peerAdjudicationSubmissions(): Promise<string> {
  try {
    const module = await getWasmModule();
    return module.peer_adjudication_submissions?.() ?? "[]";
  } catch {
    return "[]";
  }
}

/**
 * Put one token movement to the table.
 *
 * Position, rotation and scale only, and there is no argument for anything
 * else (FR-060). `false` means "not adjudicating — queue it in the outbox
 * instead", which is the caller's single fall-back.
 */
export async function proposeTokenTransform(
  entityId: string,
  transform: { x?: number; y?: number; rotation?: number; scale?: number },
): Promise<boolean> {
  try {
    const module = await getWasmModule();
    return (
      module.propose_token_transform?.(
        entityId,
        transform.x,
        transform.y,
        transform.rotation,
        transform.scale,
      ) ?? false
    );
  } catch {
    return false;
  }
}

/**
 * Stop peer transfer and close every channel.
 *
 * Called when the world closes, when the user turns the setting off, and on
 * unload. Idempotent, and safe to call when it was never started — peer
 * connections must not outlive the session that justified them (FR-050).
 */
export function stopPeerTransfer(): void {
  const session = peerSession;
  peerSession = null;
  if (session) {
    clearInterval(session.poll);
    try {
      session.dispose();
    } catch {
      // Already gone.
    }
    void session.client.dispose();
  }
  reportPeerTransferActivity({
    connectedPeers: 0,
    bytesFromPeers: 0,
    verificationFailures: 0,
  });
  void getWasmModule()
    .then((module) => module.stop_peer_transfer?.())
    .catch(() => undefined);
}

/** Whether peer transfer is running, for tests and diagnostics. */
export function isPeerTransferRunning(): boolean {
  return peerSession !== null;
}
