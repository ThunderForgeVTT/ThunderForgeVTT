//! Signalling, connection setup, and the two directions bytes move in.
//!
//! Split from `wasm.rs` for file length; the state it works on lives there.

use super::*;

// -----------------------------------------------------------------
// Signaling
// -----------------------------------------------------------------

fn signal(to: &str, payload: serde_json::Value) {
    FABRIC.with(|slot| {
        if let Some(fabric) = slot.borrow().as_ref() {
            let _ = fabric.send_signal.call2(
                &JsValue::NULL,
                &JsValue::from_str(to),
                &JsValue::from_str(&payload.to_string()),
            );
        }
    });
}

/// Offer a connection to one peer.
///
/// **The newcomer always initiates.** A client joining queries the roster
/// and offers to each name on it; nobody offers to a newcomer. That makes
/// glare — two peers offering each other at once — structurally
/// impossible rather than something to resolve, which is worth more than
/// the connection it occasionally costs when two clients join together.
pub async fn connect_to(peer: String) {
    let already = FABRIC.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|f| f.links.contains_key(&peer))
    });
    if already {
        return;
    }
    let Some(link) = new_link(&peer, true) else {
        return;
    };

    let offer = match JsFuture::from(link.connection.create_offer()).await {
        Ok(offer) => offer,
        Err(_) => return,
    };
    let Some(sdp) = sdp_of(&offer) else { return };
    let init = web_sys::RtcSessionDescriptionInit::new(web_sys::RtcSdpType::Offer);
    init.set_sdp(&sdp);
    if JsFuture::from(link.connection.set_local_description(&init))
        .await
        .is_err()
    {
        return;
    }
    signal(&peer, serde_json::json!({ "kind": "offer", "sdp": sdp }));
}

/// A signal arrived for us, relayed by the server.
///
/// The server never interprets these and neither does anything above this
/// function: an unparseable payload is dropped in silence, exactly as an
/// unparseable frame is, because the sender is not trusted and a malformed
/// message is not an error condition.
pub async fn on_signal(from: String, payload: String) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&payload) else {
        return;
    };
    match value.get("kind").and_then(serde_json::Value::as_str) {
        Some("offer") => {
            let Some(sdp) = value.get("sdp").and_then(serde_json::Value::as_str) else {
                return;
            };
            // The answerer never opens a channel of its own; it waits for
            // `ondatachannel`. Two channels on one connection would each
            // work and would double every count the indicator shows.
            let Some(link) = new_link(&from, false) else {
                return;
            };
            let remote = web_sys::RtcSessionDescriptionInit::new(web_sys::RtcSdpType::Offer);
            remote.set_sdp(sdp);
            if JsFuture::from(link.connection.set_remote_description(&remote))
                .await
                .is_err()
            {
                return;
            }
            let Ok(answer) = JsFuture::from(link.connection.create_answer()).await else {
                return;
            };
            let Some(answer_sdp) = sdp_of(&answer) else {
                return;
            };
            let local = web_sys::RtcSessionDescriptionInit::new(web_sys::RtcSdpType::Answer);
            local.set_sdp(&answer_sdp);
            if JsFuture::from(link.connection.set_local_description(&local))
                .await
                .is_err()
            {
                return;
            }
            signal(
                &from,
                serde_json::json!({ "kind": "answer", "sdp": answer_sdp }),
            );
        }
        Some("answer") => {
            let (Some(link), Some(sdp)) = (
                link_for(&from),
                value.get("sdp").and_then(serde_json::Value::as_str),
            ) else {
                return;
            };
            let remote = web_sys::RtcSessionDescriptionInit::new(web_sys::RtcSdpType::Answer);
            remote.set_sdp(sdp);
            let _ = JsFuture::from(link.connection.set_remote_description(&remote)).await;
        }
        Some("candidate") => {
            let (Some(link), Some(candidate)) = (
                link_for(&from),
                value.get("candidate").and_then(serde_json::Value::as_str),
            ) else {
                return;
            };
            let init = web_sys::RtcIceCandidateInit::new(candidate);
            init.set_sdp_mid(value.get("sdpMid").and_then(serde_json::Value::as_str));
            init.set_sdp_m_line_index(
                value
                    .get("sdpMLineIndex")
                    .and_then(serde_json::Value::as_u64)
                    .map(|i| i as u16),
            );
            if let Ok(candidate) = web_sys::RtcIceCandidate::new(&init) {
                let _ = JsFuture::from(
                    link.connection
                        .add_ice_candidate_with_opt_rtc_ice_candidate(Some(&candidate)),
                )
                .await;
            }
        }
        _ => {}
    }
}

fn sdp_of(description: &JsValue) -> Option<String> {
    Reflect::get(description, &JsValue::from_str("sdp"))
        .ok()?
        .as_string()
}

fn link_for(peer: &str) -> Option<Rc<PeerLink>> {
    FABRIC.with(|slot| slot.borrow().as_ref()?.links.get(peer).cloned())
}

fn new_link(peer: &str, initiator: bool) -> Option<Rc<PeerLink>> {
    let connection = web_sys::RtcPeerConnection::new().ok()?;
    let link = Rc::new(PeerLink {
        session: peer.to_string(),
        connection,
        channel: RefCell::new(None),
        download: RefCell::new(None),
        waiter: RefCell::new(None),
        outcome: RefCell::new(None),
        ticker: RefCell::new(None),
        retained: RefCell::new(Vec::new()),
    });

    let to = peer.to_string();
    let on_ice = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
        let Ok(candidate) = Reflect::get(&event, &JsValue::from_str("candidate")) else {
            return;
        };
        // A null candidate is "gathering finished", not a candidate.
        if candidate.is_null() || candidate.is_undefined() {
            return;
        }
        let text = Reflect::get(&candidate, &JsValue::from_str("candidate"))
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        let mid = Reflect::get(&candidate, &JsValue::from_str("sdpMid"))
            .ok()
            .and_then(|v| v.as_string());
        let index = Reflect::get(&candidate, &JsValue::from_str("sdpMLineIndex"))
            .ok()
            .and_then(|v| v.as_f64());
        signal(
            &to,
            serde_json::json!({
                "kind": "candidate",
                "candidate": text,
                "sdpMid": mid,
                "sdpMLineIndex": index,
            }),
        );
    });
    link.connection
        .set_onicecandidate(Some(on_ice.as_ref().unchecked_ref()));
    link.retained.borrow_mut().push(on_ice);

    if initiator {
        let channel = link.connection.create_data_channel(CHANNEL_LABEL);
        attach_channel(&link, channel);
    } else {
        let waiting = link.clone();
        let on_channel = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
            let Ok(channel) = Reflect::get(&event, &JsValue::from_str("channel")) else {
                return;
            };
            if let Ok(channel) = channel.dyn_into::<web_sys::RtcDataChannel>() {
                attach_channel(&waiting, channel);
            }
        });
        link.connection
            .set_ondatachannel(Some(on_channel.as_ref().unchecked_ref()));
        link.retained.borrow_mut().push(on_channel);
    }

    FABRIC.with(|slot| {
        if let Some(fabric) = slot.borrow_mut().as_mut() {
            fabric.links.insert(peer.to_string(), link.clone());
        }
    });
    Some(link)
}

fn attach_channel(link: &Rc<PeerLink>, channel: web_sys::RtcDataChannel) {
    channel.set_binary_type(web_sys::RtcDataChannelType::Arraybuffer);

    let receiving = link.clone();
    let on_message = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
        let Ok(data) = Reflect::get(&event, &JsValue::from_str("data")) else {
            return;
        };
        if !data.is_object() {
            return;
        }
        let frame = Uint8Array::new(&data).to_vec();
        on_frame(&receiving, &frame);
    });
    channel.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

    // Departures are noticed here and nowhere else — there is no
    // join/leave push in the signaling contract, by design.
    let closing = link.clone();
    let on_close = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
        peer_departed(&closing);
    });
    channel.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    let erroring = link.clone();
    let on_error = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
        peer_departed(&erroring);
    });
    channel.set_onerror(Some(on_error.as_ref().unchecked_ref()));

    link.retained.borrow_mut().push(on_message);
    link.retained.borrow_mut().push(on_close);
    link.retained.borrow_mut().push(on_error);
    *link.channel.borrow_mut() = Some(channel);
}

/// A peer went away. Any transfer in progress is abandoned whole.
fn peer_departed(link: &Rc<PeerLink>) {
    let step = link
        .download
        .borrow_mut()
        .as_mut()
        .map(PeerDownload::peer_gone);
    if let Some(DownloadStep::FallBack(reason)) = step {
        settle(link, None, Some(reason));
    }
    FABRIC.with(|slot| {
        if let Some(fabric) = slot.borrow_mut().as_mut() {
            fabric.server.peer_gone(&link.session);
            fabric.links.remove(&link.session);
            // FR-058/FR-059, and this is the only place a departure is
            // noticed: there is no join/leave push in the signaling
            // contract, so a closed channel *is* the event. Ending here
            // rather than on a timer is what makes "immediately" true.
            if let Some(adjudication) = fabric.adjudication.as_mut() {
                adjudication.peer_lost(&link.session);
            }
        }
    });
}

fn on_frame(link: &Rc<PeerLink>, frame: &[u8]) {
    // The two protocols share one channel and share no tag, so this is a
    // sort rather than a guess: an adjudication frame cannot decode as a
    // transfer frame, and a transfer frame cannot decode as an
    // adjudication one.
    if let Some(message) = AdjudicationMessage::decode(frame) {
        let step = FABRIC.with(|slot| {
            slot.borrow_mut()
                .as_mut()
                .and_then(|fabric| fabric.adjudication.as_mut())
                .map(|adjudication| adjudication.on_message(&link.session, message))
        });
        if let Some(step) = step {
            handle_adjudication(step);
        }
        return;
    }

    let Some(message) = PeerMessage::decode(frame) else {
        return;
    };

    // A request is the serving side and has nothing to do with any
    // download in flight; keeping them apart here is what stops a peer
    // from steering our own transfer by answering it with a question.
    if let PeerMessage::Request { fingerprint } = message {
        let serving = link.clone();
        wasm_bindgen_futures::spawn_local(async move { serve(serving, fingerprint).await });
        return;
    }

    let step = link
        .download
        .borrow_mut()
        .as_mut()
        .map_or(DownloadStep::Ignore, |download| {
            download.on_message(message, now_ms())
        });

    match step {
        DownloadStep::Verified(bytes) => settle(link, Some(bytes), None),
        DownloadStep::FallBack(reason) => settle(link, None, Some(reason)),
        DownloadStep::Continue | DownloadStep::Ignore => {}
    }
}

/// End a transfer: record it, hand the result to whoever is awaiting.
///
/// `bytes` is `Some` only for [`DownloadStep::Verified`], so this is the
/// last place the "no unverified bytes" property has to hold and the only
/// way any bytes reach a caller.
fn settle(link: &Rc<PeerLink>, bytes: Option<Vec<u8>>, reason: Option<Fallback>) {
    link.download.borrow_mut().take();
    link.stop_ticking();

    FABRIC.with(|slot| {
        if let Some(fabric) = slot.borrow_mut().as_mut() {
            if let Some(reason) = reason {
                fabric.trust.record(&link.session, reason);
                if reason == Fallback::VerificationFailed {
                    fabric.activity.verification_failures =
                        fabric.activity.verification_failures.saturating_add(1);
                }
            }
            if let Some(bytes) = bytes.as_ref() {
                fabric.activity.bytes_from_peers = fabric
                    .activity
                    .bytes_from_peers
                    .saturating_add(bytes.len() as u64);
            }
        }
    });

    // A peer that failed verification is not asked again this session,
    // and the channel goes with it: there is nothing else we want from
    // someone who does not send what they were asked for.
    if reason.is_some_and(Fallback::distrusts_peer) {
        link.shut_down();
        FABRIC.with(|slot| {
            if let Some(fabric) = slot.borrow_mut().as_mut() {
                fabric.links.remove(&link.session);
            }
        });
    }

    *link.outcome.borrow_mut() = bytes;
    let waiter = link.waiter.borrow_mut().take();
    if let Some(waiter) = waiter {
        let _ = waiter.call0(&JsValue::NULL);
    }
}

// -----------------------------------------------------------------
// The requester
// -----------------------------------------------------------------

/// Try to get one fingerprint from a peer.
///
/// `None` means "ask the server", and it is the answer to every one of:
/// peer transfer is off, the fingerprint is not in this client's plan, no
/// peer is connected, every peer is busy or distrusted, the peer declined,
/// the peer stalled, the peer hung up, the peer sent something that did
/// not verify. **The caller cannot tell those apart, and must not need
/// to** — that indistinguishability is SC-013.
pub async fn try_fetch(fingerprint: Fingerprint) -> Option<Vec<u8>> {
    // T089's gate. `request` is the only constructor of a `PeerRequest`
    // anywhere, and it answers `None` for anything the server's plan does
    // not list.
    let request = FABRIC.with(|slot| {
        let borrowed = slot.borrow();
        let fabric = borrowed.as_ref()?;
        fabric.scope.request(&fingerprint)
    })?;

    let link = pick_peer()?;
    let (download, frame) = PeerDownload::begin(request, now_ms());
    *link.download.borrow_mut() = Some(download);
    *link.outcome.borrow_mut() = None;

    // The promise is armed *before* the request goes out, so a peer that
    // answers synchronously cannot resolve into a slot that is not there.
    let waiting = link.clone();
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        *waiting.waiter.borrow_mut() = Some(resolve);
    });
    link.start_ticking();

    if link.send(&frame).is_err() {
        settle(&link, None, Some(Fallback::PeerGone));
        return None;
    }

    let _ = JsFuture::from(promise).await;
    link.outcome.borrow_mut().take()
}

/// The peer to ask next.
///
/// Trusted, connected, and not already carrying a transfer. Round-robin
/// would be better with many peers and is not worth the state at the
/// scale this runs at — a table is a handful of people, not a swarm.
fn pick_peer() -> Option<Rc<PeerLink>> {
    FABRIC.with(|slot| {
        let borrowed = slot.borrow();
        let fabric = borrowed.as_ref()?;
        fabric
            .links
            .values()
            .find(|link| {
                link.is_open()
                    && link.download.borrow().is_none()
                    && fabric.trust.trusts(&link.session)
            })
            .cloned()
    })
}

// -----------------------------------------------------------------
// The server
// -----------------------------------------------------------------

async fn serve(link: Rc<PeerLink>, fingerprint: Fingerprint) {
    let decision = FABRIC.with(|slot| {
        slot.borrow_mut().as_mut().map(|fabric| {
            fabric
                .server
                .on_request(&link.session, &fingerprint, now_ms())
        })
    });

    let decision = match decision {
        Some(decision) => decision,
        // Peer transfer was switched off between the request arriving and
        // this task running. Say nothing and close: an answer would be a
        // statement about content we are no longer entitled to discuss.
        None => {
            link.shut_down();
            return;
        }
    };

    match decision {
        ServeDecision::DropChannel => {
            link.shut_down();
            return;
        }
        ServeDecision::Decline(reason) => {
            link.decline(fingerprint, reason);
            return;
        }
        ServeDecision::Serve => {}
    }

    let provider = FABRIC.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(|fabric| fabric.provider.clone())
    });
    let bytes = match provider {
        Some(provider) => provider(fingerprint).await,
        None => None,
    };

    // Held a moment ago, unreadable now — evicted underneath us, or the
    // key is gone. `DECLINE` rather than send something else: the
    // contract forbids sending bytes that do not hash to what was asked
    // for, and this is the branch where that temptation would live.
    let Some(bytes) = bytes else {
        FABRIC.with(|slot| {
            if let Some(fabric) = slot.borrow_mut().as_mut() {
                fabric.server.served(&link.session, 0);
                fabric.server.forgets(&fingerprint);
            }
        });
        link.decline(fingerprint, DeclineReason::NotHeld);
        return;
    };

    // Belt and braces over `read_blob`, which has already verified. The
    // cost is one hash of bytes we are about to spend far more bandwidth
    // on, and it makes "never send bytes that do not hash to the
    // requested fingerprint" true of this function in isolation rather
    // than true of a chain of callers.
    if fingerprint::verify(&bytes, &fingerprint).is_err() {
        FABRIC.with(|slot| {
            if let Some(fabric) = slot.borrow_mut().as_mut() {
                fabric.server.served(&link.session, 0);
                fabric.server.forgets(&fingerprint);
            }
        });
        link.decline(fingerprint, DeclineReason::NotHeld);
        return;
    }

    let mut sent = 0u64;
    for frame in serve_frames(&fingerprint, &bytes) {
        // Checked between every frame, not once at the start. FR-050
        // says serving stops on losing membership, and a large map is
        // seconds of frames — stopping only at the end of one would mean
        // finishing the delivery of content we have just lost.
        let stop = FABRIC.with(|slot| {
            slot.borrow()
                .as_ref()
                .is_none_or(|fabric| fabric.server.must_abort())
        });
        if stop {
            break;
        }
        let encoded = frame.encode();
        if link.send(&encoded).is_err() {
            break;
        }
        sent = sent.saturating_add(encoded.len() as u64);
    }

    FABRIC.with(|slot| {
        if let Some(fabric) = slot.borrow_mut().as_mut() {
            fabric.server.served(&link.session, sent);
        }
    });
}

impl PeerLink {
    pub(super) fn is_open(&self) -> bool {
        self.channel
            .borrow()
            .as_ref()
            .is_some_and(|channel| channel.ready_state() == web_sys::RtcDataChannelState::Open)
    }

    pub(super) fn send(&self, frame: &[u8]) -> Result<(), ()> {
        let channel = self.channel.borrow();
        let Some(channel) = channel.as_ref() else {
            return Err(());
        };
        if channel.ready_state() != web_sys::RtcDataChannelState::Open {
            return Err(());
        }
        channel.send_with_u8_array(frame).map_err(|_| ())
    }

    fn decline(&self, fingerprint: Fingerprint, reason: DeclineReason) {
        let _ = self.send(
            &PeerMessage::Decline {
                fingerprint,
                reason,
            }
            .encode(),
        );
    }

    /// Poll the download's own deadlines. The timer only exists while a
    /// transfer does, so an idle page schedules nothing.
    fn start_ticking(self: &Rc<Self>) {
        self.stop_ticking();
        let Some(set_interval) = global_fn("setInterval") else {
            return;
        };
        let ticking = self.clone();
        let tick = Closure::<dyn FnMut(JsValue)>::new(move |_: JsValue| {
            let step = ticking
                .download
                .borrow_mut()
                .as_mut()
                .map(|download| download.tick(now_ms()));
            if let Some(DownloadStep::FallBack(reason)) = step {
                settle(&ticking, None, Some(reason));
            }
        });
        let handle = set_interval.call2(
            &JsValue::NULL,
            tick.as_ref().unchecked_ref(),
            &JsValue::from_f64(f64::from(TICK_MS)),
        );
        if let Ok(handle) = handle
            && let Some(handle) = handle.as_f64()
        {
            *self.ticker.borrow_mut() = Some(handle as i32);
        }
        self.retained.borrow_mut().push(tick);
    }

    fn stop_ticking(&self) {
        if let Some(handle) = self.ticker.borrow_mut().take()
            && let Some(clear) = global_fn("clearInterval")
        {
            let _ = clear.call1(&JsValue::NULL, &JsValue::from_f64(f64::from(handle)));
        }
    }

    pub(super) fn shut_down(&self) {
        self.stop_ticking();
        if let Some(channel) = self.channel.borrow_mut().take() {
            channel.set_onmessage(None);
            channel.set_onclose(None);
            channel.set_onerror(None);
            channel.close();
        }
        self.connection.set_onicecandidate(None);
        self.connection.set_ondatachannel(None);
        self.connection.close();
    }
}

impl Fabric {
    /// This client's own session id, for the roster query.
    #[allow(dead_code)]
    fn session(&self) -> &str {
        &self.session_id
    }
}
