//! What a world says its repository should contain.
//!
//! # Why planning is separate from writing
//!
//! A pass has two halves with very different natures. Deciding *what the
//! repository should contain* is a pure function of the world — entries, their
//! tree, their tags, their links, their images — and is where every rule in
//! FR-007 through FR-015 lives. Making the repository match is file I/O and
//! subprocess calls.
//!
//! Splitting them means the rules are testable without a remote, a clone, or a
//! credential. That is the same argument `thunderforge-repo-host` makes about
//! its own pure core, and it is worth as much here: "a moderation-disabled
//! entry is absent from the plan" is a claim about a `Vec`, not about a
//! repository someone has to set up first.
//!
//! # What is deliberately absent
//!
//! Any write to a world. This module reads `world_lore_entries`,
//! `world_lore_tags`, `world_lore_image_assets` and their revisions, and
//! writes none of them. Every lore table is read-only to the whole of
//! `lore_sync`, which is what makes the first delivery unable to damage a
//! world by construction rather than by care.

use std::collections::HashMap;

use diesel::prelude::*;
use uuid::Uuid;

use crate::AppState;
use crate::lore_sync::document::{self, DocumentHeader, LinkTarget, UnresolvableKind};
use crate::lore_sync::paths::{self, AssignedPath, EntryNode};
use crate::moderation;

/// One file the repository should contain, and the entry it represents.
#[derive(Debug, Clone)]
pub struct PlannedFile {
    pub entry_id: Uuid,
    /// Relative to the world's subtree. A label; `entry_id` is the key.
    pub path: String,
    pub contents: String,
}

/// An image the repository should contain.
///
/// FR-014: the uploaded original only. Derived renditions stay on the
/// platform — the repository is not a rendition store — and mirroring them
/// would multiply the size of an image-heavy world's clone for no gain a
/// reader of that clone would notice.
#[derive(Debug, Clone)]
pub struct PlannedImage {
    pub asset_id: Uuid,
    /// Relative to the world's subtree.
    pub path: String,
    /// The object key to read from storage.
    pub object_key: String,
}

/// Something that could not be carried across (FR-013, FR-037).
#[derive(Debug, Clone)]
pub struct PlannedFidelityNote {
    pub entry_id: Option<Uuid>,
    pub kind: &'static str,
    pub detail: String,
}

/// Everything a pass should make true of the repository.
#[derive(Debug, Default)]
pub struct Plan {
    pub files: Vec<PlannedFile>,
    pub images: Vec<PlannedImage>,
    pub notes: Vec<PlannedFidelityNote>,
}

/// The directory images live in, relative to the world's subtree.
pub const IMAGE_DIR: &str = "_images";

/// Build the plan for a world.
///
/// The moderation filter runs **before** paths are assigned, not after, and
/// the ordering is load-bearing. Assigning paths first and then dropping the
/// disabled entries would leave the survivors carrying disambiguation suffixes
/// earned against a sibling that is no longer there — a takedown on one entry
/// would rename another, producing a commit for an entry nobody touched.
pub async fn plan_world(state: &AppState, world_id: Uuid) -> Result<Plan, String> {
    use crate::schema::{world_lore_entries, world_lore_image_assets, world_lore_tags};

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    let rows: Vec<(Uuid, Option<Uuid>, String, String, chrono::NaiveDateTime)> =
        world_lore_entries::table
            .filter(world_lore_entries::world_id.eq(world_id))
            .select((
                world_lore_entries::id,
                world_lore_entries::parent_id,
                world_lore_entries::title,
                world_lore_entries::content,
                world_lore_entries::updated_at,
            ))
            .load(&mut conn)
            .map_err(|e| format!("Failed to load lore entries: {e}"))?;

    // FR-015. An entry disabled by a takedown is absent from the repository,
    // and its absence must not stop the rest of the world synchronising —
    // which is why this filters rather than failing.
    let visible = moderation::filter_visible(state, "lore_entry", rows, |r| r.0)
        .await
        .map_err(|_| "Failed to apply moderation filter".to_string())?;

    // What a `[[target]]` may resolve to besides lore. Loaded once rather than
    // queried per link: a world's own content is small (the same reasoning
    // `moderation::filter_visible` records), and a per-link query would make
    // the cost of planning scale with how heavily an author cross-references.
    //
    // Matched the way `markdown::links` matches — case-insensitively against
    // `world_actors.label`, `world_items.name`, `world_abilities.name` — so
    // the mirror and the rendered page agree about what a link points at. A
    // link the app resolves to an actor must be recorded as an unresolvable
    // cross-link here (FR-013), and one the app resolves to nothing must not
    // be, since no fidelity was lost.
    let non_lore = load_non_lore_targets(&mut conn, world_id)?;

    let nodes: Vec<EntryNode> = visible
        .iter()
        .map(|(id, parent_id, title, _, _)| EntryNode {
            id: *id,
            parent_id: *parent_id,
            title: title.clone(),
        })
        .collect();
    let assigned = paths::map_entry_paths(&nodes);

    let ids: Vec<Uuid> = visible.iter().map(|r| r.0).collect();

    let mut tags_by_entry: HashMap<Uuid, Vec<String>> = HashMap::new();
    for (entry_id, tag) in world_lore_tags::table
        .filter(world_lore_tags::lore_entry_id.eq_any(&ids))
        .select((world_lore_tags::lore_entry_id, world_lore_tags::tag))
        .load::<(Uuid, String)>(&mut conn)
        .map_err(|e| format!("Failed to load lore tags: {e}"))?
    {
        tags_by_entry.entry(entry_id).or_default().push(tag);
    }
    // Sorted, so an unordered database read cannot produce a spurious diff on
    // a pass where nothing actually changed.
    for tags in tags_by_entry.values_mut() {
        tags.sort();
    }

    let mut plan = Plan::default();

    for (entry_id, _, title, content, updated_at) in &visible {
        let Some(assignment) = assigned.get(entry_id) else {
            continue;
        };

        if assignment.was_disambiguated() {
            plan.notes.push(PlannedFidelityNote {
                entry_id: Some(*entry_id),
                kind: "path_disambiguated",
                detail: format!(
                    "\"{title}\" could not use its title as a filename, so it is at {}.",
                    assignment.path
                ),
            });
        }

        let (body, unresolvable) =
            document::rewrite_links_for_export(content, &assignment.path, |target| {
                resolve_link(target, &assigned, &nodes, &non_lore)
            });

        for link in &unresolvable {
            plan.notes.push(PlannedFidelityNote {
                entry_id: Some(*entry_id),
                kind: "unresolvable_cross_link",
                detail: format!(
                    "\"{}\" links to {}, which is not lore and cannot resolve in a repository.",
                    title,
                    link.kind.as_str()
                ),
            });
        }

        let header = DocumentHeader {
            id: *entry_id,
            title: title.clone(),
            tags: tags_by_entry.get(entry_id).cloned().unwrap_or_default(),
            updated: updated_at.and_utc(),
            unresolvable_links: unresolvable,
        };

        plan.files.push(PlannedFile {
            entry_id: *entry_id,
            path: assignment.path.clone(),
            contents: document::render(&header, &body),
        });
    }

    // FR-014. Only entries that survived the moderation filter contribute
    // images: mirroring the image of a disabled entry would leave the picture
    // in the repository after the words had gone.
    for (asset_id, content_type) in world_lore_image_assets::table
        .filter(world_lore_image_assets::lore_entry_id.eq_any(&ids))
        .select((
            world_lore_image_assets::id,
            world_lore_image_assets::content_type,
        ))
        .load::<(Uuid, String)>(&mut conn)
        .map_err(|e| format!("Failed to load lore images: {e}"))?
    {
        let _ = content_type;
        plan.images.push(PlannedImage {
            asset_id,
            // The stored original is always webp — `assets_serve::lore` writes
            // it that way — so the extension is not guessed from a content
            // type that describes what was uploaded rather than what is held.
            path: format!("{IMAGE_DIR}/{asset_id}.webp"),
            object_key: format!("lore/{asset_id}.webp"),
        });
    }

    // Deterministic order, so two passes over an unchanged world produce
    // byte-identical plans and therefore no commit.
    plan.files.sort_by(|a, b| a.path.cmp(&b.path));
    plan.images.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(plan)
}

/// How a `[[target]]` in an entry's markdown resolves for export.
///
/// Lore targets become relative paths (FR-012). Anything else stays readable
/// text and is recorded (FR-013) — a declared loss, not an error.
fn resolve_link(
    target: &str,
    assigned: &std::collections::BTreeMap<Uuid, AssignedPath>,
    nodes: &[EntryNode],
    non_lore: &HashMap<String, UnresolvableKind>,
) -> Option<LinkTarget> {
    let needle = target.trim();

    // Lore first, matching the app's own precedence: a lore entry and an actor
    // sharing a name resolve to the lore entry in the rendered page, and the
    // mirror must not disagree with it.
    if let Some(matched) = nodes.iter().find(|n| n.title.eq_ignore_ascii_case(needle))
        && let Some(path) = assigned.get(&matched.id)
    {
        return Some(LinkTarget::Lore {
            path: path.path.clone(),
        });
    }

    // Something the app resolves, that a repository cannot. FR-013's declared
    // loss: readable in the body, recorded in the header.
    if let Some(kind) = non_lore.get(&needle.to_lowercase()) {
        return Some(LinkTarget::Unresolvable { kind: *kind });
    }

    // Resolved to nothing in the app either, so nothing was lost. Recording it
    // would assert a fidelity loss that did not occur.
    None
}

/// Every non-lore target in a world that a `[[link]]` could name, lowercased.
///
/// A disabled actor or item is still loaded, deliberately: the question here
/// is "does this link resolve in the app", and the moderation gate on the
/// *target* is that target's own concern. Answering it differently would make
/// a fidelity note appear and disappear as unrelated content was moderated.
fn load_non_lore_targets(
    conn: &mut diesel::PgConnection,
    world_id: Uuid,
) -> Result<HashMap<String, UnresolvableKind>, String> {
    use crate::schema::{world_abilities, world_actors, world_items};

    let mut out = HashMap::new();

    // Inserted lowest-precedence first so that a name shared across types
    // reports the same kind the app would resolve it to: actor, then item,
    // then ability, matching `markdown::links`' order.
    for (name,) in world_abilities::table
        .filter(world_abilities::world_id.eq(world_id))
        .select((world_abilities::name,))
        .load::<(String,)>(conn)
        .map_err(|e| format!("Failed to load abilities: {e}"))?
    {
        out.insert(name.to_lowercase(), UnresolvableKind::Ability);
    }
    for (name,) in world_items::table
        .filter(world_items::world_id.eq(world_id))
        .select((world_items::name,))
        .load::<(String,)>(conn)
        .map_err(|e| format!("Failed to load items: {e}"))?
    {
        out.insert(name.to_lowercase(), UnresolvableKind::Item);
    }
    for (label,) in world_actors::table
        .filter(world_actors::world_id.eq(world_id))
        .select((world_actors::label,))
        .load::<(String,)>(conn)
        .map_err(|e| format!("Failed to load actors: {e}"))?
    {
        out.insert(label.to_lowercase(), UnresolvableKind::Actor);
    }

    Ok(out)
}

/// The note every connection carries, regardless of content (FR-037).
///
/// Recorded as a fidelity note rather than only shown once at connection time,
/// because SC-008 requires losses to be *enumerated* — something a Game Master
/// can go back and look at — not merely disclosed once and then forgotten.
pub fn permission_flattening_note() -> PlannedFidelityNote {
    PlannedFidelityNote {
        entry_id: None,
        kind: "permission_not_carried",
        detail: "Per-entry lore permissions do not exist in a repository. Every mirrored \
                 entry is visible to everyone with access to it, including entries \
                 restricted to some members of this world."
            .to_string(),
    }
}

/// The note a **public** repository earns (FR-040a, FR-037a).
///
/// Separate from permission flattening on purpose. "Everyone you invited to
/// this repository" and "everyone on the internet" are different sentences,
/// and folding them into one note would leave the users most exposed reading
/// the milder of the two.
pub fn public_mirror_note(repository_ref: &str) -> PlannedFidelityNote {
    PlannedFidelityNote {
        entry_id: None,
        kind: "mirrored_publicly",
        detail: format!(
            "{repository_ref} was publicly visible when this world last synchronised, so \
             everything mirrored there is readable by anyone."
        ),
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod plan_tests;
