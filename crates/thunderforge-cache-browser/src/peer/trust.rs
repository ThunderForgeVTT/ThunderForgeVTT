//! Who we will talk to, and what to tell the user about it.

use super::*;

// ---------------------------------------------------------------------------

/// Peers this client has stopped asking, and why it stopped.
///
/// Session-lifetime and nothing more (FR-050). There is no persistence and no
/// reporting: a peer that sent bad bytes is dropped from *this* client's
/// consideration and no further conclusion is drawn, because a peer behind a
/// broken proxy and a peer being malicious are indistinguishable from here
/// and the response to both is identical anyway.
#[derive(Debug, Clone, Default)]
pub struct PeerTrust {
    distrusted: BTreeSet<String>,
}

impl PeerTrust {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this peer may still be asked for content.
    pub fn trusts(&self, peer: &str) -> bool {
        !self.distrusted.contains(peer)
    }

    /// Record the outcome of a transfer. Only the outcomes the contract calls
    /// out — mismatched or fabricated content — cost a peer its trust.
    pub fn record(&mut self, peer: &str, fallback: Fallback) {
        if fallback.distrusts_peer() {
            self.distrusted.insert(peer.to_string());
        }
    }

    pub fn distrusted_count(&self) -> usize {
        self.distrusted.len()
    }
}

/// What the FR-049 indicator shows.
///
/// Mirrors `PeerTransferState` in `apps/web/src/services/peerTransfer.ts`
/// minus `enabled`, which is the user's to set and never this side's to
/// report. Counters only: no peer identities, no addresses, no timings — the
/// panel exists to disclose that peer transfer is happening, not to profile
/// who is in the game (FR-052, FR-054).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PeerActivity {
    pub connected_peers: usize,
    pub bytes_from_peers: u64,
    pub verification_failures: u32,
}

impl PeerActivity {
    /// The exact object `reportPeerTransferActivity` takes.
    pub fn to_json(self) -> String {
        serde_json::json!({
            "connectedPeers": self.connected_peers,
            "bytesFromPeers": self.bytes_from_peers,
            "verificationFailures": self.verification_failures,
        })
        .to_string()
    }
}
