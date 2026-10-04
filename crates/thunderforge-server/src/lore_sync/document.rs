//! Spec 034 (`contracts/repository-file-format.md`, FR-009 to FR-013): the
//! shape of one exported file, and the parse that reads it back.
//!
//! This module is the only artefact of the feature that outlives the
//! platform's involvement — it is what a Game Master holding a clone actually
//! has — so it is written to one governing rule: **the body is the entry's
//! markdown exactly as authored.** No reformatting, no normalisation, no
//! prettifying, no trailing-newline tidying. SC-008's byte-identical round trip
//! is the test of that rule, and every convenience that would make the output
//! "nicer" fails it. If a change here makes a diff read better, it is almost
//! certainly wrong.
//!
//! Two consequences worth stating, because both look like omissions:
//!
//! * **The front matter is emitted, not serialised by a YAML library.** A YAML
//!   emitter is entitled to re-wrap, re-quote and re-order at will, and a
//!   round trip that survives only by accident of the library's current mood is
//!   not a round trip. The five fields are fixed and the emitter here is
//!   explicit about each. It is YAML a reader can read, which is what the
//!   contract asks for; it is not a general YAML document.
//! * **A cross-link to an actor, item or ability is left byte-for-byte alone.**
//!   FR-013 requires it to stay readable in the body *and* be recorded in the
//!   header, and to survive a round trip "without being silently dropped or
//!   converted into a broken lore link". Leaving the authored `[[Ser Willem]]`
//!   in place does all three: it names the target, it is not a link that
//!   resolves to a file that does not exist, and it comes back out identical.
//!   Rewriting it to bare prose would satisfy "readable" and lose the ability
//!   to ever put it back — the header records where it went, not where it is.
//!
//! Pure: no database, no filesystem, no clock. Link resolution — which title
//! is a lore entry and which is an actor — is a query, so it arrives as a
//! closure the caller supplies. That is also what makes the tests below able
//! to state the interesting cases without a database.

use std::collections::HashSet;
use std::fmt;

use chrono::{DateTime, SecondsFormat, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use uuid::Uuid;

use super::paths::relative_link;

/// `[[Title]]` / `[[Title|Display]]`, matching `markdown::links`'s pattern
/// exactly. The two must agree: a link the app resolved and this module did not
/// would silently disappear from the mirror, and the app's pattern is the
/// definition of what an author can write.
static WIKI_LINK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").expect("valid regex"));

/// An inline markdown link with a destination containing no whitespace or
/// parentheses — which is every link this module writes, because path
/// components are `[a-z0-9-]` only. Anything more exotic in a hand-edited file
/// is left alone rather than guessed at.
static INLINE_LINK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[([^\]\[]*)\]\(([^()\s]+)\)").expect("valid regex"));

/// What a `[[Title]]` in an entry's body turns out to point at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    /// Another lore entry in the same world (FR-012). Carries that entry's
    /// path relative to the synchronisation directory; the rewrite makes it
    /// relative to the linking file.
    Lore { path: String },
    /// An actor, item or ability (FR-013). Cannot resolve in a repository that
    /// contains only lore, so it is left readable and recorded.
    Unresolvable { kind: UnresolvableKind },
}

/// The kinds of target a lore-only mirror cannot represent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnresolvableKind {
    Actor,
    Item,
    Ability,
}

impl UnresolvableKind {
    /// The token written to and read from the header's `kind` field.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Actor => "actor",
            Self::Item => "item",
            Self::Ability => "ability",
        }
    }

    /// Parses the header token. Unknown kinds are rejected rather than mapped
    /// to a default: a file naming a kind this build does not know is a file
    /// written by a newer build, and quietly relabelling it would be a worse
    /// answer than saying so.
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "actor" => Some(Self::Actor),
            "item" => Some(Self::Item),
            "ability" => Some(Self::Ability),
            _ => None,
        }
    }
}

/// One declared loss of fidelity, as it appears under `unresolvable_links`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvableLink {
    /// The target's name as the body names it.
    pub text: String,
    pub kind: UnresolvableKind,
}

/// The file's front matter — FR-009's minimum, plus FR-013's record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentHeader {
    /// The durable entry identifier. **The only field used for matching.** The
    /// path is a label; this is the key (research.md R7, FR-027).
    pub id: Uuid,
    pub title: String,
    pub tags: Vec<String>,
    /// The time of the revision this file represents.
    pub updated: DateTime<Utc>,
    pub unresolvable_links: Vec<UnresolvableLink>,
}

/// A file split back into its parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDocument {
    pub header: DocumentHeader,
    /// The body exactly as the file holds it — still in repository form, with
    /// lore links as relative paths. [`restore_links`] returns it to authored
    /// form.
    pub body: String,
}

/// Why a file could not be read as an exported document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentError {
    /// The file does not open with a front-matter delimiter. On import this is
    /// FR-027's "a file with no recognised identifier is a proposed new entry",
    /// not a failure — the caller decides that, this module only reports it.
    MissingFrontMatter,
    /// The opening delimiter is never closed.
    UnterminatedFrontMatter,
    /// A field FR-009 requires is absent.
    MissingField(&'static str),
    /// A field is present but unreadable.
    InvalidField { field: &'static str, detail: String },
}

impl fmt::Display for DocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFrontMatter => write!(formatter, "no front matter"),
            Self::UnterminatedFrontMatter => write!(formatter, "front matter is never closed"),
            Self::MissingField(field) => write!(formatter, "missing required field `{field}`"),
            Self::InvalidField { field, detail } => {
                write!(formatter, "field `{field}` is unreadable: {detail}")
            }
        }
    }
}

impl std::error::Error for DocumentError {}

/// Rewrites an entry's authored markdown into repository form.
///
/// `self_path` is the linking entry's own path relative to the synchronisation
/// directory, needed because FR-012's links are relative to the file that
/// carries them — that is what makes a clone navigable with no network
/// (SC-011).
///
/// `resolve` answers what one `[[Title]]` points at, by the same
/// case-insensitive title match the app itself uses; returning `None` means the
/// link was already broken in the app, and a broken link is preserved verbatim
/// rather than repaired, invented or recorded. Recording it would put a
/// nonexistent actor in the header and claim a fidelity loss that did not
/// happen.
///
/// Returns the rewritten body and the header's `unresolvable_links`,
/// deduplicated by target and in first-appearance order: the header is a list
/// of what could not be carried, not a concordance of every mention.
pub fn rewrite_links_for_export<F>(
    markdown: &str,
    self_path: &str,
    resolve: F,
) -> (String, Vec<UnresolvableLink>)
where
    F: Fn(&str) -> Option<LinkTarget>,
{
    let mut unresolvable = Vec::new();
    let mut seen: HashSet<(String, UnresolvableKind)> = HashSet::new();

    let body = WIKI_LINK
        .replace_all(markdown, |caps: &regex::Captures| {
            let whole = caps[0].to_string();
            let title = caps[1].trim().to_string();
            let display = caps
                .get(2)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_else(|| title.clone());

            match resolve(&title) {
                Some(LinkTarget::Lore { path }) => {
                    format!("[{}]({})", display, relative_link(self_path, &path))
                }
                Some(LinkTarget::Unresolvable { kind }) => {
                    if seen.insert((title.clone(), kind)) {
                        unresolvable.push(UnresolvableLink { text: title, kind });
                    }
                    whole
                }
                None => whole,
            }
        })
        .into_owned();

    (body, unresolvable)
}

/// The inverse of [`rewrite_links_for_export`]: repository form back to
/// authored form.
///
/// `resolve_destination` is handed each inline link's destination exactly as
/// written and returns the target entry's title if — and only if — that
/// destination is a lore file this world exported. Anything it does not claim
/// is left untouched, so an author's own relative link to a hand-written file
/// in the repository survives a round trip unmolested.
///
/// A `[[Title]]` whose target was an actor, item or ability never became an
/// inline link in the first place, so there is nothing here to undo — which is
/// exactly why FR-013's round trip holds.
pub fn restore_links<F>(body: &str, resolve_destination: F) -> String
where
    F: Fn(&str) -> Option<String>,
{
    let bytes = body.as_bytes();
    INLINE_LINK
        .replace_all(body, |caps: &regex::Captures| {
            let whole = caps[0].to_string();
            let start = caps.get(0).expect("group 0 always matches").start();
            // `![alt](dest)` is an image, not a link. No exported image path is
            // a lore destination, but the guard is cheaper than the bug.
            if start > 0 && bytes[start - 1] == b'!' {
                return whole;
            }
            let display = caps[1].to_string();
            match resolve_destination(&caps[2]) {
                Some(title) if display == title => format!("[[{title}]]"),
                Some(title) => format!("[[{title}|{display}]]"),
                None => whole,
            }
        })
        .into_owned()
}

/// Renders a header and an already-rewritten body into the file's exact bytes.
///
/// `body` is written verbatim after a single blank line. It is not trimmed and
/// no trailing newline is added: a body that ends mid-line ends mid-line in the
/// file, because that is what the author wrote.
pub fn render(header: &DocumentHeader, body: &str) -> String {
    let mut out = String::with_capacity(body.len() + 256);
    out.push_str("---\n");
    out.push_str(&format!("id: {}\n", header.id));
    out.push_str(&format!("title: {}\n", emit_scalar(&header.title)));
    out.push_str(&format!(
        "tags: [{}]\n",
        header
            .tags
            .iter()
            .map(|tag| emit_scalar(tag))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    out.push_str(&format!(
        "updated: {}\n",
        header.updated.to_rfc3339_opts(SecondsFormat::Secs, true)
    ));
    if header.unresolvable_links.is_empty() {
        out.push_str("unresolvable_links: []\n");
    } else {
        out.push_str("unresolvable_links:\n");
        for link in &header.unresolvable_links {
            out.push_str(&format!("  - text: {}\n", quote(&link.text)));
            out.push_str(&format!("    kind: {}\n", link.kind.as_str()));
        }
    }
    out.push_str("---\n\n");
    out.push_str(body);
    out
}

/// Splits a file back into its header and its body.
///
/// The body is returned byte-for-byte from the first character after the blank
/// line that follows the closing delimiter. Exactly one newline is consumed
/// there, which keeps the split unambiguous for a body that itself begins with
/// a blank line — the case that would otherwise quietly eat a character on
/// every round trip.
pub fn parse(file: &str) -> Result<ParsedDocument, DocumentError> {
    let rest = file
        .strip_prefix("---\n")
        .or_else(|| file.strip_prefix("---\r\n"))
        .ok_or(DocumentError::MissingFrontMatter)?;

    let mut offset = 0usize;
    let mut boundary = None;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches('\n').trim_end_matches('\r') == "---" {
            boundary = Some((offset, offset + line.len()));
            break;
        }
        offset += line.len();
    }
    let (header_end, body_start) = boundary.ok_or(DocumentError::UnterminatedFrontMatter)?;

    let header = parse_header(&rest[..header_end])?;
    let after = &rest[body_start..];
    let body = after
        .strip_prefix('\n')
        .or_else(|| after.strip_prefix("\r\n"))
        .unwrap_or(after);

    Ok(ParsedDocument {
        header,
        body: body.to_string(),
    })
}

fn parse_header(text: &str) -> Result<DocumentHeader, DocumentError> {
    let mut id = None;
    let mut title = None;
    let mut tags = None;
    let mut updated = None;
    // Kind arrives on the line after text, so it is optional until the
    // sequence is complete; a `text` that never got one is a malformed file,
    // not an actor by default.
    let mut link_items: Vec<(String, Option<UnresolvableKind>)> = Vec::new();
    let mut in_links = false;

    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }

        if in_links && line.starts_with(' ') {
            let item = line.trim_start();
            if let Some(value) = item.strip_prefix("- text:") {
                link_items.push((read_scalar(value), None));
                continue;
            }
            if let Some(value) = item.strip_prefix("kind:") {
                let token = read_scalar(value);
                let kind =
                    UnresolvableKind::parse(&token).ok_or_else(|| DocumentError::InvalidField {
                        field: "unresolvable_links",
                        detail: format!("unknown kind `{token}`"),
                    })?;
                match link_items.last_mut() {
                    Some(last) => last.1 = Some(kind),
                    None => {
                        return Err(DocumentError::InvalidField {
                            field: "unresolvable_links",
                            detail: "a `kind` with no `text` before it".to_string(),
                        });
                    }
                }
                continue;
            }
            return Err(DocumentError::InvalidField {
                field: "unresolvable_links",
                detail: format!("unreadable entry `{item}`"),
            });
        }
        in_links = false;

        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        match key.trim() {
            "id" => id = Some(read_scalar(value)),
            "title" => title = Some(read_scalar(value)),
            "tags" => tags = Some(parse_flow_sequence(value)?),
            "updated" => updated = Some(read_scalar(value)),
            "unresolvable_links" => {
                let inline = read_scalar(value);
                if !inline.is_empty() && inline != "[]" {
                    return Err(DocumentError::InvalidField {
                        field: "unresolvable_links",
                        detail: format!("expected a block sequence or `[]`, found `{inline}`"),
                    });
                }
                in_links = inline.is_empty();
            }
            _ => {}
        }
    }

    let unresolvable_links = link_items
        .into_iter()
        .map(|(text, kind)| match kind {
            Some(kind) => Ok(UnresolvableLink { text, kind }),
            None => Err(DocumentError::InvalidField {
                field: "unresolvable_links",
                detail: format!("`{text}` has no `kind`"),
            }),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let id = id.ok_or(DocumentError::MissingField("id"))?;
    let id = Uuid::parse_str(&id).map_err(|error| DocumentError::InvalidField {
        field: "id",
        detail: error.to_string(),
    })?;
    let updated = updated.ok_or(DocumentError::MissingField("updated"))?;
    let updated = DateTime::parse_from_rfc3339(&updated)
        .map_err(|error| DocumentError::InvalidField {
            field: "updated",
            detail: error.to_string(),
        })?
        .with_timezone(&Utc);

    Ok(DocumentHeader {
        id,
        title: title.ok_or(DocumentError::MissingField("title"))?,
        tags: tags.unwrap_or_default(),
        updated,
        unresolvable_links,
    })
}

/// Reads one `[a, b, "c, d"]` flow sequence. Quoted members may contain the
/// separator, which is the only reason this cannot be a `split(',')`.
fn parse_flow_sequence(value: &str) -> Result<Vec<String>, DocumentError> {
    let trimmed = value.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .ok_or_else(|| DocumentError::InvalidField {
            field: "tags",
            detail: format!("expected a flow sequence, found `{trimmed}`"),
        })?;

    let mut items = Vec::new();
    let mut current = String::new();
    let mut quote_char: Option<char> = None;
    let mut escaped = false;

    for character in inner.chars() {
        match quote_char {
            Some(quote) => {
                current.push(character);
                if escaped {
                    escaped = false;
                } else if character == '\\' && quote == '"' {
                    escaped = true;
                } else if character == quote {
                    quote_char = None;
                }
            }
            None => {
                if character == ',' {
                    items.push(current.clone());
                    current.clear();
                    continue;
                }
                if character == '"' || character == '\'' {
                    quote_char = Some(character);
                }
                current.push(character);
            }
        }
    }
    if !current.trim().is_empty() || !items.is_empty() {
        items.push(current);
    }

    // A blank member is `[]`'s or a trailing comma's leftover, never a tag; an
    // explicitly quoted empty string is a tag and survives.
    Ok(items
        .iter()
        .filter(|item| !item.trim().is_empty())
        .map(|item| read_scalar(item))
        .collect())
}

/// Reads one scalar, honouring double and single quoting.
///
/// Single quoting is accepted but never written: a Game Master editing a title
/// by hand may reasonably reach for it, and refusing to read what a person
/// plausibly typed would make the format hostile for no gain.
fn read_scalar(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(inner) = trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(character) = chars.next() {
            if character != '\\' {
                out.push(character);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        }
        return out;
    }
    if let Some(inner) = trimmed
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
    {
        return inner.replace("''", "'");
    }
    trimmed.to_string()
}

/// Emits a scalar, quoting only where a reader would otherwise misread it.
///
/// Unquoted where it is safe, because `title: The Red Keep` is what the
/// contract shows and what a human reading a diff wants to see.
fn emit_scalar(value: &str) -> String {
    if needs_quoting(value) {
        quote(value)
    } else {
        value.to_string()
    }
}

fn needs_quoting(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    if value.trim() != value {
        return true;
    }
    if value.contains([
        ':', '#', ',', '[', ']', '{', '}', '"', '\'', '\n', '\r', '\t',
    ]) {
        return true;
    }
    if value.starts_with(['-', '?', '&', '*', '!', '|', '>', '%', '@', '`']) {
        return true;
    }
    // Tokens YAML would read as something other than a string.
    let lowered = value.to_ascii_lowercase();
    if matches!(
        lowered.as_str(),
        "true" | "false" | "null" | "yes" | "no" | "on" | "off" | "~"
    ) {
        return true;
    }
    value.parse::<f64>().is_ok()
}

fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> DocumentHeader {
        DocumentHeader {
            id: Uuid::parse_str("01a06d8f-5236-76d1-af6b-cd5d71dfbf7c").expect("valid uuid"),
            title: "The Red Keep".to_string(),
            tags: vec!["location".to_string(), "ruined".to_string()],
            updated: DateTime::parse_from_rfc3339("2026-09-04T18:15:08Z")
                .expect("valid timestamp")
                .with_timezone(&Utc),
            unresolvable_links: Vec::new(),
        }
    }

    /// Resolves the fixture world used throughout: one lore entry, one actor,
    /// one item, one ability, and nothing else.
    fn resolver(title: &str) -> Option<LinkTarget> {
        match title {
            "The Black Cells" => Some(LinkTarget::Lore {
                path: "westeros/the-red-keep/the-black-cells.md".to_string(),
            }),
            "Winterfell" => Some(LinkTarget::Lore {
                path: "the-north/winterfell.md".to_string(),
            }),
            "Ser Willem" => Some(LinkTarget::Unresolvable {
                kind: UnresolvableKind::Actor,
            }),
            "Widow's Wail" => Some(LinkTarget::Unresolvable {
                kind: UnresolvableKind::Item,
            }),
            "Wildfire" => Some(LinkTarget::Unresolvable {
                kind: UnresolvableKind::Ability,
            }),
            _ => None,
        }
    }

    fn destination_resolver(destination: &str) -> Option<String> {
        match destination {
            "the-black-cells.md" => Some("The Black Cells".to_string()),
            "../../the-north/winterfell.md" => Some("Winterfell".to_string()),
            _ => None,
        }
    }

    /// The round trip SC-008 requires, end to end: authored markdown out to a
    /// file and back, byte for byte.
    fn assert_round_trip(authored: &str) {
        let self_path = "westeros/the-red-keep/the-red-keep-notes.md";
        let (body, unresolvable) = rewrite_links_for_export(authored, self_path, resolver);
        let mut head = header();
        head.unresolvable_links = unresolvable;

        let file = render(&head, &body);
        let parsed = parse(&file).expect("file parses");
        let restored = restore_links(&parsed.body, destination_resolver);

        assert_eq!(
            restored, authored,
            "round trip was not byte-identical\n--- file ---\n{file}"
        );
        assert_eq!(parsed.header, head);
    }

    #[test]
    fn the_body_survives_a_round_trip_byte_for_byte() {
        assert_round_trip(
            "# The Red Keep\n\nThe keep has stood since   the Conquest.\t\n\n\
             - a list\n-   badly aligned\n\n> a quote\n\n```rust\nlet x = 1;\n```\n",
        );
    }

    #[test]
    fn markdown_a_prettifier_would_touch_is_left_alone() {
        // Every line here is something a formatter would "fix". None of it may
        // change: FR-011 says as authored, and this is what that means.
        assert_round_trip(
            "Heading\n=======\n\n*  item with two spaces\n+ a plus bullet\n\n\
             | a | b |\n|---|--|\n| 1 |2 |\n\n\
             trailing whitespace here   \n\n\n\nfour blank lines above\n\
             no trailing newline at the end",
        );
    }

    #[test]
    fn a_body_beginning_with_a_blank_line_keeps_it() {
        // The separator between front matter and body is exactly one newline,
        // and this is the case that proves it is not two.
        assert_round_trip("\n\nThe keep has stood since the Conquest.\n");
    }

    #[test]
    fn an_empty_body_round_trips() {
        assert_round_trip("");
    }

    #[test]
    fn a_body_with_windows_line_endings_is_preserved() {
        assert_round_trip("# The Red Keep\r\n\r\nIt has stood since the Conquest.\r\n");
    }

    #[test]
    fn a_lore_link_becomes_a_relative_path_and_comes_back() {
        let authored = "Down to [[The Black Cells]], then north to [[Winterfell]].\n";
        let (body, unresolvable) = rewrite_links_for_export(
            authored,
            "westeros/the-red-keep/the-red-keep-notes.md",
            resolver,
        );

        assert_eq!(
            body,
            "Down to [The Black Cells](the-black-cells.md), then north to \
             [Winterfell](../../the-north/winterfell.md).\n"
        );
        assert!(unresolvable.is_empty());
        assert_eq!(restore_links(&body, destination_resolver), authored);
    }

    #[test]
    fn a_lore_link_with_display_text_keeps_the_alias_in_both_directions() {
        let authored = "Down to [[The Black Cells|the cells]].\n";
        let (body, _) = rewrite_links_for_export(
            authored,
            "westeros/the-red-keep/the-red-keep-notes.md",
            resolver,
        );

        assert_eq!(body, "Down to [the cells](the-black-cells.md).\n");
        assert_eq!(restore_links(&body, destination_resolver), authored);
        assert_round_trip(authored);
    }

    #[test]
    fn an_unresolvable_link_stays_in_the_body_and_is_recorded() {
        let authored = "[[Ser Willem]] carries [[Widow's Wail]] and casts [[Wildfire]].\n";
        let (body, unresolvable) = rewrite_links_for_export(authored, "a.md", resolver);

        // FR-013: readable text, unchanged, and not a link to a file that does
        // not exist.
        assert_eq!(body, authored);
        assert!(!body.contains("]("));
        assert_eq!(
            unresolvable,
            vec![
                UnresolvableLink {
                    text: "Ser Willem".to_string(),
                    kind: UnresolvableKind::Actor,
                },
                UnresolvableLink {
                    text: "Widow's Wail".to_string(),
                    kind: UnresolvableKind::Item,
                },
                UnresolvableLink {
                    text: "Wildfire".to_string(),
                    kind: UnresolvableKind::Ability,
                },
            ]
        );
    }

    #[test]
    fn an_unresolvable_link_survives_a_round_trip_without_being_dropped() {
        let authored =
            "[[Ser Willem]] met [[The Black Cells|the cells]], carrying [[Widow's Wail]].\n";
        assert_round_trip(authored);

        let (body, unresolvable) = rewrite_links_for_export(authored, "a.md", resolver);
        let mut head = header();
        head.unresolvable_links = unresolvable;
        let parsed = parse(&render(&head, &body)).expect("parses");

        assert_eq!(parsed.header.unresolvable_links.len(), 2);
        assert!(parsed.body.contains("[[Ser Willem]]"));
        assert!(parsed.body.contains("[[Widow's Wail]]"));
    }

    #[test]
    fn a_repeated_unresolvable_target_is_recorded_once() {
        let (_, unresolvable) = rewrite_links_for_export(
            "[[Ser Willem]] and again [[Ser Willem]] and [[Ser Willem|the knight]].\n",
            "a.md",
            resolver,
        );
        assert_eq!(unresolvable.len(), 1);
    }

    #[test]
    fn a_link_broken_in_the_app_stays_broken_rather_than_being_invented() {
        let authored = "See [[A Place That Was Deleted]].\n";
        let (body, unresolvable) = rewrite_links_for_export(authored, "a.md", resolver);

        assert_eq!(body, authored);
        assert!(
            unresolvable.is_empty(),
            "a link that resolved to nothing is not a fidelity loss"
        );
        assert_round_trip(authored);
    }

    #[test]
    fn an_authors_own_relative_link_is_not_claimed_on_the_way_back() {
        let body = "See [the readme](../README.md) and [notes](notes.md).\n";
        assert_eq!(restore_links(body, destination_resolver), body);
    }

    #[test]
    fn an_image_is_never_mistaken_for_a_lore_link() {
        let body = "![the cells](the-black-cells.md)\n";
        assert_eq!(restore_links(body, destination_resolver), body);
    }

    #[test]
    fn the_rendered_file_matches_the_contract() {
        let mut head = header();
        head.unresolvable_links = vec![UnresolvableLink {
            text: "Ser Willem".to_string(),
            kind: UnresolvableKind::Actor,
        }];

        assert_eq!(
            render(&head, "The keep has stood since...\n"),
            "---\n\
             id: 01a06d8f-5236-76d1-af6b-cd5d71dfbf7c\n\
             title: The Red Keep\n\
             tags: [location, ruined]\n\
             updated: 2026-09-04T18:15:08Z\n\
             unresolvable_links:\n  \
             - text: \"Ser Willem\"\n    \
             kind: actor\n\
             ---\n\n\
             The keep has stood since...\n"
        );
    }

    #[test]
    fn rendering_is_stable_across_runs() {
        let head = header();
        assert_eq!(render(&head, "body\n"), render(&head, "body\n"));
    }

    #[test]
    fn a_header_round_trips_through_render_and_parse() {
        let mut head = header();
        head.unresolvable_links = vec![
            UnresolvableLink {
                text: "Ser Willem".to_string(),
                kind: UnresolvableKind::Actor,
            },
            UnresolvableLink {
                text: "Wildfire".to_string(),
                kind: UnresolvableKind::Ability,
            },
        ];
        let parsed = parse(&render(&head, "body")).expect("parses");
        assert_eq!(parsed.header, head);
        assert_eq!(parsed.body, "body");
    }

    #[test]
    fn awkward_titles_and_tags_round_trip_through_the_header() {
        for title in [
            "A: A Colon",
            "- leading dash",
            "  padded  ",
            "true",
            "42",
            "",
            "He said \"no\"",
            "back\\slash",
            "#hash",
            "[bracketed]",
            "喉の谷",
        ] {
            let mut head = header();
            head.title = title.to_string();
            head.tags = vec![title.to_string(), "plain".to_string()];
            let parsed = parse(&render(&head, "b")).expect("parses");
            assert_eq!(parsed.header.title, title, "title {title:?}");
            assert_eq!(parsed.header.tags, vec![title.to_string(), "plain".into()]);
        }
    }

    #[test]
    fn an_empty_tag_list_round_trips() {
        let mut head = header();
        head.tags = Vec::new();
        let parsed = parse(&render(&head, "b")).expect("parses");
        assert_eq!(parsed.header.tags, Vec::<String>::new());
    }

    #[test]
    fn a_hand_written_file_is_readable() {
        // Single quotes, no `unresolvable_links` at all, extra unknown keys —
        // all things a Game Master editing a clone might plausibly leave.
        let file = "---\nid: 01a06d8f-5236-76d1-af6b-cd5d71dfbf7c\ntitle: 'The Red Keep'\n\
                    tags: [location]\nupdated: 2026-09-04T18:15:08Z\nsomething: else\n---\n\nBody\n";
        let parsed = parse(file).expect("parses");

        assert_eq!(parsed.header.title, "The Red Keep");
        assert_eq!(parsed.header.tags, vec!["location".to_string()]);
        assert!(parsed.header.unresolvable_links.is_empty());
        assert_eq!(parsed.body, "Body\n");
    }

    #[test]
    fn the_id_is_the_key_and_the_path_is_never_needed_to_read_one() {
        let file = render(&header(), "body");
        let parsed = parse(&file).expect("parses");
        assert_eq!(
            parsed.header.id,
            Uuid::parse_str("01a06d8f-5236-76d1-af6b-cd5d71dfbf7c").expect("valid uuid")
        );
    }

    #[test]
    fn a_file_with_no_front_matter_is_reported_not_guessed() {
        assert_eq!(
            parse("Just some markdown\n"),
            Err(DocumentError::MissingFrontMatter)
        );
    }

    #[test]
    fn an_unterminated_front_matter_is_reported() {
        assert_eq!(
            parse("---\nid: 01a06d8f-5236-76d1-af6b-cd5d71dfbf7c\n"),
            Err(DocumentError::UnterminatedFrontMatter)
        );
    }

    #[test]
    fn a_missing_required_field_is_reported() {
        assert_eq!(
            parse("---\ntitle: X\nupdated: 2026-09-04T18:15:08Z\n---\n\nb"),
            Err(DocumentError::MissingField("id"))
        );
        assert_eq!(
            parse(
                "---\nid: 01a06d8f-5236-76d1-af6b-cd5d71dfbf7c\nupdated: 2026-09-04T18:15:08Z\n---\n\nb"
            ),
            Err(DocumentError::MissingField("title"))
        );
        assert_eq!(
            parse("---\nid: 01a06d8f-5236-76d1-af6b-cd5d71dfbf7c\ntitle: X\n---\n\nb"),
            Err(DocumentError::MissingField("updated"))
        );
    }

    #[test]
    fn an_unreadable_field_is_reported_with_its_name() {
        let error = parse("---\nid: not-a-uuid\ntitle: X\nupdated: 2026-09-04T18:15:08Z\n---\n\nb")
            .expect_err("must fail");
        assert!(matches!(
            error,
            DocumentError::InvalidField { field: "id", .. }
        ));
    }

    #[test]
    fn an_unknown_link_kind_is_refused_rather_than_relabelled() {
        let file = "---\nid: 01a06d8f-5236-76d1-af6b-cd5d71dfbf7c\ntitle: X\ntags: []\n\
                    updated: 2026-09-04T18:15:08Z\nunresolvable_links:\n  - text: \"A\"\n    \
                    kind: spaceship\n---\n\nb";
        assert!(matches!(
            parse(file),
            Err(DocumentError::InvalidField {
                field: "unresolvable_links",
                ..
            })
        ));
    }
}
