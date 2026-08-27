//! Spec 012 (research.md §1): server-authoritative GitHub-flavored
//! Markdown rendering — the single source of rendering truth (FR-004).
//! `comrak` parses/renders GFM (tables, task lists, strikethrough,
//! autolinks) with raw-HTML passthrough disabled (`unsafe_` stays at its
//! default `false`, so any raw HTML/script in the Markdown source is
//! escaped as text, never executed); `ammonia` then sanitizes the
//! resulting HTML as defense in depth, with a small allowlist extension
//! beyond ammonia's default so GFM's own output round-trips intact:
//! `class` (needed for the client's code-block syntax-highlight pass and
//! for `task-list-item`/`task-list-item-checkbox` classes) and `input`
//! with `type`/`checked`/`disabled` (needed for task-list checkboxes,
//! which ammonia's default allowlist does not include at all).

pub mod links;
pub mod slug;

use std::collections::HashSet;

/// Render Markdown source to sanitized, GFM-complete HTML (FR-004).
/// Used both for a lore entry's live `content` and for re-rendering any
/// past `world_lore_revisions.content_markdown` on demand (FR-017).
pub fn render_to_safe_html(markdown: &str) -> String {
    let mut options = comrak::Options::default();
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.render.tasklist_classes = true;
    // `unsafe_` intentionally left at its default `false` — raw HTML in
    // user-authored Markdown must never pass through unescaped.

    let raw_html = comrak::markdown_to_html(markdown, &options);

    // Declared before `builder` so they're dropped *after* it (Rust drops
    // locals in reverse declaration order) — `Builder<'a>` borrows these,
    // so they must outlive it.
    let extra_tags: HashSet<&str> = ["input"].into_iter().collect();
    let extra_generic_attrs: HashSet<&str> = ["class"].into_iter().collect();
    let input_attrs: HashSet<&str> = ["type", "checked", "disabled"].into_iter().collect();

    let mut builder = ammonia::Builder::default();
    builder.add_tags(&extra_tags);
    builder.add_generic_attributes(&extra_generic_attrs);
    builder.add_tag_attributes("input", &input_attrs);

    builder.clean(&raw_html).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FR-004: tables, task lists, strikethrough, and fenced code blocks
    /// all render, matching standard GFM behavior.
    #[test]
    fn renders_gfm_constructs() {
        let md = "\
| a | b |\n\
|---|---|\n\
| 1 | 2 |\n\
\n\
- [x] done\n\
- [ ] not done\n\
\n\
~~strike~~\n\
\n\
```rust\n\
fn main() {}\n\
```\n\
";
        let html = render_to_safe_html(md);
        assert!(html.contains("<table>"), "table should render: {html}");
        assert!(
            html.contains("type=\"checkbox\""),
            "task list checkbox should survive sanitization: {html}"
        );
        assert!(
            html.contains("checked"),
            "checked task item should survive sanitization: {html}"
        );
        assert!(
            html.contains("<del>strike</del>"),
            "strikethrough should render: {html}"
        );
        assert!(
            html.contains("language-rust"),
            "fenced code block language class should survive sanitization: {html}"
        );
    }

    /// research.md §1: raw HTML/script in the source must never pass
    /// through unescaped (unsafe_ stays false, and ammonia is defense in
    /// depth on top of that).
    #[test]
    fn escapes_raw_html_and_strips_script() {
        let md = "before <script>alert('xss')</script> after";
        let html = render_to_safe_html(md);
        // The `<script>` tags themselves must never survive as real tags
        // (comrak's `unsafe_: false` drops/escapes raw HTML entirely, so
        // any leftover text is inert paragraph content, not an
        // executable element) — that's the actual security property.
        assert!(
            !html.contains("<script"),
            "script tag must never survive as a real element: {html}"
        );
        assert!(
            !html.contains("</script>"),
            "script tag must never survive as a real element: {html}"
        );
    }

    /// A raw autolinked URL renders as a clickable link (FR-004
    /// acceptance scenario 3).
    #[test]
    fn autolinks_raw_urls() {
        let html = render_to_safe_html("See https://example.com for details.");
        assert!(
            html.contains("<a href=\"https://example.com\""),
            "raw URL should autolink: {html}"
        );
    }
}
