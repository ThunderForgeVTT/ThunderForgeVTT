//! Redaction for the anonymous tier's `server.error` records (FR-016).
//!
//! The feedback rule set (`config/feedback-redaction.json`, through
//! `feedback::redaction::rules`) is applied first, so a token, cookie, key or
//! signed URL reads exactly as it would in a feedback report. Two rules are
//! added on top, because the disclosure promises that no email and no id
//! leaves (spec.md, "What we receive"), and the feedback set matches an email
//! only for the submitter it knows:
//!
//! - any email address becomes the set's own `[redacted: email]`;
//! - any UUID becomes `[redacted: id]`.
//!
//! They are not added to the shared file, because the server refuses (never
//! rewrites) feedback that matches a rule there, and a report that mentions an
//! email address is not a secret.

use crate::feedback::redaction::rules;
use regex::Regex;
use std::sync::OnceLock;

pub const EMAIL_MARKER: &str = "[redacted: email]";
pub const ID_MARKER: &str = "[redacted: id]";

fn extra() -> &'static [(Regex, &'static str)] {
    static EXTRA: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    EXTRA.get_or_init(|| {
        vec![
            (
                Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+").expect("email"),
                EMAIL_MARKER,
            ),
            (
                Regex::new(r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b")
                    .expect("uuid"),
                ID_MARKER,
            ),
        ]
    })
}

/// The text with every feedback rule, every email and every UUID replaced.
pub fn redact(text: &str) -> String {
    let mut out = text.to_string();
    for rule in rules() {
        out = rule
            .pattern
            .replace_all(&out, rule.replacement.as_str())
            .into_owned();
    }
    for (pattern, marker) in extra() {
        out = pattern.replace_all(&out, *marker).into_owned();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails_ids_and_tokens_are_removed() {
        let text = "user canary-7f3a@example.org in world 0192f1c4-7d1e-7a2b-9c3d-4e5f60718293 \
                    sent Authorization: Bearer abcdefghijklmnop";
        let out = redact(text);
        assert!(!out.contains("canary-7f3a@example.org"), "{out}");
        assert!(!out.contains("0192f1c4"), "{out}");
        assert!(!out.contains("abcdefghijklmnop"), "{out}");
        assert!(
            out.contains(EMAIL_MARKER) && out.contains(ID_MARKER),
            "{out}"
        );
    }

    #[test]
    fn plain_text_is_untouched() {
        assert_eq!(redact("connection refused"), "connection refused");
    }
}
