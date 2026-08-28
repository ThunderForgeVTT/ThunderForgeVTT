//! Spec 028 T088–T091: a peer may waste your bandwidth and nothing else.
//!
//! Every test here is about one of the two properties the whole protocol
//! rests on. **Content is verified against the fingerprint that was asked
//! for, before anything is done with it** (FR-046, SC-012), and **what may be
//! asked for comes from the server's plan** (FR-047, SC-014). The rest —
//! declines, stalls, disconnects, floods — exists to show the third property:
//! that every one of them ends at the server, so a peer can make a fetch
//! slower and can never make it wrong (FR-048, SC-013).

use thunderforge_cache_browser::peer::{
    CHUNK_BYTES, DeclineReason, DownloadStep, Fallback, MAX_BYTES_PER_WINDOW,
    MAX_CONCURRENT_SERVES, MAX_REQUESTS_PER_WINDOW, PeerActivity, PeerDownload, PeerMessage,
    PeerServer, PeerTrust, PlanScope, RATE_WINDOW_MS, STALL_MS, ServeDecision, serve_frames,
};
use thunderforge_cache_core::delta::{PlanItem, SyncPlan};
use thunderforge_cache_core::{Fingerprint, ItemId};
use uuid::Uuid;

fn world() -> Uuid {
    Uuid::from_u128(0x1234_5678_9abc_def0_1234_5678_9abc_def0)
}

fn content(seed: u8, len: usize) -> Vec<u8> {
    (0..len).map(|i| seed.wrapping_add(i as u8)).collect()
}

/// A plan listing exactly these blobs, with the server's byte counts.
fn plan_for(items: &[&[u8]]) -> SyncPlan {
    SyncPlan {
        fetch: items
            .iter()
            .enumerate()
            .map(|(i, bytes)| PlanItem {
                id: ItemId::CanvasAsset(Uuid::from_u128(i as u128 + 1)),
                fingerprint: Fingerprint::of_bytes(bytes),
                byte_size: bytes.len() as u64,
            })
            .collect(),
        evict: Vec::new(),
    }
}

/// Drive a whole transfer through a download, returning the last step.
fn deliver(download: &mut PeerDownload, frames: &[PeerMessage], now_ms: u64) -> DownloadStep {
    let mut last = DownloadStep::Continue;
    for frame in frames {
        last = download.on_frame(&frame.encode(), now_ms);
    }
    last
}

// ---------------------------------------------------------------------------
// T088 — framing
// ---------------------------------------------------------------------------

#[test]
fn every_message_survives_the_wire_unchanged() {
    // Catches a framing change that silently reinterprets one message as
    // another — the failure that would let a `DECLINE` be read as a `DONE`
    // and end a transfer at the verification step with an empty buffer.
    let fingerprint = Fingerprint::of_bytes(b"map");
    let messages = [
        PeerMessage::Request { fingerprint },
        PeerMessage::Offer {
            fingerprint,
            byte_size: 40_000,
        },
        PeerMessage::Chunk {
            fingerprint,
            seq: 7,
            bytes: content(3, 512),
        },
        PeerMessage::Done { fingerprint },
        PeerMessage::Decline {
            fingerprint,
            reason: DeclineReason::Busy,
        },
    ];
    for message in messages {
        let encoded = message.encode();
        assert_eq!(
            PeerMessage::decode(&encoded),
            Some(message.clone()),
            "{message:?} did not survive encode/decode"
        );
        assert_eq!(
            PeerMessage::decode(&encoded).unwrap().fingerprint(),
            fingerprint
        );
    }
}

#[test]
fn a_truncated_or_overlong_frame_is_not_a_message() {
    // A peer writes whatever it likes onto this channel. A frame that is not
    // exactly the shape declared must never be half-parsed and acted on, so
    // the decoder answers `None` rather than reaching for a default.
    let fingerprint = Fingerprint::of_bytes(b"map");
    let offer = PeerMessage::Offer {
        fingerprint,
        byte_size: 9,
    }
    .encode();

    assert_eq!(PeerMessage::decode(&[]), None);
    assert_eq!(PeerMessage::decode(&offer[..offer.len() - 1]), None);
    let mut overlong = offer.clone();
    overlong.push(0);
    assert_eq!(PeerMessage::decode(&overlong), None);
    // An unknown tag is a message from something that is not this protocol.
    let mut alien = offer.clone();
    alien[0] = 200;
    assert_eq!(PeerMessage::decode(&alien), None);
    // A decline reason we do not know is not a decline we may act on.
    let mut bad_reason = PeerMessage::Decline {
        fingerprint,
        reason: DeclineReason::NotHeld,
    }
    .encode();
    *bad_reason.last_mut().unwrap() = 99;
    assert_eq!(PeerMessage::decode(&bad_reason), None);
}

#[test]
fn serving_frames_chunk_in_order_and_never_exceed_the_channel_limit() {
    // 16 KiB is the largest payload every browser's SCTP accepts. A change
    // that raised it would work in Chrome and fail in Safari, which is worse
    // than a chattier protocol that works everywhere.
    let bytes = content(11, CHUNK_BYTES * 3 + 17);
    let fingerprint = Fingerprint::of_bytes(&bytes);
    let frames = serve_frames(&fingerprint, &bytes);

    assert!(
        matches!(frames.first(), Some(PeerMessage::Offer { byte_size, .. }) if *byte_size == bytes.len() as u64)
    );
    assert!(matches!(frames.last(), Some(PeerMessage::Done { .. })));
    for (expected_seq, frame) in frames[1..frames.len() - 1].iter().enumerate() {
        let PeerMessage::Chunk { seq, bytes, .. } = frame else {
            panic!("expected only chunks between the offer and the done");
        };
        assert_eq!(*seq as usize, expected_seq);
        assert!(bytes.len() <= CHUNK_BYTES);
    }
}

// ---------------------------------------------------------------------------
// T089 — entitlement comes from the server's plan
// ---------------------------------------------------------------------------

#[test]
fn a_fingerprint_absent_from_the_plan_cannot_be_requested() {
    // SC-014, and the reason the whole feature is safe: this is the *only*
    // constructor of a `PeerRequest` in the program, and `PeerDownload::begin`
    // takes one by value. There is no other expression that asks a peer for
    // something. If this returns `None`, nothing can be asked for.
    let mine = content(1, 64);
    let someone_elses = content(2, 64);
    let scope = PlanScope::from_plan(world(), &plan_for(&[&mine]));

    assert!(scope.request(&Fingerprint::of_bytes(&mine)).is_some());
    assert!(
        scope
            .request(&Fingerprint::of_bytes(&someone_elses))
            .is_none(),
        "a fingerprint the server did not put in this client's plan must not be requestable",
    );
}

#[test]
fn a_scope_with_no_plan_asks_for_nothing() {
    // The state a client is in before its first sync, and the state it stays
    // in if a plan never parses. Both must mean server-only rather than
    // "ask for anything", which is what a default-permissive scope would do.
    let scope = PlanScope::none(world());
    assert!(scope.is_empty());
    assert!(scope.request(&Fingerprint::of_bytes(b"anything")).is_none());
}

#[test]
fn replacing_the_plan_replaces_what_may_be_asked_for() {
    // Revocation. A scope that accumulated across syncs would let a client go
    // on asking peers for content the server has stopped listing — the same
    // hole `apply_plan` closes for content already on disk.
    let old = content(1, 32);
    let new = content(2, 32);
    let before = PlanScope::from_plan(world(), &plan_for(&[&old]));
    let after = PlanScope::from_plan(world(), &plan_for(&[&new]));

    assert!(before.request(&Fingerprint::of_bytes(&old)).is_some());
    assert!(after.request(&Fingerprint::of_bytes(&old)).is_none());
    assert!(after.request(&Fingerprint::of_bytes(&new)).is_some());
}

#[test]
fn a_request_carries_the_servers_byte_count_not_the_peers() {
    // What makes an `OFFER` checkable at all. Without the server's figure
    // there is nothing to compare a peer's claim against.
    let bytes = content(5, 999);
    let scope = PlanScope::from_plan(world(), &plan_for(&[&bytes]));
    let request = scope.request(&Fingerprint::of_bytes(&bytes)).unwrap();
    assert_eq!(request.expected_bytes(), 999);
    assert_eq!(request.world_id(), world());
}

// ---------------------------------------------------------------------------
// T090 — verify before storing, and there is no other way out
// ---------------------------------------------------------------------------

#[test]
fn a_complete_honest_transfer_yields_verified_bytes() {
    // The one path that produces content, so that every test below can be
    // read as "and this one does not".
    let bytes = content(7, CHUNK_BYTES * 2 + 5);
    let scope = PlanScope::from_plan(world(), &plan_for(&[&bytes]));
    let request = scope.request(&Fingerprint::of_bytes(&bytes)).unwrap();
    let (mut download, frame) = PeerDownload::begin(request, 0);

    assert_eq!(
        PeerMessage::decode(&frame),
        Some(PeerMessage::Request {
            fingerprint: Fingerprint::of_bytes(&bytes)
        }),
    );

    let step = deliver(
        &mut download,
        &serve_frames(&Fingerprint::of_bytes(&bytes), &bytes),
        10,
    );
    assert_eq!(step, DownloadStep::Verified(bytes));
}

#[test]
fn bytes_that_do_not_hash_to_what_was_asked_for_are_discarded_and_the_server_is_used() {
    // SC-012, with a deliberately corrupted response. The peer sends a
    // complete, well-framed, correctly-sized transfer of the *wrong content*
    // — which is the only interesting attack, because everything else is
    // caught before any bytes arrive. Nothing may come out of this but a
    // fall-back, and the peer must not be asked again.
    let wanted = content(1, 4096);
    let substituted = content(9, 4096);
    assert_eq!(wanted.len(), substituted.len());

    let scope = PlanScope::from_plan(world(), &plan_for(&[&wanted]));
    let fingerprint = Fingerprint::of_bytes(&wanted);
    let request = scope.request(&fingerprint).unwrap();
    let (mut download, _) = PeerDownload::begin(request, 0);

    // Framed under the requested fingerprint, carrying someone else's bytes.
    let frames = serve_frames(&fingerprint, &substituted);
    let step = deliver(&mut download, &frames, 10);

    assert_eq!(step, DownloadStep::FallBack(Fallback::VerificationFailed));
    assert_eq!(
        download.received(),
        0,
        "the buffer must be dropped, not left where a later change could reach it",
    );

    let mut trust = PeerTrust::new();
    trust.record("liar", Fallback::VerificationFailed);
    assert!(
        !trust.trusts("liar"),
        "a peer that sent bad bytes is not retried"
    );
}

#[test]
fn a_peer_disconnecting_mid_transfer_stores_nothing() {
    // "No partial stores": the server serves the remainder, and the half a
    // peer managed to send is not merely unused, it is unreachable — the
    // buffer is private and the only public path out of this type is
    // `Verified`, which is behind the hash check.
    let bytes = content(3, CHUNK_BYTES * 4);
    let fingerprint = Fingerprint::of_bytes(&bytes);
    let scope = PlanScope::from_plan(world(), &plan_for(&[&bytes]));
    let (mut download, _) = PeerDownload::begin(scope.request(&fingerprint).unwrap(), 0);

    let frames = serve_frames(&fingerprint, &bytes);
    // Offer plus two of the four chunks, then the peer vanishes.
    deliver(&mut download, &frames[..3], 10);
    assert!(
        download.received() > 0,
        "the fixture must actually be mid-transfer"
    );

    assert_eq!(
        download.peer_gone(),
        DownloadStep::FallBack(Fallback::PeerGone)
    );
    assert_eq!(download.received(), 0);
    // And the corpse cannot be revived by the rest of the transfer arriving.
    assert_eq!(
        deliver(&mut download, &frames[3..], 20),
        DownloadStep::Ignore
    );
}

#[test]
fn content_for_a_fingerprint_that_was_not_asked_for_is_ignored_entirely() {
    // Not "rejected" — ignored. It must not advance the state machine, must
    // not be buffered, and must not end the transfer that is actually in
    // progress, or any peer could cancel any fetch by shouting.
    let wanted = content(1, 1024);
    let other = content(2, 1024);
    let fingerprint = Fingerprint::of_bytes(&wanted);
    let scope = PlanScope::from_plan(world(), &plan_for(&[&wanted]));
    let (mut download, _) = PeerDownload::begin(scope.request(&fingerprint).unwrap(), 0);

    for frame in serve_frames(&Fingerprint::of_bytes(&other), &other) {
        assert_eq!(
            download.on_frame(&frame.encode(), 10),
            DownloadStep::Ignore,
            "unrequested content must not touch this transfer",
        );
    }
    assert_eq!(download.received(), 0);

    // The real transfer still completes, unaffected.
    assert_eq!(
        deliver(&mut download, &serve_frames(&fingerprint, &wanted), 20),
        DownloadStep::Verified(wanted),
    );
}

#[test]
fn a_decline_ends_this_transfer_and_says_nothing_about_the_content() {
    // FR-045. The server's plan said this content exists; a stranger saying
    // "no" does not overrule it. So a decline is a fall-back like any other,
    // it does not cost the peer its trust, and — the part that matters —
    // there is nowhere for a caller to learn *which* reason was given.
    let bytes = content(1, 128);
    let fingerprint = Fingerprint::of_bytes(&bytes);
    let scope = PlanScope::from_plan(world(), &plan_for(&[&bytes]));

    for reason in [
        DeclineReason::NotHeld,
        DeclineReason::NotPermitted,
        DeclineReason::Busy,
    ] {
        let (mut download, _) = PeerDownload::begin(scope.request(&fingerprint).unwrap(), 0);
        assert_eq!(
            download.on_frame(
                &PeerMessage::Decline {
                    fingerprint,
                    reason
                }
                .encode(),
                5
            ),
            DownloadStep::FallBack(Fallback::Declined),
            "every decline reason must be indistinguishable to the requester",
        );
    }

    let mut trust = PeerTrust::new();
    trust.record("polite", Fallback::Declined);
    assert!(
        trust.trusts("polite"),
        "a peer that does not hold something has done nothing wrong",
    );
}

#[test]
fn an_offer_disagreeing_with_the_servers_size_ends_the_transfer_before_any_bytes_arrive() {
    // The server's byte count is the authority. A peer offering a different
    // size is offering different content, and the cheapest place to find that
    // out is before allocating for it — a hostile `OFFER` must not be a way
    // to make this client reserve a gigabyte.
    let bytes = content(1, 1024);
    let fingerprint = Fingerprint::of_bytes(&bytes);
    let scope = PlanScope::from_plan(world(), &plan_for(&[&bytes]));
    let (mut download, _) = PeerDownload::begin(scope.request(&fingerprint).unwrap(), 0);

    assert_eq!(
        download.on_frame(
            &PeerMessage::Offer {
                fingerprint,
                byte_size: 8 * 1024 * 1024 * 1024,
            }
            .encode(),
            5,
        ),
        DownloadStep::FallBack(Fallback::SizeMismatch),
    );
}

#[test]
fn a_peer_sending_more_than_it_offered_is_cut_off_rather_than_buffered() {
    // Otherwise an offer of one byte followed by an endless stream of chunks
    // is an out-of-memory primitive any participant can reach for.
    let bytes = content(1, 2048);
    let fingerprint = Fingerprint::of_bytes(&bytes);
    let scope = PlanScope::from_plan(world(), &plan_for(&[&bytes]));
    let (mut download, _) = PeerDownload::begin(scope.request(&fingerprint).unwrap(), 0);

    download.on_frame(
        &PeerMessage::Offer {
            fingerprint,
            byte_size: 2048,
        }
        .encode(),
        5,
    );
    download.on_frame(
        &PeerMessage::Chunk {
            fingerprint,
            seq: 0,
            bytes: content(1, 2048),
        }
        .encode(),
        6,
    );
    assert_eq!(
        download.on_frame(
            &PeerMessage::Chunk {
                fingerprint,
                seq: 1,
                bytes: vec![0; 16],
            }
            .encode(),
            7,
        ),
        DownloadStep::FallBack(Fallback::SizeMismatch),
    );
}

#[test]
fn chunks_out_of_sequence_end_the_transfer_instead_of_being_reassembled() {
    // Reassembling a stream whose shape an untrusted party chooses is how a
    // parser differential starts. Strict sequence means the receiver never
    // has to hold a model of what the sender meant.
    let bytes = content(4, CHUNK_BYTES * 2);
    let fingerprint = Fingerprint::of_bytes(&bytes);
    let scope = PlanScope::from_plan(world(), &plan_for(&[&bytes]));
    let (mut download, _) = PeerDownload::begin(scope.request(&fingerprint).unwrap(), 0);

    let frames = serve_frames(&fingerprint, &bytes);
    download.on_frame(&frames[0].encode(), 5);
    // Skip chunk 0 and send chunk 1.
    assert_eq!(
        download.on_frame(&frames[2].encode(), 6),
        DownloadStep::FallBack(Fallback::Protocol),
    );
}

#[test]
fn a_chunk_arriving_before_any_offer_is_refused() {
    // A peer that skips the offer is a peer whose byte count was never
    // checked against the server's, so nothing downstream would bound it.
    let bytes = content(1, 64);
    let fingerprint = Fingerprint::of_bytes(&bytes);
    let scope = PlanScope::from_plan(world(), &plan_for(&[&bytes]));
    let (mut download, _) = PeerDownload::begin(scope.request(&fingerprint).unwrap(), 0);

    assert_eq!(
        download.on_frame(
            &PeerMessage::Chunk {
                fingerprint,
                seq: 0,
                bytes: bytes.clone(),
            }
            .encode(),
            5,
        ),
        DownloadStep::FallBack(Fallback::Protocol),
    );
}

#[test]
fn a_done_before_all_the_offered_bytes_arrived_never_reaches_the_hash_check() {
    // A short transfer would fail verification anyway; ending it here says
    // the length is checked on its own terms, so a future change to the hash
    // step cannot turn a truncated transfer into an accepted one.
    let bytes = content(1, 2048);
    let fingerprint = Fingerprint::of_bytes(&bytes);
    let scope = PlanScope::from_plan(world(), &plan_for(&[&bytes]));
    let (mut download, _) = PeerDownload::begin(scope.request(&fingerprint).unwrap(), 0);

    let frames = serve_frames(&fingerprint, &bytes);
    download.on_frame(&frames[0].encode(), 5);
    assert_eq!(
        download.on_frame(&PeerMessage::Done { fingerprint }.encode(), 6),
        DownloadStep::FallBack(Fallback::SizeMismatch),
    );
}

#[test]
fn a_peer_that_stops_sending_is_abandoned_rather_than_waited_on() {
    // FR-048's teeth. A stalled peer must never be slower than not having
    // used one, and silence is indistinguishable from slowness — so the
    // clock, not the peer, decides when this is over.
    let bytes = content(1, CHUNK_BYTES * 4);
    let fingerprint = Fingerprint::of_bytes(&bytes);
    let scope = PlanScope::from_plan(world(), &plan_for(&[&bytes]));
    let (mut download, _) = PeerDownload::begin(scope.request(&fingerprint).unwrap(), 0);

    let frames = serve_frames(&fingerprint, &bytes);
    download.on_frame(&frames[0].encode(), 0);
    download.on_frame(&frames[1].encode(), 100);

    assert_eq!(download.tick(100 + STALL_MS), DownloadStep::Continue);
    assert_eq!(
        download.tick(100 + STALL_MS + 1),
        DownloadStep::FallBack(Fallback::Stalled),
    );
    assert_eq!(download.received(), 0);
}

#[test]
fn a_peer_making_steady_but_useless_progress_still_hits_the_deadline() {
    // Progress is not the same as usefulness. A peer trickling one chunk
    // every second never trips the stall timer and is still worse than the
    // server, which is the failure the total deadline exists for.
    let bytes = content(1, CHUNK_BYTES * 32);
    let fingerprint = Fingerprint::of_bytes(&bytes);
    let scope = PlanScope::from_plan(world(), &plan_for(&[&bytes]));
    let (mut download, _) = PeerDownload::begin(scope.request(&fingerprint).unwrap(), 0);

    let frames = serve_frames(&fingerprint, &bytes);
    download.on_frame(&frames[0].encode(), 0);
    let mut now = 0;
    let mut ended = false;
    for frame in &frames[1..] {
        now += STALL_MS / 2;
        if download.on_frame(&frame.encode(), now) == DownloadStep::FallBack(Fallback::Stalled) {
            ended = true;
            break;
        }
        if download.tick(now) == DownloadStep::FallBack(Fallback::Stalled) {
            ended = true;
            break;
        }
    }
    assert!(
        ended,
        "a transfer that never finishes must end at the deadline anyway"
    );
}

#[test]
fn only_dishonesty_costs_a_peer_its_trust() {
    // The distinction the fall-backs exist to draw. A peer whose laptop shut
    // is useful again in a moment; a peer that sent the wrong bytes is not
    // useful again at all, and the contract says so: "do not retry that peer".
    for benign in [Fallback::Declined, Fallback::PeerGone, Fallback::Stalled] {
        assert!(!benign.distrusts_peer(), "{benign:?} is not a peer's fault");
    }
    for hostile in [
        Fallback::VerificationFailed,
        Fallback::SizeMismatch,
        Fallback::Protocol,
    ] {
        assert!(
            hostile.distrusts_peer(),
            "{hostile:?} is a peer failing to send what it was asked for",
        );
    }
}

// ---------------------------------------------------------------------------
// T091 — what this client is willing to serve
// ---------------------------------------------------------------------------

#[test]
fn a_peer_asking_for_something_not_held_is_declined() {
    // "DECLINE rather than fabricate." There is no branch that sends bytes
    // for a fingerprint this client cannot produce, because it has none.
    let mut server = PeerServer::new(world());
    let held = content(1, 64);
    server.holds(Fingerprint::of_bytes(&held));

    assert_eq!(
        server.on_request("someone", &Fingerprint::of_bytes(&held), 0),
        ServeDecision::Serve,
    );
    assert_eq!(
        server.on_request("someone", &Fingerprint::of_bytes(b"never seen"), 0),
        ServeDecision::Decline(DeclineReason::NotHeld),
    );
}

#[test]
fn serving_stops_the_instant_world_membership_is_lost() {
    // FR-050. Not at the end of the current transfer, and not at the next
    // sync: `must_abort` is checked between frames, so a large map stops
    // mid-delivery. A client that has lost a world must not finish handing
    // it out.
    let mut server = PeerServer::new(world());
    let held = content(1, 64);
    let fingerprint = Fingerprint::of_bytes(&held);
    server.holds(fingerprint);
    assert!(server.is_serving());
    assert!(!server.must_abort());

    server.membership_lost();

    assert!(!server.is_serving());
    assert!(
        server.must_abort(),
        "a transfer already under way must be abandoned"
    );
    assert_eq!(server.held_count(), 0, "and nothing may remain offerable");
    assert_eq!(
        server.on_request("someone", &fingerprint, 0),
        ServeDecision::Decline(DeclineReason::NotPermitted),
    );
}

#[test]
fn a_client_that_has_lost_a_world_does_not_reveal_what_it_used_to_hold() {
    // The membership check comes before the held-set check for a reason: if
    // it did not, the *shape* of the refusal — NOT_HELD versus NOT_PERMITTED
    // — would tell a former co-player which assets this client still has on
    // disk from a world it was removed from.
    let mut server = PeerServer::new(world());
    let held = content(1, 64);
    server.holds(Fingerprint::of_bytes(&held));
    server.membership_lost();

    assert_eq!(
        server.on_request("someone", &Fingerprint::of_bytes(&held), 0),
        ServeDecision::Decline(DeclineReason::NotPermitted),
    );
    assert_eq!(
        server.on_request("someone", &Fingerprint::of_bytes(b"never held"), 0),
        ServeDecision::Decline(DeclineReason::NotPermitted),
        "both answers must be identical, or the refusal is an oracle",
    );
}

#[test]
fn evicted_content_stops_being_offered() {
    // The held set is a claim about disk. A fingerprint the budget or the
    // eviction pass has removed must stop being servable, or a peer is
    // promised bytes that are not there.
    let mut server = PeerServer::new(world());
    let bytes = content(1, 64);
    let fingerprint = Fingerprint::of_bytes(&bytes);
    server.holds(fingerprint);
    server.served("peer", 64);

    server.forgets(&fingerprint);
    assert_eq!(
        server.on_request("peer", &fingerprint, 0),
        ServeDecision::Decline(DeclineReason::NotHeld),
    );

    // And a whole-set replacement, which is what a sync performs.
    server.holds_only([fingerprint]);
    assert_eq!(
        server.on_request("peer", &fingerprint, 0),
        ServeDecision::Serve
    );
    server.served("peer", 64);
    server.holds_only([]);
    assert_eq!(
        server.on_request("peer", &fingerprint, 0),
        ServeDecision::Decline(DeclineReason::NotHeld),
    );
}

#[test]
fn a_peer_asking_too_often_is_told_busy_and_then_dropped() {
    // "A peer is a participant in a game, not a CDN." The two stages matter:
    // BUSY is the ordinary answer, and dropping the channel is what stops a
    // peer that ignores it — because a DECLINE still costs a read and a write
    // per request, so declining is not itself a limit.
    let mut server = PeerServer::new(world());
    let bytes = content(1, 64);
    let fingerprint = Fingerprint::of_bytes(&bytes);
    server.holds(fingerprint);

    let mut busied = false;
    let mut dropped = false;
    for i in 0..1000 {
        match server.on_request("greedy", &fingerprint, 0) {
            ServeDecision::Serve => {
                // Released immediately so the concurrency cap is not what is
                // being measured here.
                server.served("greedy", 0);
                assert!(
                    i < MAX_REQUESTS_PER_WINDOW as usize,
                    "nothing may be served past the per-window request limit",
                );
            }
            ServeDecision::Decline(DeclineReason::Busy) => busied = true,
            ServeDecision::DropChannel => {
                dropped = true;
                break;
            }
            other => panic!("unexpected {other:?} while flooding"),
        }
    }
    assert!(
        busied,
        "the rate limit must engage before the channel is dropped"
    );
    assert!(
        dropped,
        "a peer that ignores BUSY must eventually be cut off"
    );
}

#[test]
fn the_rate_limit_forgives_a_peer_once_the_window_passes() {
    // A limit that never resets would permanently exile a player who opened
    // a large scene, which turns an optimization into a penalty.
    let mut server = PeerServer::new(world());
    let bytes = content(1, 64);
    let fingerprint = Fingerprint::of_bytes(&bytes);
    server.holds(fingerprint);

    for _ in 0..=MAX_REQUESTS_PER_WINDOW {
        if let ServeDecision::Serve = server.on_request("busy", &fingerprint, 0) {
            server.served("busy", 0);
        }
    }
    assert_eq!(
        server.on_request("busy", &fingerprint, 0),
        ServeDecision::Decline(DeclineReason::Busy),
    );
    assert_eq!(
        server.on_request("busy", &fingerprint, RATE_WINDOW_MS),
        ServeDecision::Serve,
    );
}

#[test]
fn a_peer_that_has_taken_its_share_of_bandwidth_waits() {
    // Requests are cheap; bytes are not. A handful of requests for very large
    // maps is the shape that actually costs an uplink, and a request-count
    // limit alone would not notice it.
    let mut server = PeerServer::new(world());
    let bytes = content(1, 64);
    let fingerprint = Fingerprint::of_bytes(&bytes);
    server.holds(fingerprint);

    assert_eq!(
        server.on_request("hungry", &fingerprint, 0),
        ServeDecision::Serve
    );
    server.served("hungry", MAX_BYTES_PER_WINDOW + 1);
    assert_eq!(
        server.on_request("hungry", &fingerprint, 0),
        ServeDecision::Decline(DeclineReason::Busy),
    );
    assert_eq!(
        server.on_request("hungry", &fingerprint, RATE_WINDOW_MS),
        ServeDecision::Serve,
    );
}

#[test]
fn one_peer_may_not_open_unbounded_concurrent_transfers() {
    // Each in-flight serve is a blob read and a buffer. Without a cap, a peer
    // that never closes a transfer accumulates them.
    let mut server = PeerServer::new(world());
    let bytes = content(1, 64);
    let fingerprint = Fingerprint::of_bytes(&bytes);
    server.holds(fingerprint);

    for _ in 0..MAX_CONCURRENT_SERVES {
        assert_eq!(
            server.on_request("eager", &fingerprint, 0),
            ServeDecision::Serve
        );
    }
    assert_eq!(
        server.on_request("eager", &fingerprint, 0),
        ServeDecision::Decline(DeclineReason::Busy),
    );

    // A finished transfer releases its slot, whether or not it completed —
    // otherwise a peer whose channel dropped mid-serve is BUSY forever.
    server.served("eager", 0);
    assert_eq!(
        server.on_request("eager", &fingerprint, 0),
        ServeDecision::Serve
    );
}

#[test]
fn one_peers_rate_limit_does_not_starve_another() {
    // Accounting is per peer. A shared budget would let one greedy client
    // deny the rest of the table the feature entirely.
    let mut server = PeerServer::new(world());
    let bytes = content(1, 64);
    let fingerprint = Fingerprint::of_bytes(&bytes);
    server.holds(fingerprint);

    for _ in 0..=MAX_REQUESTS_PER_WINDOW {
        if let ServeDecision::Serve = server.on_request("greedy", &fingerprint, 0) {
            server.served("greedy", 0);
        }
    }
    assert_eq!(
        server.on_request("greedy", &fingerprint, 0),
        ServeDecision::Decline(DeclineReason::Busy),
    );
    assert_eq!(
        server.on_request("polite", &fingerprint, 0),
        ServeDecision::Serve,
    );
}

// ---------------------------------------------------------------------------
// What the user is told (FR-049)
// ---------------------------------------------------------------------------

#[test]
fn the_indicator_reports_counters_and_nothing_identifying() {
    // FR-052/FR-054. The panel exists to disclose that peer transfer is
    // happening, not to describe who is in the game — so this payload must
    // stay three numbers, and this test is what makes adding a fourth field
    // with a peer id in it a deliberate act.
    let json = PeerActivity {
        connected_peers: 2,
        bytes_from_peers: 4096,
        verification_failures: 1,
    }
    .to_json();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let object = parsed.as_object().unwrap();

    assert_eq!(object["connectedPeers"], 2);
    assert_eq!(object["bytesFromPeers"], 4096);
    assert_eq!(object["verificationFailures"], 1);
    assert_eq!(
        object.len(),
        3,
        "the indicator payload is counters only; anything else is telemetry",
    );
}
