//! The server's half of FR-012: **refuse, never rewrite.**
//!
//! # Whose job this is not
//!
//! Redaction happens in the browser, at capture time, before a line enters the
//! log ring buffer. That is what makes the promise keepable: what the review
//! renders is the buffer, what is submitted is the buffer, and they are the
//! same bytes because they are the same array. Nothing here re-derives an
//! attachment, and nothing here edits one.
//!
//! # Why refusing is the correct behaviour and rewriting is not
//!
//! An edit after approval would mean the person saw something other than what
//! was sent — which is exactly the failure FR-012 exists to prevent, **even
//! when the edit is an improvement**. So a match is a refusal that names the
//! kind found, the draft survives on the client, and nothing is written.
//!
//! It also means the guarantee does not depend on the client behaving: a
//! client tampered with to skip its own filter cannot post a token through
//! this path.
//!
//! # One rule set, two consumers
//!
//! `config/feedback-redaction.json` is read by
//! `apps/web/src/services/feedbackRedaction.ts` in the browser and by this
//! module on the server, and neither owns it. A rule that existed only on the
//! server would produce refusals the person could not have anticipated; a rule
//! that existed only on the client would be no rule at all.
//!
//! The file is compiled in with `include_str!`, the way `config/` files
//! already are on this side, because a rule set that can be absent at runtime
//! is a rule set that will be.
//!
//! # The pattern dialect, and why parsing can fail loudly
//!
//! Every pattern is written in the subset JavaScript's `RegExp` and Rust's
//! `regex` crate share — no lookaround, no backreferences, no inline flags —
//! and the file's own header says so at length. A pattern that Rust cannot
//! compile is therefore a bug in the rule set rather than a runtime condition,
//! and [`rules`] treats it as one: the rule is skipped and the others still
//! apply, so a typo in one pattern cannot switch the whole validator off.
//! [`every_rule_compiles`] is the test that stops it reaching a release.

use std::sync::OnceLock;

use regex::{Regex, RegexBuilder};
use serde::Deserialize;

/// The rule set, verbatim. One file, two consumers, no second copy.
const RULE_SET: &str = include_str!("../../../../config/feedback-redaction.json");

#[derive(Debug, Deserialize)]
struct RuleSet {
    rules: Vec<DeclaredRule>,
}

#[derive(Debug, Deserialize)]
struct DeclaredRule {
    kind: String,
    pattern: Option<String>,
    #[serde(default)]
    ignore_case: bool,
    replacement: String,
    /// A rule whose pattern is not a constant — it is built at runtime from a
    /// value the side in question knows. There is one: the submitter's own
    /// email address. Declared on both sides so that they agree the kind
    /// exists and share its replacement.
    #[serde(default)]
    dynamic: bool,
}

/// One compiled rule: what it is called, what it matches, and the marker a
/// client that applied it would have left behind.
pub struct Rule {
    pub kind: String,
    pub pattern: Regex,
    pub replacement: String,
}

/// The compiled rule set. Built once; the file cannot change under a running
/// process because it is compiled into it.
pub fn rules() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let parsed: RuleSet =
            serde_json::from_str(RULE_SET).expect("config/feedback-redaction.json is valid JSON");
        parsed
            .rules
            .into_iter()
            .filter(|rule| !rule.dynamic)
            .filter_map(|rule| {
                let pattern = rule.pattern.as_ref()?;
                let compiled = RegexBuilder::new(pattern)
                    .case_insensitive(rule.ignore_case)
                    .build()
                    .ok()?;
                Some(Rule {
                    kind: rule.kind,
                    pattern: compiled,
                    replacement: rule.replacement,
                })
            })
            .collect()
    })
}

/// The kind of secret an attachment still contains, if it contains one.
///
/// A **kind**, never the matched text. FR-021's rule about credentials in
/// diagnostics applies just as much to a refusal a person reads: telling
/// somebody "your log contains a bearer token" is help, and quoting it back at
/// them in an error a browser will log is not.
pub fn secret_kind_in(text: &str) -> Option<&'static str> {
    let scanned = without_markers(text);
    for rule in rules() {
        if rule.pattern.is_match(&scanned) {
            // The kinds are a fixed vocabulary from a compiled-in file, so a
            // `&'static str` is honest here and keeps the refusal free of
            // anything derived from the payload.
            return Some(kind_name(&rule.kind));
        }
    }
    None
}

/// Blank out the markers the client's own filter left, before scanning.
///
/// **This is not cosmetic, and leaving it out breaks the feature.** A marker
/// is evidence that a rule fired, and several markers match the very rule that
/// wrote them: `[redacted: bearer token]` contains `bearer token`, which is a
/// bearer-token match by the shared pattern, so a validator that scanned the
/// raw text would refuse every correctly redacted log bundle — a false
/// positive on exactly the submissions this feature exists to accept. Found by
/// the test below, which is why it is a test and not a comment.
///
/// The neutral placeholder is deliberately wordless: it must match no rule.
fn without_markers(text: &str) -> String {
    let mut out = text.to_string();
    for rule in rules() {
        if out.contains(&rule.replacement) {
            out = out.replace(&rule.replacement, "[redacted]");
        }
    }
    out
}

/// The submitter's own address, which is not a constant and so cannot live in
/// the file (FR-013).
///
/// Checked separately from [`secret_kind_in`] because the server knows the
/// address from the row and the client knows it from the session — the same
/// rule, built from two different places, which is why the declaration marks
/// it `dynamic` rather than carrying a pattern.
pub fn contains_submitter_email(text: &str, email: &str) -> bool {
    if email.trim().is_empty() {
        return false;
    }
    text.to_ascii_lowercase()
        .contains(&email.trim().to_ascii_lowercase())
}

/// What an attachment is refused for, or nothing.
///
/// Takes the bytes as given and **returns nothing derived from them**. The
/// caller refuses on `Some`; there is deliberately no variant that returns
/// altered content, because there is no code path in this product that edits
/// an approved attachment.
pub fn refusal_for(content: &[u8], submitter_email: &str) -> Option<&'static str> {
    // Bytes that are not text cannot carry a pattern-shaped secret, and a
    // screenshot is not text. Decoding lossily rather than refusing on invalid
    // UTF-8 keeps a log bundle with one broken byte from being unsubmittable.
    let text = String::from_utf8_lossy(content);
    if let Some(kind) = secret_kind_in(&text) {
        return Some(kind);
    }
    if contains_submitter_email(&text, submitter_email) {
        return Some("submitter_email");
    }
    None
}

/// The stable name for one rule kind.
///
/// The vocabulary is closed and shared with the client's marker text, so a
/// kind the file grows without this function growing with it reports as
/// `secret` rather than as a leaked string from the rule set.
fn kind_name(kind: &str) -> &'static str {
    match kind {
        "pem" => "pem",
        "cookie" => "cookie",
        "bearer_token" => "bearer_token",
        "jwt" => "jwt",
        "url_credential" => "url_credential",
        "aws_key" => "aws_key",
        "submitter_email" => "submitter_email",
        _ => "secret",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dialect constraint the file's header states, enforced rather than
    /// trusted: a pattern Rust cannot compile is silently dropped by
    /// [`rules`], and a dropped rule is a rule that is not protecting anybody.
    #[test]
    fn every_rule_compiles() {
        let parsed: RuleSet = serde_json::from_str(RULE_SET).expect("valid JSON");
        let declared = parsed.rules.iter().filter(|r| !r.dynamic).count();
        assert_eq!(
            rules().len(),
            declared,
            "a pattern in config/feedback-redaction.json did not compile as a Rust regex"
        );
    }

    /// The one rule with no pattern is the one that cannot have one, and it is
    /// checked by its own function against a value the row supplies.
    #[test]
    fn the_dynamic_rule_is_the_submitters_address_and_only_that() {
        let parsed: RuleSet = serde_json::from_str(RULE_SET).expect("valid JSON");
        let dynamic: Vec<&str> = parsed
            .rules
            .iter()
            .filter(|r| r.dynamic)
            .map(|r| r.kind.as_str())
            .collect();
        assert_eq!(dynamic, vec!["submitter_email"]);
    }

    #[test]
    fn a_clean_log_bundle_is_not_refused() {
        let clean = b"2026-09-07T14:01:58Z ERROR GraphQL request failed (rollDice): 500\n\
                      2026-09-07T14:01:59Z WARN  reconnecting to /world/abc/play\n";
        assert_eq!(refusal_for(clean, "someone@example.invalid"), None);
    }

    /// One assertion per rule kind. Each of these is a route a token could
    /// take out of the browser, and each has to be refused rather than
    /// quietly accepted.
    #[test]
    fn each_kind_of_secret_is_found() {
        let cases: [(&str, &str); 6] = [
            (
                "bearer_token",
                "GET /graphql Authorization: Bearer abcdefghijklmnop",
            ),
            ("cookie", "cookie: session=abc123; other=1"),
            (
                "jwt",
                "token eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.dBjftJeZ4CVPmB92K27uhbUJU1p1r_wW1g",
            ),
            (
                "url_credential",
                "GET https://storage/x.webp?X-Amz-Signature=deadbeefcafe failed",
            ),
            ("aws_key", "creds AKIAIOSFODNN7EXAMPLE rejected"),
            (
                "pem",
                "-----BEGIN RSA PRIVATE KEY-----\nMIIE\n-----END RSA PRIVATE KEY-----",
            ),
        ];
        for (kind, line) in cases {
            assert_eq!(
                secret_kind_in(line),
                Some(kind),
                "`{line}` was not refused as {kind}"
            );
        }
    }

    /// The marker a properly redacted client leaves is evidence that the rule
    /// fired, not a secret. Refusing it would refuse every correct submission,
    /// which is the failure mode that would have made this whole validator
    /// useless the day it shipped.
    #[test]
    fn a_client_that_did_its_job_is_not_refused_for_saying_so() {
        let redacted = b"GET /graphql [redacted: bearer token]\n\
                         [redacted: cookie]\n\
                         [redacted: signed url]\n\
                         [redacted: private key]\n";
        assert_eq!(refusal_for(redacted, "someone@example.invalid"), None);
    }

    /// The specific trap: the marker for a bearer token is itself a
    /// bearer-token match under the shared pattern (`bearer` followed by
    /// `token`). Without [`without_markers`] every properly redacted bundle
    /// would be refused, which is a false positive on precisely the
    /// submissions this validator exists to let through.
    #[test]
    fn a_markers_own_text_does_not_re_trigger_the_rule_that_wrote_it() {
        for rule in rules() {
            assert_eq!(
                secret_kind_in(&rule.replacement),
                None,
                "the marker `{}` is matched by a rule",
                rule.replacement
            );
        }
    }

    /// FR-013's second route. The address is not in the file because it is not
    /// a constant, and a rule that exists only in a comment is not a rule.
    #[test]
    fn the_submitters_own_address_is_refused_case_insensitively() {
        let line = b"POST /graphql as Player.One@Example.Invalid";
        assert_eq!(
            refusal_for(line, "player.one@example.invalid"),
            Some("submitter_email")
        );
        assert_eq!(refusal_for(line, ""), None);
    }

    /// A refusal names a kind from a closed vocabulary and never anything
    /// derived from the payload. The one thing this function must never do is
    /// quote the secret back.
    #[test]
    fn a_refusal_names_a_kind_and_never_the_match() {
        let kind = refusal_for(b"Authorization: Bearer sup3rsecrettoken", "")
            .expect("a bearer token is refused");
        assert_eq!(kind, "bearer_token");
        assert!(!kind.contains("sup3rsecret"));
    }
}
