use super::*;

/// A hash written by the previous `argon2`/`password-hash` line must
/// still verify.
///
/// This is the half of a hashing upgrade that cannot be caught by
/// round-tripping a fresh hash: the new code agreeing with itself proves
/// nothing about the credentials already in the database, and getting it
/// wrong locks every existing account out. The literal below is the
/// seeded demo hash from `seeds/e2e_demo.sql`, produced before the
/// upgrade, with the password that file documents.
#[test]
fn a_hash_written_before_the_upgrade_still_verifies() {
    let stored = "$argon2id$v=19$m=19456,t=2,p=1$niEwA63DF+T39rY601qniQ$r0q7cdblJI4nH9jsOohucWwiYaWLtXKAqDxvq62Bj+s";
    let parsed = PasswordHash::new(stored).expect("the stored PHC string must still parse");
    Argon2::default()
        .verify_password(b"Sup3r-Secret-Passphrase!", &parsed)
        .expect("an existing account's password must still verify");
    assert!(
        Argon2::default()
            .verify_password(b"not-the-password", &parsed)
            .is_err(),
        "and the wrong password must still be refused",
    );
}

/// Salts are generated per call, now that the hasher owns that step.
#[test]
fn each_hash_gets_its_own_salt() {
    let first = hash_password("Sup3r-Secret-Passphrase!").expect("hashing must succeed");
    let second = hash_password("Sup3r-Secret-Passphrase!").expect("hashing must succeed");
    assert_ne!(
        first, second,
        "the same password hashed twice must not produce the same string",
    );
    for hash in [&first, &second] {
        let parsed = PasswordHash::new(hash).expect("output must be a valid PHC string");
        Argon2::default()
            .verify_password(b"Sup3r-Secret-Passphrase!", &parsed)
            .expect("a freshly written hash must verify");
    }
}
