//! What actually arrives at the destination: a title, labels, and a body.
//!
//! Pure — no database, no host, no clock. Every rule in
//! `contracts/delivery.md` § 3 is therefore one assertion at the bottom of
//! this file rather than something to be read out of a delivery attempt.
//!
//! # The three things this file is answerable for
//!
//! 1. **FR-020** — each kind is distinguishable at the destination *without
//!    reading the body*. That is [`labels`], and the labels go on in the
//!    creation request so that a failure between creating and labelling cannot
//!    exist.
//! 2. **FR-013** — the submitter is a reference, never an address. The body
//!    carries the submission id and nothing else about the person. An operator
//!    can map it back; the tracker cannot, and neither can anyone who reads the
//!    repository.
//! 3. **FR-019** — the trailing `delivery_key` comment is load-bearing. It is
//!    what a retry after an ambiguous failure searches for, and what makes a
//!    duplicate — if the one unavoidable case ever produces one — findable
//!    rather than silent. `lore_sync/binding.rs` recognises its own issue the
//!    same way.

use uuid::Uuid;

use super::{AttachmentKind, Kind};

/// The marker a search looks for. Kept as a constant because two places have
/// to agree on it — the body that writes it and the query that finds it — and
/// a literal in each is how they stop agreeing.
pub const KEY_MARKER: &str = "thunderforge-feedback";

/// How much log the body carries inline.
///
/// GitHub caps an issue body at 65,536 characters. 48 KB of log leaves room
/// for the message, the table and the links without the host truncating the
/// end, which is where the marker lives. The complete bundle is the committed
/// file, so nothing is lost by the cut — but the summary line says what was
/// withheld, because FR-011's "the person is told what was kept" is the same
/// sentence answering a second question.
pub const INLINE_LOG_LIMIT: usize = 48 * 1024;

/// What the app already knows about where somebody was. Every field is
/// optional except the ones a submission cannot be without, because spec.md's
/// first Edge Case is a submission from a screen with no world at all.
#[derive(Debug, Clone, Default)]
pub struct Context {
    pub screen_path: Option<String>,
    pub world_name: Option<String>,
    pub game_system_id: Option<String>,
    pub client_version: String,
    pub server_version: String,
    pub browser: String,
    pub submitted_at: String,
}

/// One attachment as it exists at the destination.
#[derive(Debug, Clone)]
pub struct AttachedFile {
    pub kind: AttachmentKind,
    /// The blob page. Always usable, public repository or not.
    pub blob_url: String,
    /// The raw URL, present only when embedding is the right choice — which is
    /// decided from the visibility this instance actually observed, not from a
    /// guess.
    pub embed_url: Option<String>,
    pub entries_kept: Option<i32>,
    pub entries_dropped: Option<i32>,
    pub redaction_count: i32,
}

/// The title, with the kind in front of it.
///
/// The prefix is redundant with the label on purpose: a label is invisible in
/// a notification email and in half of GitHub's own listings, and FR-020 asks
/// that the kind be readable without opening the body.
pub fn title(kind: Kind, summary: Option<&str>, message: &str) -> String {
    let prefix = match kind {
        Kind::Issue => "[Issue]",
        Kind::FeatureRequest => "[Feature]",
        Kind::General => "[Feedback]",
    };
    // A general message has no summary field by contract, so its title comes
    // from the first line of what the person wrote — trimmed to something a
    // list can render, and never empty.
    let line = summary
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| first_line(message));
    format!("{prefix} {line}")
}

fn first_line(message: &str) -> String {
    let line = message.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return "Feedback".to_string();
    }
    if trimmed.chars().count() <= 80 {
        return trimmed.to_string();
    }
    let cut: String = trimmed.chars().take(77).collect();
    format!("{cut}…")
}

/// `feedback` on every issue, plus exactly one kind label (FR-020).
///
/// Two labels rather than one so a maintainer can watch the whole feature and
/// a triager can filter to bugs, without either of them needing to know the
/// other's query.
pub fn labels(kind: Kind) -> Vec<String> {
    let specific = match kind {
        Kind::Issue => "feedback:issue",
        Kind::FeatureRequest => "feedback:feature-request",
        Kind::General => "feedback:general",
    };
    vec!["feedback".to_string(), specific.to_string()]
}

/// The whole body.
///
/// The person's message comes first and verbatim — it is not redacted, not
/// reflowed and not summarised, because spec.md's Edge Cases settle that what
/// somebody deliberately types is theirs.
pub fn body(
    message: &str,
    context: &Context,
    submission_id: Uuid,
    delivery_key: Uuid,
    files: &[AttachedFile],
    log_text: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str(message.trim());
    out.push_str("\n\n---\n\n### Context\n\n| | |\n|---|---|\n");

    push_row(&mut out, "Screen", context.screen_path.as_deref().map(code));
    push_row(&mut out, "World", world_cell(context));
    push_row(
        &mut out,
        "App",
        Some(format!(
            "web {} · server {}",
            context.client_version, context.server_version
        )),
    );
    push_row(&mut out, "Browser", Some(context.browser.clone()));
    push_row(&mut out, "Submitted", Some(context.submitted_at.clone()));
    // FR-013. The submission id, and nothing else about the person: an
    // operator can map it back, and the tracker cannot.
    push_row(
        &mut out,
        "Submitter",
        Some(format!(
            "`{submission_id}` — an instance reference, not an identity"
        )),
    );

    if let Some(shot) = files.iter().find(|f| f.kind == AttachmentKind::Screenshot) {
        out.push_str("\n### Screenshot\n\n");
        match &shot.embed_url {
            // A public repository renders it inline, which is "ordinary issue
            // content" in the fullest sense US3 asks for.
            Some(url) => out.push_str(&format!("![screenshot]({url})\n")),
            // A private one gets a link: raw.githubusercontent.com needs a
            // token, and a link that works beats an image that renders broken
            // for every maintainer.
            None => out.push_str(&format!("[screenshot]({})\n", shot.blob_url)),
        }
    }

    if let Some(logs) = files.iter().find(|f| f.kind == AttachmentKind::Logs) {
        out.push_str("\n<details><summary>Browser logs");
        out.push_str(&log_summary(logs, log_text));
        out.push_str("</summary>\n\n```\n");
        if let Some(text) = log_text {
            out.push_str(&truncate_log(text));
            if !text.ends_with('\n') {
                out.push('\n');
            }
        }
        out.push_str("```\n\n</details>\n\n");
        out.push_str(&format!("Full log: [{}]({})\n", "logs.log", logs.blob_url));
    }

    // Load-bearing, and last: what a retry searches for. An HTML comment so it
    // is invisible to a reader and exact for a query.
    out.push_str(&format!("\n<!-- {KEY_MARKER}: {delivery_key} -->\n"));
    out
}

/// The one line FR-011 asks for: what was kept, what was dropped, and how many
/// times the client's filter fired. A count, not a shrug.
fn log_summary(logs: &AttachedFile, log_text: Option<&str>) -> String {
    let mut parts = Vec::new();
    if let Some(kept) = logs.entries_kept {
        parts.push(format!("{kept} entries kept"));
    }
    if let Some(dropped) = logs.entries_dropped.filter(|d| *d > 0) {
        parts.push(format!("{dropped} dropped for size"));
    }
    parts.push(format!("{} redactions", logs.redaction_count));
    if log_text.is_some_and(|t| t.len() > INLINE_LOG_LIMIT) {
        parts.push("truncated here, complete in the linked file".to_string());
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" — {}", parts.join(", "))
    }
}

/// The inline block, cut at the limit on a line boundary so the last entry in
/// the body is a whole one rather than half of one.
fn truncate_log(text: &str) -> String {
    if text.len() <= INLINE_LOG_LIMIT {
        return text.to_string();
    }
    let mut cut = INLINE_LOG_LIMIT;
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    let head = &text[..cut];
    match head.rfind('\n') {
        Some(last) => head[..last].to_string(),
        None => head.to_string(),
    }
}

fn world_cell(context: &Context) -> Option<String> {
    let name = context.world_name.as_deref()?;
    Some(match context.game_system_id.as_deref() {
        Some(system) => format!("{name} ({system})"),
        None => name.to_string(),
    })
}

fn code(value: &str) -> String {
    format!("`{value}`")
}

/// A row that says "none" rather than being absent.
///
/// An absent row and an empty one read the same to a maintainer, and the
/// difference matters: "no world" is a fact about where somebody was, and a
/// missing row is a fact about this product's rendering.
fn push_row(out: &mut String, label: &str, value: Option<String>) {
    let rendered = value.unwrap_or_else(|| "—".to_string());
    out.push_str(&format!("| {label} | {rendered} |\n"));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_context() -> Context {
        Context {
            screen_path: Some("/world/abc/compendium".to_string()),
            world_name: Some("Ashfall".to_string()),
            game_system_id: Some("dnd5e".to_string()),
            client_version: "1.4.2".to_string(),
            server_version: "1.4.2 (a1b2c3d)".to_string(),
            browser: "Chrome on Linux".to_string(),
            submitted_at: "2026-09-07T14:02:11Z".to_string(),
        }
    }

    /// FR-020, and the reason there are two labels: one to watch the feature,
    /// one to triage a kind.
    #[test]
    fn each_kind_is_distinguishable_without_reading_the_body() {
        assert_eq!(
            labels(Kind::Issue),
            vec!["feedback".to_string(), "feedback:issue".to_string()]
        );
        assert_eq!(
            labels(Kind::FeatureRequest),
            vec![
                "feedback".to_string(),
                "feedback:feature-request".to_string()
            ]
        );
        assert_eq!(
            labels(Kind::General),
            vec!["feedback".to_string(), "feedback:general".to_string()]
        );
        assert!(title(Kind::Issue, Some("Tokens vanish"), "").starts_with("[Issue] "));
        assert!(title(Kind::FeatureRequest, Some("Reorder"), "").starts_with("[Feature] "));
        assert!(title(Kind::General, None, "Thanks for the search").starts_with("[Feedback] "));
    }

    /// A general message has no summary by contract, so its title has to come
    /// from somewhere — and an issue list full of `[Feedback]` with nothing
    /// after it is a list nobody can work.
    #[test]
    fn a_message_with_no_summary_still_gets_a_title_somebody_can_read() {
        assert_eq!(
            title(Kind::General, None, "\n\nThe compendium search is lovely\n"),
            "[Feedback] The compendium search is lovely"
        );
        assert_eq!(title(Kind::General, None, "   "), "[Feedback] Feedback");
        let long = "x".repeat(200);
        assert!(title(Kind::General, None, &long).chars().count() <= 90);
    }

    /// FR-013, stated as an assertion rather than as an intention: nothing
    /// about the person reaches the destination except a reference the
    /// instance can map back.
    #[test]
    fn the_submitter_is_a_reference_and_never_an_address() {
        let id = Uuid::now_v7();
        let out = body(
            "Tokens vanish when the scene changes",
            &a_context(),
            id,
            Uuid::now_v7(),
            &[],
            None,
        );
        assert!(out.contains(&id.to_string()));
        assert!(!out.contains('@'), "{out}");
        assert!(out.contains("an instance reference, not an identity"));
    }

    /// FR-019's handle. Without it a retry has nothing to search for and a
    /// duplicate is silent rather than findable.
    #[test]
    fn the_delivery_key_is_written_where_a_search_can_find_it() {
        let key = Uuid::now_v7();
        let out = body("hello", &a_context(), Uuid::now_v7(), key, &[], None);
        assert!(
            out.trim_end()
                .ends_with(&format!("<!-- {KEY_MARKER}: {key} -->"))
        );
    }

    #[test]
    fn a_submission_from_a_screen_with_no_world_still_renders_a_table() {
        let context = Context {
            screen_path: None,
            world_name: None,
            game_system_id: None,
            ..a_context()
        };
        let out = body(
            "it broke",
            &context,
            Uuid::now_v7(),
            Uuid::now_v7(),
            &[],
            None,
        );
        assert!(out.contains("| World | — |"), "{out}");
        assert!(out.contains("| Screen | — |"), "{out}");
    }

    /// The image is embedded for a public repository and linked for a private
    /// one, and the decision is carried in, not made here — this asserts the
    /// rendering honours it either way.
    #[test]
    fn visibility_decides_between_an_embed_and_a_link() {
        let public = AttachedFile {
            kind: AttachmentKind::Screenshot,
            blob_url: "https://github.com/o/n/blob/b/p".to_string(),
            embed_url: Some("https://raw.githubusercontent.com/o/n/b/p".to_string()),
            entries_kept: None,
            entries_dropped: None,
            redaction_count: 0,
        };
        let private = AttachedFile {
            embed_url: None,
            ..public.clone()
        };
        let rendered = |f: AttachedFile| {
            body(
                "m",
                &a_context(),
                Uuid::now_v7(),
                Uuid::now_v7(),
                &[f],
                None,
            )
        };
        assert!(rendered(public).contains("![screenshot](https://raw.githubusercontent.com"));
        let linked = rendered(private);
        assert!(linked.contains("[screenshot](https://github.com/o/n/blob/b/p)"));
        assert!(!linked.contains("!["));
    }

    /// The inline block is bounded and says what it withheld. A chatty session
    /// exceeds the host's body limit, and the truncated half is frequently the
    /// interesting half — which is why the complete bundle is a linked file
    /// and the summary says to go and read it.
    #[test]
    fn the_inline_log_is_cut_and_the_cut_is_stated() {
        let logs = AttachedFile {
            kind: AttachmentKind::Logs,
            blob_url: "https://github.com/o/n/blob/b/logs.log".to_string(),
            embed_url: None,
            entries_kept: Some(214),
            entries_dropped: Some(61),
            redaction_count: 3,
        };
        let long = "a line of log output\n".repeat(6000);
        assert!(long.len() > INLINE_LOG_LIMIT);
        let out = body(
            "m",
            &a_context(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            &[logs],
            Some(&long),
        );
        assert!(out.contains("214 entries kept"), "{out}");
        assert!(out.contains("61 dropped for size"), "{out}");
        assert!(out.contains("3 redactions"), "{out}");
        assert!(out.contains("truncated here, complete in the linked file"));
        assert!(out.contains("Full log: [logs.log]"));
        assert!(out.len() < 65_536, "the body must fit the host's limit");
    }

    /// A short log is not cut, and is not reported as if it had been.
    #[test]
    fn a_short_log_is_carried_whole_and_says_nothing_about_truncation() {
        let logs = AttachedFile {
            kind: AttachmentKind::Logs,
            blob_url: "https://github.com/o/n/blob/b/logs.log".to_string(),
            embed_url: None,
            entries_kept: Some(2),
            entries_dropped: Some(0),
            redaction_count: 0,
        };
        let text = "one\ntwo\n";
        let out = body(
            "m",
            &a_context(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            &[logs],
            Some(text),
        );
        assert!(out.contains("one\ntwo\n"));
        assert!(!out.contains("truncated here"));
        assert!(!out.contains("dropped for size"));
    }
}
