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

use crate::schema::{world_abilities, world_actors, world_items, world_lore_entries};

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
    pub target_item_id: Option<Uuid>,
    pub target_ability_id: Option<Uuid>,
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
/// title against `world_lore_entries.title`, `world_actors.label`, and
/// (spec 013 US3) `world_items.name` (case-insensitive exact match)
/// scoped to `world_id`, and returns the Markdown source with each
/// occurrence replaced by an opaque placeholder token, alongside the
/// resolved link list.
///
/// If a title matches more than one kind, resolution falls back in a
/// fixed, deterministic order — lore entry, then actor, then item; the
/// authoring UI's autocomplete (`loreLinkTargets`, T033/spec 013's
/// FR-016) is expected to prevent an author from ever typing an
/// ambiguous title in the first place by disambiguating at selection
/// time (FR-007a).
///
/// Blocking — callers run this inside `tokio::task::spawn_blocking`.
///
/// **Viewer-dependent since spec 025.** `viewer_is_dm` gates whether GM-only
/// abilities are resolvable (FR-030b), so the same lore entry can legitimately
/// render a working link for a DM and an unresolved span for a player. Callers:
///
/// * **Save time** (`mutations_lore.rs`) passes `true` — `world_lore_links` is
///   a canonical index driving backlinks, not a per-viewer view, so a GM's link
///   to their own hidden ability must still be recorded.
/// * **Render time** (`queries::lore::render_lore_content`) passes the actual
///   reader's DM status, which is what withholds the link from a player.
pub fn extract_and_resolve(
    conn: &mut PgConnection,
    world_id: Uuid,
    markdown: &str,
    viewer_is_dm: bool,
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
                // FR-030a: deterministic — without an explicit order Postgres
                // may return either of two same-titled rows, so the same link
                // could resolve differently between reads.
                .order(world_lore_entries::created_at.asc())
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
                    target_item_id: None,
                    target_ability_id: None,
                    href: Some(format!("/world/{world_id}/lore/{slug}/view")),
                }
            } else {
                let actor_match = world_actors::table
                    .filter(world_actors::world_id.eq(world_id))
                    .filter(world_actors::label.ilike(&title))
                    .order(world_actors::created_at.asc())
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
                        target_item_id: None,
                        target_ability_id: None,
                        href: Some(format!("/world/{world_id}/actor/{actor_id}/view")),
                    }
                } else {
                    let item_match = world_items::table
                        .filter(world_items::world_id.eq(world_id))
                        .filter(world_items::name.ilike(&title))
                        .order(world_items::created_at.asc())
                        .select(world_items::id)
                        .first::<Uuid>(conn)
                        .optional()
                        .unwrap_or(None);

                    if let Some(item_id) = item_match {
                        PreparedLink {
                            placeholder: placeholder.clone(),
                            raw_title: title.clone(),
                            display,
                            target_kind: "item",
                            target_lore_entry_id: None,
                            target_actor_id: None,
                            target_item_id: Some(item_id),
                            target_ability_id: None,
                            href: Some(format!("/world/{world_id}/item/{item_id}/view")),
                        }
                    } else {
                        // Spec 025: abilities append LAST, after items. FR-030a
                        // orders by created_at so a duplicate name always
                        // resolves to the earliest; FR-030b hides GM-only
                        // abilities from a non-DM reader, which is why this
                        // whole function is viewer-dependent.
                        let mut ability_query = world_abilities::table
                            .filter(world_abilities::world_id.eq(world_id))
                            .filter(world_abilities::name.ilike(&title))
                            .into_boxed();
                        if !viewer_is_dm {
                            ability_query =
                                ability_query.filter(world_abilities::gm_only.eq(false));
                        }
                        let ability_match = ability_query
                            .order(world_abilities::created_at.asc())
                            .select(world_abilities::id)
                            .first::<Uuid>(conn)
                            .optional()
                            .unwrap_or(None);

                        if let Some(ability_id) = ability_match {
                            PreparedLink {
                                placeholder: placeholder.clone(),
                                raw_title: title.clone(),
                                display,
                                target_kind: "ability",
                                target_lore_entry_id: None,
                                target_actor_id: None,
                                target_item_id: None,
                                target_ability_id: Some(ability_id),
                                href: Some(format!("/world/{world_id}/ability/{ability_id}/view")),
                            }
                        } else {
                            PreparedLink {
                                placeholder: placeholder.clone(),
                                raw_title: title.clone(),
                                display,
                                target_kind: "unresolved",
                                target_lore_entry_id: None,
                                target_actor_id: None,
                                target_item_id: None,
                                target_ability_id: None,
                                href: None,
                            }
                        }
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
    use crate::test_support::{
        insert_test_scene, insert_test_user, insert_test_world, test_app_state,
    };

    fn insert_lore_entry(
        conn: &mut PgConnection,
        world_id: Uuid,
        created_by: Uuid,
        title: &str,
        slug: &str,
    ) -> Uuid {
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

    fn insert_actor(
        conn: &mut PgConnection,
        world_id: Uuid,
        scene_id: Uuid,
        created_by: Uuid,
        label: &str,
    ) -> Uuid {
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
            extract_and_resolve(&mut conn, world_id, "See [[Entry B]] for details.", true)
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

        let (_rewritten, links) =
            extract_and_resolve(&mut conn, world_id, "Meet [[Bo Jangles]].", true)
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

        let (rewritten, links) =
            extract_and_resolve(&mut conn, world_id, "[[Nonexistent Title]]", true)
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

        let (_rewritten, links) = extract_and_resolve(&mut conn, world_id, "[[Ambiguous]]", true)
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
            extract_and_resolve(&mut conn, world_id, "[[Entry B|the ruins]]", true)
                .expect("should resolve");

        assert_eq!(links[0].raw_title, "Entry B");
        assert_eq!(links[0].display, "the ruins");
    }

    fn insert_ability(
        conn: &mut PgConnection,
        world_id: Uuid,
        owner_id: Uuid,
        name: &str,
        gm_only: bool,
    ) -> Uuid {
        use crate::schema::world_abilities;
        diesel::insert_into(world_abilities::table)
            .values((
                world_abilities::world_id.eq(world_id),
                world_abilities::name.eq(name),
                world_abilities::classification.eq("spell"),
                world_abilities::gm_only.eq(gm_only),
                world_abilities::created_by.eq(owner_id),
                world_abilities::updated_by.eq(owner_id),
            ))
            .returning(world_abilities::id)
            .get_result::<Uuid>(conn)
            .expect("insert ability")
    }

    /// FR-028: a title matching only an ability resolves to it.
    #[tokio::test]
    async fn resolves_link_to_existing_ability() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let ability_id = insert_ability(&mut conn, world_id, owner_id, "Fireball", false);

        let (_rewritten, links) =
            extract_and_resolve(&mut conn, world_id, "Cast [[Fireball]] now.", true)
                .expect("should resolve");

        assert_eq!(links[0].target_kind, "ability");
        assert_eq!(links[0].target_ability_id, Some(ability_id));
        assert!(
            links[0]
                .href
                .as_deref()
                .unwrap()
                .contains(&format!("/ability/{ability_id}/view"))
        );
    }

    /// Abilities append LAST in the cascade, so an item of the same name still
    /// wins. Appending rather than inserting is what guarantees no
    /// already-saved link changes target when abilities were introduced.
    #[tokio::test]
    async fn item_wins_over_ability_on_title_collision() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);

        use crate::schema::world_items;
        let item_id = diesel::insert_into(world_items::table)
            .values((
                world_items::world_id.eq(world_id),
                world_items::name.eq("Overlap"),
                world_items::created_by.eq(owner_id),
            ))
            .returning(world_items::id)
            .get_result::<Uuid>(&mut conn)
            .expect("insert item");
        insert_ability(&mut conn, world_id, owner_id, "Overlap", false);

        let (_rewritten, links) =
            extract_and_resolve(&mut conn, world_id, "[[Overlap]]", true).expect("should resolve");

        assert_eq!(links[0].target_kind, "item");
        assert_eq!(links[0].target_item_id, Some(item_id));
    }

    /// FR-030a: duplicate ability names are permitted (FR-006), so resolution
    /// must be deterministic — the earliest-created wins, stably. Without the
    /// explicit ORDER BY, Postgres may return either row.
    #[tokio::test]
    async fn duplicate_ability_names_resolve_to_the_oldest() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);

        let first = insert_ability(&mut conn, world_id, owner_id, "Twin", false);
        // Ensure a distinct created_at ordering.
        std::thread::sleep(std::time::Duration::from_millis(5));
        let second = insert_ability(&mut conn, world_id, owner_id, "Twin", false);
        assert_ne!(first, second);

        for _ in 0..5 {
            let (_r, links) =
                extract_and_resolve(&mut conn, world_id, "[[Twin]]", true).expect("should resolve");
            assert_eq!(
                links[0].target_ability_id,
                Some(first),
                "the earliest-created ability must win, stably across reads"
            );
        }
    }

    /// FR-030b: resolution is viewer-dependent. The same Markdown yields a
    /// working link for a DM and an unresolved span for a player.
    #[tokio::test]
    async fn gm_only_ability_is_unresolved_for_a_non_dm_reader() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let secret_id = insert_ability(&mut conn, world_id, owner_id, "Soul Harvest", true);

        let (_r, dm_links) =
            extract_and_resolve(&mut conn, world_id, "[[Soul Harvest]]", true).unwrap();
        assert_eq!(dm_links[0].target_kind, "ability");
        assert_eq!(dm_links[0].target_ability_id, Some(secret_id));

        let (_r, player_links) =
            extract_and_resolve(&mut conn, world_id, "[[Soul Harvest]]", false).unwrap();
        assert_eq!(
            player_links[0].target_kind, "unresolved",
            "a player must not get a working link to a GM-only ability"
        );
        assert_eq!(player_links[0].target_ability_id, None);
        assert!(player_links[0].href.is_none());
    }

    /// A non-DM's link falls through to the earliest *visible* match, skipping
    /// a hidden one — so the same title can legitimately resolve differently
    /// for a DM and a player. That is the intended effect of hiding.
    #[tokio::test]
    async fn a_player_resolves_past_a_hidden_ability_to_the_visible_one() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);

        let hidden = insert_ability(&mut conn, world_id, owner_id, "Echo", true);
        std::thread::sleep(std::time::Duration::from_millis(5));
        let visible = insert_ability(&mut conn, world_id, owner_id, "Echo", false);

        let (_r, dm_links) = extract_and_resolve(&mut conn, world_id, "[[Echo]]", true).unwrap();
        assert_eq!(
            dm_links[0].target_ability_id,
            Some(hidden),
            "DM gets the oldest overall"
        );

        let (_r, player_links) =
            extract_and_resolve(&mut conn, world_id, "[[Echo]]", false).unwrap();
        assert_eq!(
            player_links[0].target_ability_id,
            Some(visible),
            "a player resolves past the hidden one to the earliest visible match"
        );
    }
}
