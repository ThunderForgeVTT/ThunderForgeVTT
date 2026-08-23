//! Spec 012 (research.md §2): `[[Title]]` / `[[Title|Display]]` in-text
//! wiki-link extraction and resolution against lore entries and actors,
//! scoped to a world (FR-005, FR-006, FR-007, FR-007a).
//!
//! Resolution happens once, at save time, against the raw Markdown
//! source — never re-resolved live on every read (research.md §2's
//! "immutable snapshot" rationale). Because comrak's `unsafe_` render
//! flag must stay `false` (see `markdown::mod`'s doc comment), a `[[...]]`
//! occurrence cannot become a styled `<a class="...">`/broken-link
//! `<span>` by injecting raw HTML into the Markdown source — comrak would
//! escape it. Instead, each occurrence is replaced with an opaque
//! placeholder token before rendering, and the token is substituted for
//! real (already-escaped) anchor/span HTML *after* comrak+ammonia have
//! produced the sanitized page — see `substitute_placeholders_into_html`.

use diesel::prelude::*;
use once_cell::sync::Lazy;
use regex::Regex;
use uuid::Uuid;

use crate::schema::{world_actors, world_lore_entries};

/// Matches `[[Title]]` or `[[Title|Display Text]]`. Title/Display may not
/// contain `]` or `|`.
static LINK_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").expect("valid regex"));

/// One resolved (or unresolved) `[[...]]` occurrence, ready both for
/// persisting a `world_lore_links` row and for substituting real HTML in
/// place of its placeholder token.
#[derive(Debug, Clone)]
pub struct PreparedLink {
    /// Opaque token substituted into the Markdown source in place of the
    /// original `[[...]]` text; never seen by comrak as anything but
    /// plain text, so it survives rendering untouched.
    pub placeholder: String,
    pub raw_title: String,
    pub display: String,
    pub target_kind: &'static str,
    pub target_lore_entry_id: Option<Uuid>,
    pub target_actor_id: Option<Uuid>,
    /// `None` for an unresolved link.
    pub href: Option<String>,
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Extracts every `[[...]]` occurrence from `markdown`, resolves each
/// title against `world_lore_entries.title` and `world_actors.label`
/// (case-insensitive exact match) scoped to `world_id`, and returns the
/// Markdown source with each occurrence replaced by an opaque
/// placeholder token, alongside the resolved link list.
///
/// If a title matches both a lore entry and an actor, the lore entry
/// wins (deterministic tie-break); the authoring UI's autocomplete
/// (`loreLinkTargets`, T033) is expected to prevent an author from ever
/// typing an ambiguous title in the first place by disambiguating at
/// selection time (FR-007a).
///
/// Blocking — callers run this inside `tokio::task::spawn_blocking`.
pub fn extract_and_resolve(
    conn: &mut PgConnection,
    world_id: Uuid,
    markdown: &str,
) -> Result<(String, Vec<PreparedLink>), diesel::result::Error> {
    let mut links = Vec::new();
    let mut counter = 0usize;

    let rewritten = LINK_PATTERN
        .replace_all(markdown, |caps: &regex::Captures| {
            let title = caps[1].trim().to_string();
            let display = caps
                .get(2)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_else(|| title.clone());

            let placeholder = format!("\u{E001}LORELINK{counter}\u{E002}");
            counter += 1;

            let lore_match = world_lore_entries::table
                .filter(world_lore_entries::world_id.eq(world_id))
                .filter(world_lore_entries::title.ilike(&title))
                .select((world_lore_entries::id, world_lore_entries::slug))
                .first::<(Uuid, String)>(conn)
                .optional()
                .unwrap_or(None);

            let prepared = if let Some((entry_id, slug)) = lore_match {
                PreparedLink {
                    placeholder: placeholder.clone(),
                    raw_title: title.clone(),
                    display,
                    target_kind: "lore_entry",
                    target_lore_entry_id: Some(entry_id),
                    target_actor_id: None,
                    href: Some(format!("/world/{world_id}/lore/{slug}/view")),
                }
            } else {
                let actor_match = world_actors::table
                    .filter(world_actors::world_id.eq(world_id))
                    .filter(world_actors::label.ilike(&title))
                    .select(world_actors::id)
                    .first::<Uuid>(conn)
                    .optional()
                    .unwrap_or(None);

                if let Some(actor_id) = actor_match {
                    PreparedLink {
                        placeholder: placeholder.clone(),
                        raw_title: title.clone(),
                        display,
                        target_kind: "actor",
                        target_lore_entry_id: None,
                        target_actor_id: Some(actor_id),
                        href: Some(format!("/world/{world_id}/actor/{actor_id}/view")),
                    }
                } else {
                    PreparedLink {
                        placeholder: placeholder.clone(),
                        raw_title: title.clone(),
                        display,
                        target_kind: "unresolved",
                        target_lore_entry_id: None,
                        target_actor_id: None,
                        href: None,
                    }
                }
            };

            links.push(prepared);
            placeholder
        })
        .into_owned();

    Ok((rewritten, links))
}

/// Substitutes each `PreparedLink`'s placeholder token in `html` (the
/// already-rendered, already-sanitized page) for a real, styled anchor
/// (resolved) or broken-link span (unresolved) — FR-007.
pub fn substitute_placeholders_into_html(html: &str, links: &[PreparedLink]) -> String {
    let mut out = html.to_string();
    for link in links {
        let replacement = match &link.href {
            Some(href) => format!(
                "<a class=\"lore-link\" href=\"{}\">{}</a>",
                html_escape(href),
                html_escape(&link.display)
            ),
            None => format!(
                "<span class=\"lore-link-broken\" title=\"Unresolved link\">{}</span>",
                html_escape(&link.display)
            ),
        };
        out = out.replace(&link.placeholder, &replacement);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{insert_test_scene, insert_test_user, insert_test_world, test_app_state};

    fn insert_lore_entry(conn: &mut PgConnection, world_id: Uuid, created_by: Uuid, title: &str, slug: &str) -> Uuid {
        let id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_lore_entries::table)
            .values((
                world_lore_entries::id.eq(id),
                world_lore_entries::world_id.eq(world_id),
                world_lore_entries::title.eq(title),
                world_lore_entries::slug.eq(slug),
                world_lore_entries::content.eq(""),
                world_lore_entries::created_by.eq(created_by),
                world_lore_entries::created_at.eq(now),
                world_lore_entries::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("failed to insert test lore entry");
        id
    }

    fn insert_actor(conn: &mut PgConnection, world_id: Uuid, scene_id: Uuid, created_by: Uuid, label: &str) -> Uuid {
        let id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_actors::table)
            .values((
                world_actors::id.eq(id),
                world_actors::world_id.eq(world_id),
                world_actors::scene_id.eq(scene_id),
                world_actors::actor_type.eq("npc"),
                world_actors::game_system_id.eq("generic"),
                world_actors::label.eq(label),
                world_actors::created_by.eq(created_by),
                world_actors::owned_by.eq(created_by),
                world_actors::is_public.eq(false),
                world_actors::is_npc.eq(true),
                world_actors::created_at.eq(now),
                world_actors::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("failed to insert test actor");
        id
    }

    /// FR-005/FR-006: a `[[Title]]` occurrence resolves to an existing
    /// lore entry, and the placeholder round-trips into a real link.
    #[tokio::test]
    async fn resolves_link_to_existing_lore_entry() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let entry_id = insert_lore_entry(&mut conn, world_id, owner_id, "Entry B", "entry-b");

        let (rewritten, links) =
            extract_and_resolve(&mut conn, world_id, "See [[Entry B]] for details.")
                .expect("resolution should succeed");

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target_kind, "lore_entry");
        assert_eq!(links[0].target_lore_entry_id, Some(entry_id));
        assert!(rewritten.contains(&links[0].placeholder));
        assert!(!rewritten.contains("[[Entry B]]"));

        let html = substitute_placeholders_into_html("<p>See PLACEHOLDER for details.</p>", &links);
        // (placeholder token itself is opaque; just confirm substitution produces a real anchor)
        let html2 = substitute_placeholders_into_html(&rewritten, &links);
        assert!(html2.contains("<a class=\"lore-link\""), "{html2}");
        let _ = html;
    }

    /// FR-005: a `[[Title]]` occurrence resolves to an existing actor
    /// when no lore entry matches.
    #[tokio::test]
    async fn resolves_link_to_existing_actor() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_actor(&mut conn, world_id, scene_id, owner_id, "Bo Jangles");

        let (_rewritten, links) = extract_and_resolve(&mut conn, world_id, "Meet [[Bo Jangles]].")
            .expect("resolution should succeed");

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target_kind, "actor");
        assert_eq!(links[0].target_actor_id, Some(actor_id));
    }

    /// FR-007: a title matching neither a lore entry nor an actor is
    /// unresolved, and renders as a broken-link span, not a crash.
    #[tokio::test]
    async fn unresolved_link_renders_as_broken_span() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);

        let (rewritten, links) = extract_and_resolve(&mut conn, world_id, "[[Nonexistent Title]]")
            .expect("resolution should succeed even with no match");

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target_kind, "unresolved");
        assert!(links[0].href.is_none());

        let html = substitute_placeholders_into_html(&rewritten, &links);
        assert!(html.contains("lore-link-broken"), "{html}");
    }

    /// FR-007a: when a title matches both a lore entry and an actor, the
    /// lore entry wins the deterministic tie-break.
    #[tokio::test]
    async fn lore_entry_wins_over_actor_on_title_collision() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let entry_id = insert_lore_entry(&mut conn, world_id, owner_id, "Ambiguous", "ambiguous");
        insert_actor(&mut conn, world_id, scene_id, owner_id, "Ambiguous");

        let (_rewritten, links) = extract_and_resolve(&mut conn, world_id, "[[Ambiguous]]")
            .expect("resolution should succeed");

        assert_eq!(links[0].target_kind, "lore_entry");
        assert_eq!(links[0].target_lore_entry_id, Some(entry_id));
    }

    /// `[[Title|Display]]` uses the display text as the link's visible
    /// text while resolving against the title.
    #[tokio::test]
    async fn supports_display_text_syntax() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_lore_entry(&mut conn, world_id, owner_id, "Entry B", "entry-b");

        let (_rewritten, links) =
            extract_and_resolve(&mut conn, world_id, "[[Entry B|the ruins]]").expect("should resolve");

        assert_eq!(links[0].raw_title, "Entry B");
        assert_eq!(links[0].display, "the ruins");
    }
}
