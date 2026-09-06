//! Spec 028 T098/T100/T101: peer adjudication while server-isolated.
//!
//! Split out of `peer.rs`, which had grown past the file-length limit. The
//! transfer protocol lives there; what a severed table may agree among itself
//! lives here.

// Three rules are on trial here, and each of them stops play more often than
// a looser one would. **Everyone must be reachable**, so a partition leaves
// no half still making progress and there is never a second history to merge.
// **The Game Master specifically**, with no election, so there is one chain of
// authority. **Position, rotation and scale only**, so nothing peers agree
// among themselves can create, delete, or grant anything. What the tests below
// are really defending is the *cost* of those rules being paid — a future
// change that makes play continue in one more case is the failure they exist
// to catch.

use thunderforge_cache_browser::peer::{
    Adjudication, AdjudicationEnd, AdjudicationMessage, AdjudicationStep, Nonce, NonceSequence,
    PeerMessage, Refusal, TokenTransform, Verdict,
};
use thunderforge_cache_core::Fingerprint;
use uuid::Uuid;

fn gm_user() -> Uuid {
    Uuid::from_u128(0xA1)
}

fn player_one() -> Uuid {
    Uuid::from_u128(0xB2)
}

fn player_two() -> Uuid {
    Uuid::from_u128(0xC3)
}

fn token() -> Uuid {
    Uuid::from_u128(0xD4)
}

/// The Game Master's own client, with two players at the table.
fn gm_client() -> Adjudication {
    Adjudication::begin(
        "gm",
        gm_user(),
        gm_user(),
        ["p1".to_string(), "p2".to_string()],
    )
    .expect("a table with two players is a session")
}

/// A player's client, having heard everyone say who they are.
fn player_client(session: &str, user: Uuid, others: [&str; 2]) -> Adjudication {
    let mut client = Adjudication::begin(
        session,
        user,
        gm_user(),
        others.iter().map(|s| (*s).to_string()),
    )
    .expect("a table with two other people is a session");
    // Nobody knows which channel the Game Master is on until they say so —
    // and the claim is only ever believed against the user the *server* named
    // while this client was still connected.
    client.on_message("gm", AdjudicationMessage::Hello { user_id: gm_user() });
    client
}

#[test]
fn a_proposal_can_name_a_position_a_rotation_or_a_scale_and_nothing_else() {
    // FR-060, and the failure it catches is the one that would matter most:
    // a peer-adjudicated *deletion*. There is no field for one, so the check
    // is at the decoder — a frame claiming a field this protocol does not
    // have is not a rejected message, it is not a message.
    let nonce = Nonce {
        seq: 3,
        origin: "p1".into(),
    };
    for transform in [
        TokenTransform::position(12.0, -4.5),
        TokenTransform::rotation(1.5),
        TokenTransform::scale(2.0),
        TokenTransform::position(1.0, 2.0)
            .with_rotation(0.25)
            .with_scale(3.0),
    ] {
        let frame = AdjudicationMessage::Propose {
            nonce: nonce.clone(),
            origin_user: player_one(),
            entity_id: token(),
            transform,
        }
        .encode();
        let decoded = AdjudicationMessage::decode(&frame);
        assert!(
            matches!(decoded, Some(AdjudicationMessage::Propose { transform: t, .. }) if t == transform),
            "a transform in scope must survive the wire unchanged",
        );
    }

    // Now corrupt only the field mask, leaving a frame that is well-formed in
    // every other respect. The mask sits after the tag, the nonce, and the
    // two uuids.
    let mut frame = AdjudicationMessage::Propose {
        nonce,
        origin_user: player_one(),
        entity_id: token(),
        transform: TokenTransform::position(1.0, 2.0),
    }
    .encode();
    let mask_at = 1 + 8 + 1 + "p1".len() + 16 + 16;
    frame[mask_at] = 0b1000;
    assert_eq!(
        AdjudicationMessage::decode(&frame),
        None,
        "a field outside position/rotation/scale must not decode at all",
    );
    frame[mask_at] = 0;
    assert_eq!(
        AdjudicationMessage::decode(&frame),
        None,
        "a proposal proposing nothing is not a proposal",
    );
}

#[test]
fn an_adjudication_frame_is_never_mistaken_for_a_transfer_frame() {
    // The two protocols share one data channel. If a tag collided, a
    // `PROPOSE` could be read as a `CHUNK` and its bytes buffered into
    // somebody's map — so the decoders must each answer `None` for the
    // other's frames, in both directions.
    let adjudication = [
        AdjudicationMessage::Hello { user_id: gm_user() },
        AdjudicationMessage::Apply {
            nonce: Nonce {
                seq: 1,
                origin: "gm".into(),
            },
        },
        AdjudicationMessage::Adjudicate {
            nonce: Nonce {
                seq: 1,
                origin: "gm".into(),
            },
            verdict: Verdict::Reject,
        },
    ];
    for message in adjudication {
        let frame = message.encode();
        assert_eq!(
            PeerMessage::decode(&frame),
            None,
            "{message:?} is not transfer"
        );
        assert_eq!(
            AdjudicationMessage::decode(&frame),
            Some(message.clone()),
            "{message:?} must survive its own decoder",
        );
    }
    let transfer = PeerMessage::Request {
        fingerprint: Fingerprint::of_bytes(b"map"),
    };
    assert_eq!(
        AdjudicationMessage::decode(&transfer.encode()),
        None,
        "a transfer request is not an adjudication message",
    );
}

#[test]
fn a_frame_carries_no_clock_for_a_skewed_one_to_poison() {
    // The reason ordering is a nonce and not a timestamp: a client's clock is
    // chosen by that client. This is the structural half of that argument —
    // encoding the same proposal at two different wall-clock moments produces
    // identical bytes, so there is nothing in a frame for a skewed or lying
    // clock to influence.
    let message = AdjudicationMessage::Propose {
        nonce: Nonce {
            seq: 7,
            origin: "p1".into(),
        },
        origin_user: player_one(),
        entity_id: token(),
        transform: TokenTransform::position(3.0, 4.0),
    };
    let first = message.encode();
    std::thread::sleep(std::time::Duration::from_millis(2));
    assert_eq!(
        first,
        message.encode(),
        "a frame must be a function of its content"
    );
}

#[test]
fn a_nonce_is_raised_past_everything_seen_so_a_reply_is_ordered_after_it() {
    // Ordering has to be causal without a clock: a proposal made *after*
    // seeing someone else's must sort after it, on every client, computed
    // from the messages alone.
    let mut sequence = NonceSequence::new("p1");
    let first = sequence.issue();
    sequence.observe(&Nonce {
        seq: 40,
        origin: "gm".into(),
    });
    let reply = sequence.issue();
    assert!(first.seq < 40, "the first nonce predates what was seen");
    assert!(
        reply
            > Nonce {
                seq: 40,
                origin: "gm".into()
            },
        "a nonce issued after seeing 40 must sort after it",
    );
    // Two clients at the same logical instant are broken apart by session id
    // rather than left equal, or one of the two would be silently discarded.
    assert!(
        Nonce {
            seq: 9,
            origin: "a".into()
        } < Nonce {
            seq: 9,
            origin: "b".into()
        },
    );
}

#[test]
fn order_follows_the_nonce_and_not_the_order_frames_happen_to_arrive() {
    // Two people move the same token while server-isolated. The one that is
    // *ordered* later must win on every client, whatever the network did to
    // the frames — otherwise a slow link decides the outcome, which is the
    // conflict-by-timestamp failure wearing different clothes.
    let mut client = player_client("p2", player_two(), ["gm", "p1"]);
    client.on_message(
        "p1",
        AdjudicationMessage::Hello {
            user_id: player_one(),
        },
    );

    let early = Nonce {
        seq: 4,
        origin: "p1".into(),
    };
    let late = Nonce {
        seq: 9,
        origin: "gm".into(),
    };
    client.on_message(
        "p1",
        AdjudicationMessage::Propose {
            nonce: early.clone(),
            origin_user: player_one(),
            entity_id: token(),
            transform: TokenTransform::position(1.0, 1.0),
        },
    );
    client.on_message(
        "gm",
        AdjudicationMessage::Propose {
            nonce: late.clone(),
            origin_user: gm_user(),
            entity_id: token(),
            transform: TokenTransform::position(9.0, 9.0),
        },
    );

    // The later-ordered apply arrives first.
    let applied = client.on_message("gm", AdjudicationMessage::Apply { nonce: late });
    assert!(
        matches!(&applied, AdjudicationStep::Applied(change)
            if change.transform.position_of() == Some([9.0, 9.0])),
        "the later nonce applies: {applied:?}",
    );
    let stale = client.on_message("gm", AdjudicationMessage::Apply { nonce: early });
    assert_eq!(
        stale,
        AdjudicationStep::Refused(Refusal::Superseded),
        "an earlier nonce arriving later must not overwrite it",
    );
}

#[test]
fn losing_any_participant_ends_adjudicated_play_at_once() {
    // FR-058. Not "most of them", not "after a grace period": one person's
    // laptop closing ends adjudicated play immediately, because a window in
    // which play continues without them is a window in which two groups can
    // both make progress.
    let mut gm = gm_client();
    assert!(gm.is_adjudicating(), "a full table adjudicates");
    assert_eq!(gm.peer_lost("p2"), Some(AdjudicationEnd::PeerLost));
    assert!(
        !gm.is_adjudicating(),
        "losing one player must stop play, not continue with a quorum",
    );
    assert_eq!(
        gm.propose(token(), TokenTransform::position(1.0, 1.0)),
        AdjudicationStep::Refused(Refusal::NotAdjudicating),
        "and nothing may be proposed afterwards",
    );
}

#[test]
fn losing_the_game_master_stops_play_rather_than_electing_a_replacement() {
    // FR-059. The tempting alternative — promote whoever is left — would put
    // two adjudicators in one session the moment the Game Master's connection
    // flickers back, so there is deliberately no election here to test.
    let mut player = player_client("p1", player_one(), ["gm", "p2"]);
    assert!(player.is_adjudicating());
    assert_eq!(
        player.peer_lost("gm"),
        Some(AdjudicationEnd::GameMasterLost)
    );
    assert!(!player.is_adjudicating());
    assert_eq!(
        player.gm_session(),
        None,
        "nobody is promoted into the empty role",
    );
    assert_eq!(
        player.propose(token(), TokenTransform::rotation(0.5)),
        AdjudicationStep::Refused(Refusal::NotAdjudicating),
    );
}

#[test]
fn both_halves_of_a_partition_stop_and_neither_wins() {
    // The split-brain case the full-connectivity rule exists for. A quorum
    // would let the larger half carry on and produce a history the smaller
    // half never saw; the test that matters is that *both* stop.
    let mut gm = gm_client();
    let mut player = player_client("p1", player_one(), ["gm", "p2"]);
    assert!(gm.is_adjudicating() && player.is_adjudicating());

    // The network splits {gm, p2} from {p1}.
    gm.peer_lost("p1");
    player.peer_lost("gm");
    player.peer_lost("p2");

    assert!(!gm.is_adjudicating(), "the larger half stops too");
    assert!(!player.is_adjudicating(), "and so does the smaller one");
    assert_eq!(gm.ended(), Some(AdjudicationEnd::PeerLost));
    assert_eq!(player.ended(), Some(AdjudicationEnd::GameMasterLost));
}

#[test]
fn only_the_game_masters_client_may_adjudicate() {
    // FR-059 on the receiving side: a peer that decides to start issuing
    // verdicts is ignored, whether or not the Game Master is still here.
    let mut player = player_client("p1", player_one(), ["gm", "p2"]);
    let nonce = Nonce {
        seq: 2,
        origin: "p2".into(),
    };
    player.on_message(
        "p2",
        AdjudicationMessage::Hello {
            user_id: player_two(),
        },
    );
    player.on_message(
        "p2",
        AdjudicationMessage::Propose {
            nonce: nonce.clone(),
            origin_user: player_two(),
            entity_id: token(),
            transform: TokenTransform::scale(2.0),
        },
    );
    assert_eq!(
        player.on_message(
            "p2",
            AdjudicationMessage::Adjudicate {
                nonce: nonce.clone(),
                verdict: Verdict::Accept
            }
        ),
        AdjudicationStep::Refused(Refusal::NotTheGameMaster),
    );
    assert_eq!(
        player.on_message("p2", AdjudicationMessage::Apply { nonce }),
        AdjudicationStep::Refused(Refusal::NotTheGameMaster),
        "and an APPLY from a peer applies nothing",
    );
}

#[test]
fn a_player_may_not_propose_on_someone_elses_behalf_but_the_game_master_may() {
    // The peer-side mirror of FR-061a and FR-061b together. The negative half
    // is the one worth guarding: a player attributing a move to someone else.
    // The positive half is deliberate — a Game Master acting for a player is
    // table authority, not an attack, and the software does not police it.
    let mut client = player_client("p2", player_two(), ["gm", "p1"]);
    client.on_message(
        "p1",
        AdjudicationMessage::Hello {
            user_id: player_one(),
        },
    );

    let forged = client.on_message(
        "p1",
        AdjudicationMessage::Propose {
            nonce: Nonce {
                seq: 1,
                origin: "p1".into(),
            },
            origin_user: player_two(),
            entity_id: token(),
            transform: TokenTransform::position(5.0, 5.0),
        },
    );
    assert_eq!(forged, AdjudicationStep::Refused(Refusal::NotYours));

    let on_behalf = client.on_message(
        "gm",
        AdjudicationMessage::Propose {
            nonce: Nonce {
                seq: 2,
                origin: "gm".into(),
            },
            origin_user: player_one(),
            entity_id: token(),
            transform: TokenTransform::position(6.0, 6.0),
        },
    );
    assert_eq!(
        on_behalf,
        AdjudicationStep::Ignore,
        "the Game Master may speak for a player; the proposal is held for the verdict",
    );
}

#[test]
fn a_nonce_borrowed_from_another_session_is_refused() {
    // Ordering is only meaningful while each client owns its own sequence. A
    // peer issuing nonces in someone else's name could order its proposals
    // ahead of theirs at will.
    let mut client = player_client("p2", player_two(), ["gm", "p1"]);
    client.on_message(
        "p1",
        AdjudicationMessage::Hello {
            user_id: player_one(),
        },
    );
    let step = client.on_message(
        "p1",
        AdjudicationMessage::Propose {
            nonce: Nonce {
                seq: 1,
                origin: "gm".into(),
            },
            origin_user: player_one(),
            entity_id: token(),
            transform: TokenTransform::scale(1.5),
        },
    );
    assert_eq!(step, AdjudicationStep::Refused(Refusal::NotYours));
}

#[test]
fn the_game_master_decides_its_own_move_and_tells_everyone() {
    // The Game Master both proposes and adjudicates, and the frames it emits
    // must be the same ones a player's proposal would have produced — or the
    // other clients would need two code paths and one of them would rot.
    let mut gm = gm_client();
    let step = gm.propose(token(), TokenTransform::position(2.0, 3.0));
    let AdjudicationStep::Broadcast { frames, applied } = step else {
        panic!("the Game Master's own move is broadcast and applied");
    };
    assert_eq!(frames.len(), 3, "PROPOSE, ADJUDICATE, APPLY");
    assert!(matches!(
        AdjudicationMessage::decode(&frames[0]),
        Some(AdjudicationMessage::Propose { .. })
    ));
    assert!(matches!(
        AdjudicationMessage::decode(&frames[1]),
        Some(AdjudicationMessage::Adjudicate {
            verdict: Verdict::Accept,
            ..
        })
    ));
    assert!(matches!(
        AdjudicationMessage::decode(&frames[2]),
        Some(AdjudicationMessage::Apply { .. })
    ));
    let applied = applied.expect("the Game Master applies its own move locally");
    assert_eq!(applied.origin_user, gm_user());
    assert_eq!(applied.transform.position_of(), Some([2.0, 3.0]));
}

#[test]
fn everything_adjudicated_is_kept_for_the_server_to_confirm() {
    // Adjudication is provisional (FR-062). The failure this catches is a
    // change that was applied on screens and then quietly forgotten when play
    // stopped — the server would never see it, and the person who made it
    // would find their work gone with no explanation.
    let mut gm = gm_client();
    gm.propose(token(), TokenTransform::position(1.0, 1.0));
    gm.propose(Uuid::from_u128(0xE5), TokenTransform::rotation(0.5));
    assert_eq!(gm.submissions().len(), 2);

    assert_eq!(gm.server_returned(), Some(AdjudicationEnd::ServerReturned));
    assert!(!gm.is_adjudicating(), "the server is the arbiter again");
    assert_eq!(
        gm.submissions().len(),
        2,
        "what was applied still owes the server a submission",
    );
    let payload = gm.submissions_json();
    assert!(payload.contains(&token().to_string()));
    assert_eq!(gm.take_submissions().len(), 2);
    assert!(gm.submissions().is_empty(), "and is not submitted twice",);
}

#[test]
fn a_client_with_nobody_to_ask_is_offline_rather_than_server_isolated() {
    // Server-isolated means "the server is gone and the table is here". One
    // person alone is the ordinary offline case, which the outbox already
    // handles — and a session of one that called itself server-isolated would
    // be a client adjudicating for itself.
    assert!(
        Adjudication::begin("gm", gm_user(), gm_user(), Vec::<String>::new()).is_none(),
        "there is no adjudicated play with nobody to adjudicate among",
    );
}

#[test]
fn play_waits_until_the_game_master_is_known_to_be_here() {
    // A player's client cannot tell which channel belongs to the Game Master
    // until somebody says so. Until then the third condition is unmet and
    // nothing is adjudicated — the safe direction, because the alternative is
    // a table playing on with no arbiter present.
    let mut player = Adjudication::begin(
        "p1",
        player_one(),
        gm_user(),
        ["gm".to_string(), "p2".to_string()],
    )
    .expect("two others is a session");
    assert!(
        !player.is_adjudicating(),
        "everyone reachable is not enough; the Game Master must be among them",
    );
    player.on_message(
        "p2",
        AdjudicationMessage::Hello {
            user_id: player_two(),
        },
    );
    assert!(
        !player.is_adjudicating(),
        "another player is not the Game Master"
    );
    player.on_message("gm", AdjudicationMessage::Hello { user_id: gm_user() });
    assert!(player.is_adjudicating());
}

/// Two clients do not begin adjudicating at the same instant, and the
/// greeting must survive that.
///
/// `HELLO` is broadcast once, at `begin`. Whoever begins first announces
/// itself to peers that have no adjudication yet, and the frame is simply
/// dropped. Before this was answered in kind the pair ended up in a stable
/// asymmetry — measured in the browser with both clients severed and their
/// data channel open, the Game Master sat at `server-isolated` while the
/// player sat at `reconnecting` forever, because a player's client will not
/// adjudicate until it knows which session the Game Master is speaking on.
#[test]
fn a_client_that_began_first_still_learns_who_the_game_master_is() {
    let gm_user = Uuid::from_u128(0xC3);
    let player_user = Uuid::from_u128(0xD4);

    // The Game Master begins first. Its greeting goes out to a player that
    // is not adjudicating yet, so nothing here receives it.
    let mut gm = Adjudication::begin("gm-session", gm_user, gm_user, vec!["p-session".into()])
        .expect("a table of two adjudicates");

    // The player begins second and greets. The Game Master learns the
    // player, and — the fix — says who it is in return.
    let mut player =
        Adjudication::begin("p-session", player_user, gm_user, vec!["gm-session".into()])
            .expect("a table of two adjudicates");
    assert!(
        !player.is_adjudicating(),
        "precondition: a player that has not heard from the Game Master must not adjudicate",
    );

    let answer = gm.on_message(
        "p-session",
        AdjudicationMessage::Hello {
            user_id: player_user,
        },
    );
    let frames = match answer {
        AdjudicationStep::Broadcast { frames, .. } => frames,
        other => panic!("a new greeting must be answered, got {other:?}"),
    };

    for frame in &frames {
        let message = AdjudicationMessage::decode(frame).expect("a greeting must decode");
        player.on_message("gm-session", message);
    }

    assert!(
        player.is_adjudicating(),
        "the player must be adjudicating once the Game Master has answered",
    );

    // And it stops there: a greeting that teaches nothing new is not
    // answered, so two clients cannot greet each other forever.
    assert!(
        matches!(
            gm.on_message(
                "p-session",
                AdjudicationMessage::Hello {
                    user_id: player_user
                },
            ),
            AdjudicationStep::Ignore
        ),
        "a repeated greeting must not be answered, or the two never stop",
    );
}
