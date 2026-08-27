//! The session key: non-extractable AES-GCM, held in IndexedDB (T025).
//!
//! Spec 028 FR-016/FR-016a/FR-016c, research.md R4.
//!
//! # Non-extractable is the whole design
//!
//! The key is generated with `extractable: false` and never exists as bytes
//! anywhere JavaScript can reach. It is stored as a `CryptoKey` *object* —
//! browsers structured-clone `CryptoKey` into IndexedDB precisely so this
//! pattern is available — so what persists is a handle to key material the
//! browser holds, not the material.
//!
//! That single property is what makes storing a key in IndexedDB defensible
//! at all. An XSS payload running with full page privileges can read the
//! record, hand it to `encrypt` and `decrypt`, and thereby read this cache
//! while it is running; what it cannot do is *exfiltrate* the key, so it
//! cannot read cached bytes copied off the machine, and its access ends when
//! the page does. An extractable key, or one parked in `sessionStorage`,
//! would give away the content of every world the user has opened, forever.
//!
//! Nothing in this module ever calls `exportKey`, and there is deliberately
//! no API here that returns key bytes. If one is ever added, this file's
//! reason for existing is gone.
//!
//! # Lifetime
//!
//! Survives page reload within a session — otherwise every refresh cold-starts
//! the cache and SC-002 is unreachable. Does not survive sign-out (FR-016a):
//! [`forget`] deletes the record, and the encrypted blobs left on disk become
//! unreadable immediately, before the slow reclamation of the bytes
//! themselves has to finish.
//!
//! # Key loss is not an error
//!
//! FR-016c: a missing key must be indistinguishable from a cold cache. So
//! [`SessionKey::open`] answers `Ok(None)` when the ciphertext will not open,
//! and a caller that finds no key simply generates one and refetches. There
//! is no error path to handle, no user-facing message, and no state in which
//! the cache is "broken" rather than "empty".

use std::fmt;

/// AES-GCM nonce length, in bytes. 96 bits is the size the construction is
/// defined over; other lengths are legal but re-derived internally and buy
/// nothing.
pub const NONCE_LEN: usize = 12;

/// AES key length, in bits.
pub const KEY_BITS: u16 = 256;

/// The WebCrypto algorithm name, used for both generation and operation.
pub const ALGORITHM: &str = "AES-GCM";

/// Why a stored envelope could not be taken apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeError {
    /// Shorter than a nonce plus a non-empty ciphertext. Since GCM's tag
    /// alone is 16 bytes, anything this short was never produced by us.
    TooShort { found: usize },
}

impl fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { found } => write!(
                f,
                "encrypted envelope must exceed {NONCE_LEN} bytes, found {found}"
            ),
        }
    }
}

impl std::error::Error for EnvelopeError {}

/// The on-disk framing: a fresh random nonce, then the AES-GCM output.
///
/// Prepending the nonce rather than storing it beside the blob keeps a blob
/// file self-contained — which matters because the fingerprint filename
/// already makes a blob self-validating, and a second file to lose would
/// undo that. Pure and native-testable: framing bugs are silent data loss,
/// and they should not need a browser to catch.
pub struct Envelope;

impl Envelope {
    /// Frame a nonce and ciphertext for storage.
    pub fn seal(nonce: &[u8; NONCE_LEN], ciphertext: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        out.extend_from_slice(nonce);
        out.extend_from_slice(ciphertext);
        out
    }

    /// Split a stored envelope back into nonce and ciphertext.
    pub fn split(stored: &[u8]) -> Result<([u8; NONCE_LEN], &[u8]), EnvelopeError> {
        if stored.len() <= NONCE_LEN {
            return Err(EnvelopeError::TooShort {
                found: stored.len(),
            });
        }
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(&stored[..NONCE_LEN]);
        Ok((nonce, &stored[NONCE_LEN..]))
    }
}

/// The order in which a session key is obtained, as a state machine
/// (FR-021a).
///
/// # Why this is a type rather than four lines of `async`
///
/// The whole of FR-021a is one ordering claim: **the second look in the
/// store happens with the cross-tab lock held**. Written inline, that claim
/// lives only in the shape of an `async fn` that cannot run outside a
/// browser, and the way this feature has failed before is code that compiled
/// and was never executed. Here the ordering is a value, so `cargo test`
/// runs it — including a two-tab interleaving — and the wasm half below is
/// reduced to performing whatever step it is handed.
///
/// The re-check is not an optimisation. Without it the lock accomplishes
/// nothing at all: both tabs would still generate, merely one after the
/// other, and the loser's writes would still be unreadable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStep {
    /// Read whatever is stored, with no lock held. The overwhelmingly common
    /// case is a hit here, which is why it is not done under the lock: a
    /// warm start must not queue behind anything.
    Lookup,
    /// Take the cross-tab key-creation lock for this scope.
    AcquireLock,
    /// Read the stored key **again**. The point of the lock.
    Recheck,
    /// Generate a key and persist it. Only ever reached from [`Self::Recheck`].
    Generate,
    /// A key was found; use it.
    UseFound,
}

/// What the driver learned by performing a [`KeyStep`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    /// A usable key was in the store.
    Found,
    /// Nothing usable was in the store.
    Missing,
    /// The lock is now held.
    LockGranted,
    /// The lock could not be taken — no Web Locks, or the wait ran out.
    ///
    /// Proceeds down exactly the same path as a grant, which is FR-021d:
    /// without coordination the behaviour is today's, a possible duplicate
    /// key and therefore a cold cache, never a failed load.
    LockDenied,
}

/// A key acquisition in progress. See [`KeyStep`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyCreation {
    step: KeyStep,
    locked: bool,
}

impl Default for KeyCreation {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyCreation {
    /// Begin, at [`KeyStep::Lookup`].
    pub fn new() -> Self {
        Self {
            step: KeyStep::Lookup,
            locked: false,
        }
    }

    /// The step the driver should perform now.
    pub fn step(&self) -> KeyStep {
        self.step
    }

    /// Whether the cross-tab lock is held at this point.
    ///
    /// Exposed so the ordering claim can be asserted rather than believed.
    pub fn locked(&self) -> bool {
        self.locked
    }

    /// Record the result of the current step and return the next one.
    ///
    /// An event that does not belong to the current step leaves the machine
    /// where it was — there is no state in which a stray answer should be
    /// able to skip the re-check.
    pub fn advance(&mut self, event: KeyEvent) -> KeyStep {
        self.step = match (self.step, event) {
            (KeyStep::Lookup, KeyEvent::Found) => KeyStep::UseFound,
            (KeyStep::Lookup, KeyEvent::Missing) => KeyStep::AcquireLock,
            (KeyStep::AcquireLock, KeyEvent::LockGranted) => {
                self.locked = true;
                KeyStep::Recheck
            }
            (KeyStep::AcquireLock, KeyEvent::LockDenied) => KeyStep::Recheck,
            (KeyStep::Recheck, KeyEvent::Found) => KeyStep::UseFound,
            (KeyStep::Recheck, KeyEvent::Missing) => KeyStep::Generate,
            (step, _) => step,
        };
        self.step
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::{SessionKey, forget, load_or_create};

#[cfg(target_arch = "wasm32")]
mod wasm {
    use js_sys::{Array, Uint8Array};
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{AesGcmParams, AesKeyGenParams, Crypto, CryptoKey, SubtleCrypto};

    use super::{ALGORITHM, Envelope, KEY_BITS, KeyCreation, KeyEvent, KeyStep, NONCE_LEN};
    use crate::idb::Db;
    use crate::locks;
    use crate::opfs::UserScope;
    use crate::{CacheError, Result, STORE_KEYS, global_property, js_err};

    /// A handle to key material the browser holds and will not hand over.
    ///
    /// Clonable because a `CryptoKey` reference is just that — a reference.
    /// Cloning it grants no more access than the original had.
    #[derive(Clone)]
    pub struct SessionKey {
        key: CryptoKey,
    }

    impl SessionKey {
        /// Encrypt with a fresh random nonce.
        ///
        /// A new nonce per call is not optional for GCM: reusing one under
        /// the same key destroys both confidentiality and authenticity. The
        /// nonce comes from `crypto.getRandomValues`, never from a counter we
        /// would have to persist and could lose on a crash.
        pub async fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
            let mut nonce = [0u8; NONCE_LEN];
            subtle_crypto_root()?
                .get_random_values_with_u8_array(&mut nonce)
                .map_err(js_err)?;

            let params = AesGcmParams::new(ALGORITHM, &Uint8Array::from(&nonce[..]));
            let promise = subtle()?
                .encrypt_with_object_and_u8_array(&params, &self.key, plaintext)
                .map_err(js_err)?;
            let buffer = JsFuture::from(promise).await.map_err(js_err)?;
            let ciphertext = Uint8Array::new(&buffer).to_vec();
            Ok(Envelope::seal(&nonce, &ciphertext))
        }

        /// Decrypt a stored envelope.
        ///
        /// `Ok(None)` covers every way this legitimately fails: a malformed
        /// envelope, a tag that does not authenticate, or — the case that
        /// matters — a key that is simply not the one this blob was written
        /// under, because the session ended and a new key was generated.
        /// FR-016c requires that to be indistinguishable from a cold cache,
        /// so it is not an error and produces no diagnostic.
        pub async fn open(&self, stored: &[u8]) -> Result<Option<Vec<u8>>> {
            let Ok((nonce, ciphertext)) = Envelope::split(stored) else {
                return Ok(None);
            };
            let params = AesGcmParams::new(ALGORITHM, &Uint8Array::from(&nonce[..]));
            let promise =
                match subtle()?.decrypt_with_object_and_u8_array(&params, &self.key, ciphertext) {
                    Ok(promise) => promise,
                    Err(_) => return Ok(None),
                };
            match JsFuture::from(promise).await {
                Ok(buffer) => Ok(Some(Uint8Array::new(&buffer).to_vec())),
                Err(_) => Ok(None),
            }
        }

        /// Whether the browser considers this key exportable.
        ///
        /// Should always be `false`. Exposed so the invariant can be asserted
        /// from a browser test rather than merely believed, and checked on
        /// every load below.
        pub fn is_extractable(&self) -> bool {
            self.key.extractable()
        }
    }

    /// Fetch this scope's session key, generating and persisting one if there
    /// is none.
    ///
    /// # Two tabs starting together (FR-021a)
    ///
    /// This is the one operation in the cache where losing a race is
    /// expensive. Two tabs cold-starting both find no key, both generate one,
    /// and the last write wins — after which everything the loser wrote is
    /// ciphertext under a key nobody has. It degrades safely (FR-016c makes
    /// that a cache miss, not a failure) but a cache that silently never
    /// works in the situation users are actually in is still broken.
    ///
    /// So generation is serialised across tabs with a Web Lock, and — the
    /// part that matters — **the store is read again with the lock held**.
    /// The lock without the re-check would only make the two tabs generate in
    /// turn. The ordering is enforced by [`KeyCreation`], which is a pure
    /// state machine so that `cargo test` can run it; the loop below only
    /// performs whatever step it is told to.
    ///
    /// A lock that cannot be taken is not an error. `LockDenied` follows the
    /// same path as a grant, leaving today's behaviour: possibly a second key
    /// and therefore a cold cache, never a failed load (FR-021d).
    ///
    /// A key recovered from IndexedDB that reports `extractable: true` is
    /// discarded and replaced rather than used. Nothing in this crate can
    /// produce such a key, so its presence means something else wrote to our
    /// store — and honouring it would silently downgrade the one property
    /// this module exists to guarantee. The cost of being wrong is a cold
    /// cache, which is exactly the cost FR-016c already makes free.
    pub async fn load_or_create(scope: &UserScope) -> Result<SessionKey> {
        let db = Db::open().await?;
        let mut creation = KeyCreation::new();
        // Held from the moment it is granted until this function returns,
        // which is what keeps the re-check and the generation inside the
        // same critical section.
        let mut lock = None;
        let mut found = None;

        loop {
            match creation.step() {
                KeyStep::Lookup | KeyStep::Recheck => {
                    found = stored_key(&db, scope).await?;
                    creation.advance(if found.is_some() {
                        KeyEvent::Found
                    } else {
                        KeyEvent::Missing
                    });
                }
                KeyStep::AcquireLock => {
                    // At most once: the machine never revisits this step,
                    // and asking twice would queue us behind ourselves.
                    if lock.is_none() {
                        lock = locks::acquire_exclusive(
                            &locks::key_creation_lock(scope.as_str()),
                            locks::KEY_LOCK_TIMEOUT_MS,
                        )
                        .await;
                    }
                    creation.advance(if lock.is_some() {
                        KeyEvent::LockGranted
                    } else {
                        KeyEvent::LockDenied
                    });
                }
                KeyStep::UseFound => {
                    // The machine only reaches this from an event this loop
                    // raised on a `Some`, so the `None` arm is unreachable
                    // rather than a condition to handle.
                    return found.ok_or_else(|| {
                        CacheError::Corrupt("session key vanished between steps".into())
                    });
                }
                KeyStep::Generate => {
                    let key = generate().await?;
                    db.put(STORE_KEYS, scope.as_str(), &key.key).await?;
                    // `lock` drops here, releasing it for whichever tab is
                    // queued behind us — which will then find this key.
                    return Ok(key);
                }
            }
        }
    }

    /// The stored key for `scope`, if there is a usable one.
    ///
    /// Absence and unusability are the same answer, because the caller does
    /// the same thing with both.
    async fn stored_key(db: &Db, scope: &UserScope) -> Result<Option<SessionKey>> {
        let Some(value) = db.get(STORE_KEYS, scope.as_str()).await? else {
            return Ok(None);
        };
        let Ok(key) = value.dyn_into::<CryptoKey>() else {
            return Ok(None);
        };
        let key = SessionKey { key };
        if key.is_extractable() {
            db.delete(STORE_KEYS, scope.as_str()).await?;
            return Ok(None);
        }
        Ok(Some(key))
    }

    /// Discard this scope's key. Sign-out (FR-016a).
    ///
    /// This is the operation that makes the cache unreadable, and it is fast
    /// and bounded — one IndexedDB delete. Deleting the blobs is a separate,
    /// slower reclamation (FR-016b) that must never be relied on for
    /// confidentiality, because a multi-gigabyte store cannot be wiped before
    /// the tab closes.
    pub async fn forget(scope: &UserScope) -> Result<()> {
        Db::open().await?.delete(STORE_KEYS, scope.as_str()).await
    }

    /// Generate a fresh AES-GCM key, **non-extractable**.
    ///
    /// The `false` below is the security property this whole module is built
    /// around. Do not make it configurable.
    async fn generate() -> Result<SessionKey> {
        let params = AesKeyGenParams::new(ALGORITHM, KEY_BITS);
        let usages = Array::of2(&JsValue::from_str("encrypt"), &JsValue::from_str("decrypt"));
        let promise = subtle()?
            .generate_key_with_object(&params, false, &usages)
            .map_err(js_err)?;
        let key = JsFuture::from(promise).await.map_err(js_err)?;
        let key: CryptoKey = key
            .dyn_into()
            .map_err(|_| CacheError::Unsupported("WebCrypto AES-GCM key generation"))?;
        Ok(SessionKey { key })
    }

    fn subtle_crypto_root() -> Result<Crypto> {
        Ok(global_property("crypto")?.unchecked_into())
    }

    /// `crypto.subtle` is only present in secure contexts, so its absence is
    /// a deployment condition rather than a bug — reported as `Unsupported`
    /// so the caller degrades instead of failing the session.
    fn subtle() -> Result<SubtleCrypto> {
        let crypto = subtle_crypto_root()?;
        let subtle = crypto.subtle();
        if subtle.is_undefined() || subtle.is_null() {
            return Err(CacheError::Unsupported(
                "WebCrypto subtle (requires a secure context)",
            ));
        }
        Ok(subtle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_round_trips() {
        let nonce = [7u8; NONCE_LEN];
        let ciphertext = b"opaque".to_vec();
        let stored = Envelope::seal(&nonce, &ciphertext);
        let (got_nonce, got_ct) = Envelope::split(&stored).expect("well-formed envelope");
        assert_eq!(got_nonce, nonce);
        assert_eq!(got_ct, &ciphertext[..]);
    }

    #[test]
    fn envelope_carries_its_own_nonce() {
        // Self-containment is the property: the framed bytes are everything
        // needed to attempt a decryption, with no companion file.
        let stored = Envelope::seal(&[1u8; NONCE_LEN], b"x");
        assert_eq!(stored.len(), NONCE_LEN + 1);
        assert_eq!(&stored[..NONCE_LEN], &[1u8; NONCE_LEN]);
    }

    #[test]
    fn truncated_envelopes_are_refused() {
        for len in 0..=NONCE_LEN {
            let stored = vec![0u8; len];
            assert_eq!(
                Envelope::split(&stored),
                Err(EnvelopeError::TooShort { found: len }),
                "expected {len} bytes to be refused"
            );
        }
    }

    /// Drive a [`KeyCreation`] against a fake store and a fake cross-tab
    /// lock, recording every step. This is what makes FR-021a testable
    /// without a browser: the ordering is the thing being asserted, and the
    /// ordering lives in the machine, not in the I/O.
    #[derive(Default)]
    struct Profile {
        /// The one stored key, shared by every "tab". A `u32` stands in for
        /// the `CryptoKey`; what matters is only whether both tabs end up
        /// with the same one.
        stored: Option<u32>,
        /// Which tab, if any, holds the key-creation lock.
        lock_held_by: Option<usize>,
        /// Incremented on every generation. FR-021a is the claim that this
        /// reaches 1, not 2.
        generated: u32,
    }

    /// One tab's progress, and the trace it left.
    struct Tab {
        id: usize,
        machine: KeyCreation,
        /// `(step performed, was the lock held while performing it)`.
        trace: Vec<(KeyStep, bool)>,
        key: Option<u32>,
    }

    impl Tab {
        fn new(id: usize) -> Self {
            Self {
                id,
                machine: KeyCreation::new(),
                trace: Vec::new(),
                key: None,
            }
        }

        fn done(&self) -> bool {
            self.key.is_some()
        }

        /// Perform one step. Returns `false` if the tab is blocked on the
        /// lock and made no progress — which is precisely what serialisation
        /// looks like from inside a tab.
        fn tick(&mut self, profile: &mut Profile, locks_available: bool) -> bool {
            let step = self.machine.step();
            match step {
                KeyStep::Lookup | KeyStep::Recheck => {
                    self.trace.push((step, self.machine.locked()));
                    let found = profile.stored;
                    self.machine.advance(if found.is_some() {
                        KeyEvent::Found
                    } else {
                        KeyEvent::Missing
                    });
                }
                KeyStep::AcquireLock => {
                    if !locks_available {
                        self.trace.push((step, false));
                        self.machine.advance(KeyEvent::LockDenied);
                        return true;
                    }
                    match profile.lock_held_by {
                        // Somebody else has it. Wait — no progress this tick.
                        Some(holder) if holder != self.id => return false,
                        _ => {
                            profile.lock_held_by = Some(self.id);
                            self.trace.push((step, false));
                            self.machine.advance(KeyEvent::LockGranted);
                        }
                    }
                }
                KeyStep::Generate => {
                    self.trace.push((step, self.machine.locked()));
                    profile.generated += 1;
                    let key = profile.generated;
                    profile.stored = Some(key);
                    self.key = Some(key);
                    self.release(profile);
                }
                KeyStep::UseFound => {
                    self.trace.push((step, self.machine.locked()));
                    self.key = profile.stored;
                    self.release(profile);
                }
            }
            true
        }

        fn release(&self, profile: &mut Profile) {
            if profile.lock_held_by == Some(self.id) {
                profile.lock_held_by = None;
            }
        }

        fn steps(&self) -> Vec<KeyStep> {
            self.trace.iter().map(|(step, _)| *step).collect()
        }

        fn locked_during(&self, step: KeyStep) -> bool {
            self.trace
                .iter()
                .find(|(performed, _)| *performed == step)
                .map(|(_, locked)| *locked)
                .unwrap_or(false)
        }
    }

    fn run_alone(profile: &mut Profile, locks_available: bool) -> Tab {
        let mut tab = Tab::new(0);
        for _ in 0..16 {
            if tab.done() {
                break;
            }
            tab.tick(profile, locks_available);
        }
        assert!(tab.done(), "a lone tab must always terminate");
        tab
    }

    #[test]
    fn a_warm_start_takes_no_lock_at_all() {
        // The common case. Queueing every reload behind a lock would make
        // the coordination cost more than the race it prevents.
        let mut profile = Profile {
            stored: Some(9),
            ..Profile::default()
        };
        let tab = run_alone(&mut profile, true);
        assert_eq!(tab.steps(), vec![KeyStep::Lookup, KeyStep::UseFound]);
        assert_eq!(tab.key, Some(9));
        assert_eq!(profile.generated, 0);
    }

    #[test]
    fn the_recheck_happens_with_the_lock_held() {
        // FR-021a in one assertion. A re-check performed *before* the lock,
        // or a generation performed outside it, would leave the race exactly
        // where it was.
        let mut profile = Profile::default();
        let tab = run_alone(&mut profile, true);
        assert_eq!(
            tab.steps(),
            vec![
                KeyStep::Lookup,
                KeyStep::AcquireLock,
                KeyStep::Recheck,
                KeyStep::Generate,
            ]
        );
        assert!(
            tab.locked_during(KeyStep::Recheck),
            "the re-check must be inside the critical section, or it proves nothing"
        );
        assert!(tab.locked_during(KeyStep::Generate));
        assert!(!tab.locked_during(KeyStep::Lookup));
    }

    #[test]
    fn generation_is_only_ever_reached_through_the_recheck() {
        // Exhaustive over the machine: from every state, feed every event,
        // and confirm nothing lands on `Generate` except a `Recheck` that
        // found nothing.
        let states = [
            KeyStep::Lookup,
            KeyStep::AcquireLock,
            KeyStep::Recheck,
            KeyStep::Generate,
            KeyStep::UseFound,
        ];
        let events = [
            KeyEvent::Found,
            KeyEvent::Missing,
            KeyEvent::LockGranted,
            KeyEvent::LockDenied,
        ];
        for from in states {
            for event in events {
                let mut machine = KeyCreation::new();
                // Walk the machine into `from`.
                match from {
                    KeyStep::Lookup => {}
                    KeyStep::AcquireLock => {
                        machine.advance(KeyEvent::Missing);
                    }
                    KeyStep::Recheck => {
                        machine.advance(KeyEvent::Missing);
                        machine.advance(KeyEvent::LockGranted);
                    }
                    KeyStep::Generate => {
                        machine.advance(KeyEvent::Missing);
                        machine.advance(KeyEvent::LockGranted);
                        machine.advance(KeyEvent::Missing);
                    }
                    KeyStep::UseFound => {
                        machine.advance(KeyEvent::Found);
                    }
                }
                assert_eq!(machine.step(), from);
                let next = machine.advance(event);
                if next == KeyStep::Generate && from != KeyStep::Generate {
                    assert_eq!(
                        (from, event),
                        (KeyStep::Recheck, KeyEvent::Missing),
                        "generation must not be reachable from {from:?} on {event:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn two_tabs_cold_starting_together_produce_one_key() {
        // The failure this whole mechanism exists for. Both tabs look, both
        // find nothing, and only then does either take the lock — so the
        // loser's re-check is what saves it.
        let mut profile = Profile::default();
        let mut a = Tab::new(0);
        let mut b = Tab::new(1);

        // Both look first, before either can lock. This is the interleaving
        // that breaks an unlocked implementation.
        assert!(a.tick(&mut profile, true));
        assert!(b.tick(&mut profile, true));
        assert_eq!(a.machine.step(), KeyStep::AcquireLock);
        assert_eq!(b.machine.step(), KeyStep::AcquireLock);

        // A takes the lock; B cannot, and makes no progress until A is done.
        assert!(a.tick(&mut profile, true));
        assert!(
            !b.tick(&mut profile, true),
            "the second tab must block rather than proceed to generate"
        );

        // Round-robin to completion.
        for _ in 0..16 {
            if a.done() && b.done() {
                break;
            }
            if !a.done() {
                a.tick(&mut profile, true);
            }
            if !b.done() {
                b.tick(&mut profile, true);
            }
        }

        assert!(a.done() && b.done(), "neither tab may be left waiting");
        assert_eq!(
            profile.generated, 1,
            "exactly one key may be generated across the profile"
        );
        assert_eq!(a.key, b.key, "both tabs must end up on the same key");
        assert_eq!(
            b.steps().last(),
            Some(&KeyStep::UseFound),
            "the losing tab must adopt the winner's key, not mint its own"
        );
        assert!(!b.steps().contains(&KeyStep::Generate));
    }

    #[test]
    fn without_web_locks_both_tabs_still_finish_with_a_key() {
        // FR-021d. Degraded means a duplicate key and therefore a cold cache
        // for one tab — which FR-016c already makes free — and emphatically
        // not a hang or a failure.
        let mut profile = Profile::default();
        let a = run_alone(&mut profile, false);
        let b = run_alone(&mut profile, false);
        assert!(a.key.is_some() && b.key.is_some());
        assert!(
            !a.locked_during(KeyStep::Recheck),
            "no lock was available, so none may be claimed"
        );
        // The second tab still finds the first's key, because the re-check
        // runs whether or not the lock was granted.
        assert_eq!(profile.generated, 1);
        assert_eq!(b.steps().last(), Some(&KeyStep::UseFound));
    }

    #[test]
    fn nonce_is_ninety_six_bits_and_key_is_two_fifty_six() {
        assert_eq!(NONCE_LEN * 8, 96);
        assert_eq!(KEY_BITS, 256);
    }
}
