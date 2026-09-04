//! Symmetric encryption for secrets this server stores and must read back.
//!
//! # Why this is its own module
//!
//! It was not. These three functions were private to `auth/mod.rs`, which was
//! the right home while two-factor secrets and OAuth tokens were the only
//! things being encrypted — they are all authentication concerns and they all
//! lived together.
//!
//! Spec 034 broke that. A repository credential is not an authentication
//! concern, it needs exactly this treatment, and `auth/mod.rs` did not export
//! any of it. The choice was to move these here or to write a second
//! implementation beside them, and **a second encryption implementation is how
//! two of them drift until one is wrong** — usually the newer one, usually in a
//! way nothing notices until it matters.
//!
//! So this is a move, not a rewrite. The format, the key derivation and the
//! error strings are unchanged, which matters more than it sounds: every
//! `two_factor_secret_encrypted` and `access_token_encrypted` value already in
//! a database was written by this code, and a "tidying" pass over it would be a
//! silent, unrecoverable data migration.
//!
//! # The stored format
//!
//! `v1.<nonce>.<ciphertext>`, both URL-safe base64 without padding, AES-256-GCM
//! with a 12-byte random nonce per encryption. The version prefix is checked on
//! read and refused if unrecognised, which is what makes a future format change
//! possible without guessing at what an old value meant.
//!
//! # The key
//!
//! Derived from the instance's configured secret by SHA-256, so there is one
//! key per instance and it is not stored anywhere separately. That also means
//! **rotating the instance secret makes every stored ciphertext unreadable** —
//! true before this move and stated here because this is now the place someone
//! looks to find it out.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine as _;
use base64::engine::general_purpose;
use rand::RngExt as _;
use sha2::{Digest, Sha256};

/// The instance's encryption key, derived from its configured secret.
pub fn encryption_key_from_config_secret(secret_b64: &str) -> Result<[u8; 32], String> {
    let secret_bytes = general_purpose::STANDARD
        .decode(secret_b64)
        .map_err(|_| "Config secret is not valid base64".to_string())?;
    let digest = Sha256::digest(secret_bytes);
    let mut key = [0u8; 32];
    key.copy_from_slice(&digest[..32]);
    Ok(key)
}

/// Encrypt a secret for storage, as `v1.<nonce>.<ciphertext>`.
pub fn encrypt_secret(plaintext: &str, key: &[u8; 32]) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("Cipher init failed: {e}"))?;
    let mut nonce_bytes = [0u8; 12];
    let mut rng = rand::rng();
    rng.fill(&mut nonce_bytes);
    let nonce = Nonce::from(nonce_bytes);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encryption failed: {e}"))?;

    let nonce_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(nonce_bytes);
    let cipher_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(ciphertext);
    Ok(format!("v1.{nonce_b64}.{cipher_b64}"))
}

/// Read back a secret written by [`encrypt_secret`].
pub fn decrypt_secret(ciphertext: &str, key: &[u8; 32]) -> Result<String, String> {
    let mut parts = ciphertext.split('.');
    let version = parts
        .next()
        .ok_or_else(|| "Invalid encrypted secret format".to_string())?;
    if version != "v1" {
        return Err("Unsupported encrypted secret version".to_string());
    }
    let nonce_b64 = parts
        .next()
        .ok_or_else(|| "Invalid encrypted secret format".to_string())?;
    let cipher_b64 = parts
        .next()
        .ok_or_else(|| "Invalid encrypted secret format".to_string())?;
    if parts.next().is_some() {
        return Err("Invalid encrypted secret format".to_string());
    }

    let nonce_vec = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(nonce_b64)
        .map_err(|_| "Invalid encrypted secret nonce".to_string())?;
    if nonce_vec.len() != 12 {
        return Err("Invalid encrypted secret nonce length".to_string());
    }
    let cipher_vec = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cipher_b64)
        .map_err(|_| "Invalid encrypted secret payload".to_string())?;

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("Cipher init failed: {e}"))?;
    let nonce = Nonce::try_from(&nonce_vec[..])
        .map_err(|_| "Invalid encrypted secret nonce length".to_string())?;
    let plaintext = cipher
        .decrypt(&nonce, cipher_vec.as_ref())
        .map_err(|e| format!("Decryption failed: {e}"))?;

    String::from_utf8(plaintext).map_err(|_| "Decrypted secret is not valid UTF-8".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> [u8; 32] {
        encryption_key_from_config_secret(&general_purpose::STANDARD.encode("a test secret"))
            .expect("key derives")
    }

    #[test]
    fn a_secret_survives_a_round_trip() {
        let k = key();
        let encrypted = encrypt_secret("hunter2", &k).expect("encrypts");
        assert_eq!(decrypt_secret(&encrypted, &k).expect("decrypts"), "hunter2");
    }

    /// A fresh nonce per call, so the same plaintext never produces the same
    /// stored value. Two identical two-factor secrets that encrypted alike
    /// would leak that they were identical to anyone reading the table.
    #[test]
    fn the_same_plaintext_encrypts_differently_every_time() {
        let k = key();
        let a = encrypt_secret("same", &k).expect("encrypts");
        let b = encrypt_secret("same", &k).expect("encrypts");
        assert_ne!(a, b);
        assert_eq!(
            decrypt_secret(&a, &k).unwrap(),
            decrypt_secret(&b, &k).unwrap()
        );
    }

    /// Rotating the instance secret makes stored ciphertext unreadable. Asserted
    /// rather than only documented, because it is the operational consequence
    /// most likely to be discovered the hard way.
    #[test]
    fn another_key_cannot_read_it() {
        let encrypted = encrypt_secret("hunter2", &key()).expect("encrypts");
        let other = encryption_key_from_config_secret(
            &general_purpose::STANDARD.encode("a different secret"),
        )
        .expect("key derives");
        assert!(decrypt_secret(&encrypted, &other).is_err());
    }

    /// The version prefix is what makes a future format change possible without
    /// guessing at what an old value meant, so an unknown one must be refused
    /// rather than parsed hopefully.
    #[test]
    fn an_unknown_version_is_refused() {
        let k = key();
        let encrypted = encrypt_secret("hunter2", &k).expect("encrypts");
        let tampered = encrypted.replacen("v1.", "v2.", 1);
        assert!(decrypt_secret(&tampered, &k).is_err());
    }

    #[test]
    fn a_malformed_value_is_refused_rather_than_panicking() {
        let k = key();
        for bad in ["", "v1", "v1.only-two", "v1.a.b.c", "v1.!!!.!!!"] {
            assert!(decrypt_secret(bad, &k).is_err(), "accepted {bad:?}");
        }
    }

    /// AES-GCM authenticates as well as encrypts, so a flipped byte must fail
    /// rather than decrypt to something plausible.
    #[test]
    fn a_tampered_payload_is_refused() {
        let k = key();
        let encrypted = encrypt_secret("hunter2", &k).expect("encrypts");
        let mut parts: Vec<&str> = encrypted.split('.').collect();
        let payload = parts[2].to_string();
        let flipped: String = payload
            .chars()
            .enumerate()
            .map(|(i, c)| if i == 0 && c != 'A' { 'A' } else { c })
            .collect();
        parts[2] = &flipped;
        assert!(decrypt_secret(&parts.join("."), &k).is_err());
    }
}
