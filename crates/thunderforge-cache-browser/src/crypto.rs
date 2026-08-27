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

#[cfg(target_arch = "wasm32")]
pub use wasm::{SessionKey, forget, load_or_create};

#[cfg(target_arch = "wasm32")]
mod wasm {
    use js_sys::{Array, Uint8Array};
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{AesGcmParams, AesKeyGenParams, Crypto, CryptoKey, SubtleCrypto};

    use super::{ALGORITHM, Envelope, KEY_BITS, NONCE_LEN};
    use crate::idb::Db;
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
    /// A key recovered from IndexedDB that reports `extractable: true` is
    /// discarded and replaced rather than used. Nothing in this crate can
    /// produce such a key, so its presence means something else wrote to our
    /// store — and honouring it would silently downgrade the one property
    /// this module exists to guarantee. The cost of being wrong is a cold
    /// cache, which is exactly the cost FR-016c already makes free.
    pub async fn load_or_create(scope: &UserScope) -> Result<SessionKey> {
        let db = Db::open().await?;

        if let Some(value) = db.get(STORE_KEYS, scope.as_str()).await?
            && let Ok(key) = value.dyn_into::<CryptoKey>()
        {
            let key = SessionKey { key };
            if key.is_extractable() {
                db.delete(STORE_KEYS, scope.as_str()).await?;
            } else {
                return Ok(key);
            }
        }

        let key = generate().await?;
        db.put(STORE_KEYS, scope.as_str(), &key.key).await?;
        Ok(key)
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

    #[test]
    fn nonce_is_ninety_six_bits_and_key_is_two_fifty_six() {
        assert_eq!(NONCE_LEN * 8, 96);
        assert_eq!(KEY_BITS, 256);
    }
}
