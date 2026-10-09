//! How long a value may be, in characters (FR-040).
//!
//! Every honest sender cuts its own values at or below these, so only a
//! sender that ignores its contract trips the gateway's `attribute_too_large`.

pub const CAP_DEFAULT: usize = 1024;
/// `error.message` and `exception.message`.
pub const CAP_MESSAGE: usize = 512;
/// `error.stack` and `exception.stacktrace`.
pub const CAP_STACK: usize = 4096;
pub const CAP_LOG_BODY: usize = 4096;

/// The cap for an attribute's value.
pub fn cap_for(key: &str) -> usize {
    match key {
        "error.message" | "exception.message" => CAP_MESSAGE,
        "error.stack" | "exception.stacktrace" => CAP_STACK,
        _ => CAP_DEFAULT,
    }
}

/// `text` cut to at most `max` characters, on a character boundary.
pub fn truncate_chars(text: &str, max: usize) -> &str {
    match text.char_indices().nth(max) {
        Some((i, _)) => &text[..i],
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps() {
        assert_eq!(cap_for("error.message"), 512);
        assert_eq!(cap_for("exception.message"), 512);
        assert_eq!(cap_for("error.stack"), 4096);
        assert_eq!(cap_for("exception.stacktrace"), 4096);
        assert_eq!(cap_for("route"), 1024);
        assert_eq!(CAP_LOG_BODY, 4096);
    }

    #[test]
    fn truncation_counts_characters() {
        assert_eq!(truncate_chars("héllo", 2), "hé");
        assert_eq!(truncate_chars("abc", 10), "abc");
        assert_eq!(truncate_chars("abc", 0), "");
    }
}
