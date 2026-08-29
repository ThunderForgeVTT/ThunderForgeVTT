//! Comparison that does not leak *where* two secrets first differ.

/// Compare two byte strings in time independent of their contents.
///
/// A naive `==` returns as soon as it finds a mismatched byte, and an
/// attacker who can time the answer can then recover a secret one byte at a
/// time instead of guessing all of it at once. Every caller comparing a
/// value an attacker supplies against one they must not learn — CSRF tokens,
/// OAuth `state`, PKCE challenges — goes through here.
///
/// Lengths are folded into the same accumulator rather than short-circuiting
/// on `a.len() != b.len()`, so a wrong-length guess costs the attacker the
/// same observation as a wrong-content one. The loop runs to the longer of
/// the two either way.
pub fn secure_equals(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    let max_len = std::cmp::max(a.len(), b.len());
    for i in 0..max_len {
        let av = *a.get(i).unwrap_or(&0);
        let bv = *b.get(i).unwrap_or(&0);
        diff |= (av ^ bv) as usize;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::secure_equals;
    use proptest::prelude::*;

    #[test]
    fn equal_slices_match() {
        assert!(secure_equals(b"correct-horse", b"correct-horse"));
    }

    #[test]
    fn a_prefix_is_not_a_match() {
        // The padding-with-zeroes trick must not make a short guess equal to
        // a longer secret; that would accept any prefix of a real token.
        assert!(!secure_equals(b"corr", b"correct-horse"));
    }

    #[test]
    fn zero_bytes_do_not_alias_absent_bytes() {
        assert!(!secure_equals(b"a\0", b"a"));
    }

    proptest! {
        /// Agreement with `==` is the whole correctness claim: the constant
        /// time is a property of *how* it answers, not *what* it answers.
        #[test]
        fn agrees_with_plain_equality(a: Vec<u8>, b: Vec<u8>) {
            prop_assert_eq!(secure_equals(&a, &b), a == b);
        }

        #[test]
        fn is_reflexive(a: Vec<u8>) {
            prop_assert!(secure_equals(&a, &a));
        }

        #[test]
        fn is_symmetric(a: Vec<u8>, b: Vec<u8>) {
            prop_assert_eq!(secure_equals(&a, &b), secure_equals(&b, &a));
        }
    }
}
