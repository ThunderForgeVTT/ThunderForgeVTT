//! Spec 039 US6 and ADR-079: a takedown reaches the copies people took.
//!
//! # The only reader of the adoption record
//!
//! Every read and every write of `content_adoptions` is in this file. Nothing
//! else in the crate names the table, and `graphql/adoption_surface_tests.rs`
//! fails the day something does. ADR-069's determination that a collection
//! share is not a repository rests on nobody being able to ask "what came from
//! this" or "what did this world take" — so the only way in is a walk from an
//! entity a notice has already named.
//!
//! # Child cases
//!
//! A copy is disabled in a case of its own whose `parent_case_id` is the
//! notice's. Not more rows in the notice's case: `strikes_by_account` judges a
//! case by its latest event, and copies written into the sharer's case would
//! make their strike depend on which copy was written last. The child case has
//! no `account_id`, and that is the whole of why an adopter never accrues a
//! strike for somebody else's upload (FR-023b).
//!
//! # Restoration needs no code of its own
//!
//! `fan_out_forward` writes the parent's `counter_notice_forwarded`, with the
//! same due date, into every child case. `effective_status`'s lazy restoration
//! then brings each copy back on its own next read (FR-023d), and calls
//! [`tell_of_restoration_sync`] so the adopter hears about it.
//!
//! # Disabled, never deleted
//!
//! Nothing here deletes anything, and nothing here touches more than the one
//! copied entity: not the world it sits in, not the collection it was filed
//! in, not the adopter's edits beside it (FR-023, FR-023a, FR-023c).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde_json::json;
use uuid::Uuid;

use crate::moderation::action_type;
use crate::notices;
use crate::schema::{content_adoptions, content_moderation_actions};

/// Record that a copy happened, inside the transaction that made it.
///
/// `entity_type` is the moderation vocabulary (`world_item`, …) and names both
/// ends: a copy is always the same kind of thing as its source.
pub fn record_adoption_sync(
    conn: &mut PgConnection,
    entity_type: &str,
    source_id: Uuid,
    copy_id: Uuid,
    destination_world_id: Uuid,
    adopted_by: Uuid,
) -> QueryResult<()> {
    diesel::insert_into(content_adoptions::table)
        .values((
            content_adoptions::id.eq(Uuid::now_v7()),
            content_adoptions::source_entity_type.eq(entity_type),
            content_adoptions::source_entity_id.eq(source_id),
            content_adoptions::copy_entity_type.eq(entity_type),
            content_adoptions::copy_entity_id.eq(copy_id),
            content_adoptions::destination_world_id.eq(destination_world_id),
            content_adoptions::adopted_by.eq(adopted_by),
        ))
        .execute(conn)
        .map(|_| ())
}

/// One copy the walk reached.
struct Reached {
    entity_type: String,
    entity_id: Uuid,
    world_id: Uuid,
    adopted_by: Uuid,
}

/// Every copy descended from `(entity_type, entity_id)`, nearest first.
///
/// Transitive: a copy of a copy is a row whose source is itself a copy, so
/// each copy found is walked from in turn. `seen` makes it terminate whatever
/// the graph looks like, and it is bounded in practice by `MAX_MEMBERS` per
/// copy (ADR-079).
fn copies_of(
    conn: &mut PgConnection,
    entity_type: &str,
    entity_id: Uuid,
) -> QueryResult<Vec<Reached>> {
    let mut seen: BTreeSet<(String, Uuid)> = BTreeSet::from([(entity_type.to_string(), entity_id)]);
    let mut frontier = VecDeque::from([(entity_type.to_string(), entity_id)]);
    let mut reached = Vec::new();

    while let Some((source_type, source_id)) = frontier.pop_front() {
        let rows: Vec<(String, Uuid, Uuid, Uuid)> = content_adoptions::table
            .filter(content_adoptions::source_entity_type.eq(&source_type))
            .filter(content_adoptions::source_entity_id.eq(source_id))
            .order(content_adoptions::adopted_at.asc())
            .select((
                content_adoptions::copy_entity_type,
                content_adoptions::copy_entity_id,
                content_adoptions::destination_world_id,
                content_adoptions::adopted_by,
            ))
            .load(conn)?;

        for (copy_type, copy_id, world_id, adopted_by) in rows {
            if seen.insert((copy_type.clone(), copy_id)) {
                frontier.push_back((copy_type.clone(), copy_id));
                reached.push(Reached {
                    entity_type: copy_type,
                    entity_id: copy_id,
                    world_id,
                    adopted_by,
                });
            }
        }
    }
    Ok(reached)
}

/// Who took this copy — for telling them it came back.
fn adopter_of(
    conn: &mut PgConnection,
    entity_type: &str,
    entity_id: Uuid,
) -> QueryResult<Option<Uuid>> {
    content_adoptions::table
        .filter(content_adoptions::copy_entity_type.eq(entity_type))
        .filter(content_adoptions::copy_entity_id.eq(entity_id))
        .select(content_adoptions::adopted_by)
        .first(conn)
        .optional()
}

/// The copy's own name, in the adopter's world — `None` when it no longer
/// exists. A notice names the adopter's copy so they know which one; it never
/// names the source, the sharer or the claimant.
fn name_of(
    conn: &mut PgConnection,
    entity_type: &str,
    entity_id: Uuid,
) -> QueryResult<Option<String>> {
    use crate::schema::{world_abilities, world_actors, world_items, world_lore_entries};

    match entity_type {
        "world_actor" => world_actors::table
            .filter(world_actors::id.eq(entity_id))
            .select(world_actors::label)
            .first(conn)
            .optional(),
        "world_item" => world_items::table
            .filter(world_items::id.eq(entity_id))
            .select(world_items::name)
            .first(conn)
            .optional(),
        "world_ability" => world_abilities::table
            .filter(world_abilities::id.eq(entity_id))
            .select(world_abilities::name)
            .first(conn)
            .optional(),
        "world_lore_entry" => world_lore_entries::table
            .filter(world_lore_entries::id.eq(entity_id))
            .select(world_lore_entries::title)
            .first(conn)
            .optional(),
        _ => Ok(None),
    }
}

/// One child case, as its opening row describes it.
struct Child {
    case_id: Uuid,
    entity_type: String,
    entity_id: Uuid,
    world_id: Uuid,
}

fn child_cases(conn: &mut PgConnection, parent_case_id: Uuid) -> QueryResult<Vec<Child>> {
    let rows: Vec<(Uuid, String, Uuid, Uuid)> = content_moderation_actions::table
        .filter(content_moderation_actions::parent_case_id.eq(parent_case_id))
        .order(content_moderation_actions::created_at.asc())
        .select((
            content_moderation_actions::case_id,
            content_moderation_actions::entity_type,
            content_moderation_actions::entity_id,
            content_moderation_actions::world_id,
        ))
        .load(conn)?;

    let mut seen = BTreeSet::new();
    Ok(rows
        .into_iter()
        .filter(|(case_id, ..)| seen.insert(*case_id))
        .map(|(case_id, entity_type, entity_id, world_id)| Child {
            case_id,
            entity_type,
            entity_id,
            world_id,
        })
        .collect())
}

/// One event in a child case. No `account_id`, ever, and none of the
/// claimant's columns: the claimant made no claim about this copy, and the
/// adopter is not a party to the notice.
#[allow(clippy::too_many_arguments)]
fn insert_child_event(
    conn: &mut PgConnection,
    case_id: Uuid,
    parent_case_id: Uuid,
    action: &str,
    entity_type: &str,
    entity_id: Uuid,
    world_id: Uuid,
    restoration_due_at: Option<DateTime<Utc>>,
) -> QueryResult<()> {
    diesel::insert_into(content_moderation_actions::table)
        .values((
            content_moderation_actions::case_id.eq(case_id),
            content_moderation_actions::parent_case_id.eq(Some(parent_case_id)),
            content_moderation_actions::action_type.eq(action),
            content_moderation_actions::entity_type.eq(entity_type),
            content_moderation_actions::entity_id.eq(entity_id),
            content_moderation_actions::world_id.eq(world_id),
            content_moderation_actions::account_id.eq(None::<Uuid>),
            content_moderation_actions::claimant_name.eq(""),
            content_moderation_actions::claimant_contact.eq(""),
            content_moderation_actions::copyrighted_work_description.eq(""),
            content_moderation_actions::infringing_material_location.eq(""),
            content_moderation_actions::good_faith_statement.eq(false),
            content_moderation_actions::accuracy_statement.eq(false),
            content_moderation_actions::signature.eq(""),
            content_moderation_actions::restoration_due_at.eq(restoration_due_at),
        ))
        .execute(conn)
        .map(|_| ())
}

/// A takedown was upheld against `(entity_type, entity_id)`: disable every
/// copy taken of it, each in a child case of `parent_case_id`, and tell each
/// adopter and the sharer. Returns the child case ids.
///
/// One transaction of its own, so a takedown never reaches half the copies. A
/// copy its adopter has since deleted is skipped — there is nothing to disable
/// — but the walk still passes through it to the copies made of it.
pub fn fan_out_disable(
    conn: &mut PgConnection,
    parent_case_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
    sharer: Option<Uuid>,
) -> Result<Vec<Uuid>, String> {
    conn.transaction::<_, diesel::result::Error, _>(|conn| {
        let mut children = Vec::new();
        let mut by_adopter: BTreeMap<Uuid, (Vec<Uuid>, Vec<String>)> = BTreeMap::new();

        for copy in copies_of(conn, entity_type, entity_id)? {
            let Some(name) = name_of(conn, &copy.entity_type, copy.entity_id)? else {
                continue;
            };
            let child = Uuid::now_v7();
            insert_child_event(
                conn,
                child,
                parent_case_id,
                action_type::CONTENT_DISABLED_AS_COPY,
                &copy.entity_type,
                copy.entity_id,
                copy.world_id,
                None,
            )?;
            children.push(child);
            let (cases, names) = by_adopter.entry(copy.adopted_by).or_default();
            cases.push(child);
            names.push(name);
        }

        // FR-023b: told, and not accused. The payload names their own copies
        // and nothing else — no claimant, no source, no sharer.
        for (adopter, (cases, names)) in by_adopter {
            notices::record_sync(
                conn,
                adopter,
                notices::kind::ADOPTED_COPY_DISABLED,
                Some(json!({ "caseIds": cases })),
                Some(json!({ "copies": names })),
            )?;
        }

        // FR-024: the sharer already has `strike_recorded` for the source;
        // this is the part only the fan-out knows — that it reached copies.
        if !children.is_empty()
            && let Some(sharer) = sharer
        {
            notices::record_sync(
                conn,
                sharer,
                notices::kind::SHARE_TAKEN_DOWN,
                Some(json!({ "caseId": parent_case_id })),
                Some(json!({ "copiesDisabled": children.len() })),
            )?;
        }

        Ok(children)
    })
    .map_err(|e| format!("Failed to reach the copies: {e}"))
}

/// A counter-notice was forwarded on `parent_case_id`: forward it, with the
/// **same** due date, in every child case — so each copy comes back when the
/// source does and not a moment apart (FR-023d). Returns how many.
pub fn fan_out_forward(
    conn: &mut PgConnection,
    parent_case_id: Uuid,
    restoration_due_at: DateTime<Utc>,
) -> Result<usize, String> {
    conn.transaction::<_, diesel::result::Error, _>(|conn| {
        let children = child_cases(conn, parent_case_id)?;
        for child in &children {
            insert_child_event(
                conn,
                child.case_id,
                parent_case_id,
                action_type::COUNTER_NOTICE_FORWARDED,
                &child.entity_type,
                child.entity_id,
                child.world_id,
                Some(restoration_due_at),
            )?;
        }
        Ok(children.len())
    })
    .map_err(|e| format!("Failed to forward to the copies: {e}"))
}

/// An administrator resolved `parent_case_id`: the same resolution in every
/// child case. Without this the source comes back while its copies stay dark,
/// and nobody notices until an adopter complains. Returns how many.
pub fn fan_out_resolve(
    conn: &mut PgConnection,
    parent_case_id: Uuid,
    resolution: &str,
) -> Result<usize, String> {
    conn.transaction::<_, diesel::result::Error, _>(|conn| {
        let children = child_cases(conn, parent_case_id)?;
        for child in &children {
            insert_child_event(
                conn,
                child.case_id,
                parent_case_id,
                resolution,
                &child.entity_type,
                child.entity_id,
                child.world_id,
                None,
            )?;
            if resolution == action_type::CONTENT_RESTORED {
                tell_of_restoration_sync(conn, child.case_id, &child.entity_type, child.entity_id)?;
            }
        }
        Ok(children.len())
    })
    .map_err(|e| format!("Failed to resolve the copies: {e}"))
}

/// A case was just restored. If it is a copy's, tell the adopter it is back
/// (FR-023d) — they did not ask, and should not have to find out by looking.
/// Anything else is not this module's to announce, and returns quietly.
pub fn tell_of_restoration_sync(
    conn: &mut PgConnection,
    case_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
) -> QueryResult<()> {
    let is_child: bool = diesel::select(diesel::dsl::exists(
        content_moderation_actions::table
            .filter(content_moderation_actions::case_id.eq(case_id))
            .filter(content_moderation_actions::parent_case_id.is_not_null()),
    ))
    .get_result(conn)?;
    if !is_child {
        return Ok(());
    }
    let Some(adopter) = adopter_of(conn, entity_type, entity_id)? else {
        return Ok(());
    };
    let Some(name) = name_of(conn, entity_type, entity_id)? else {
        return Ok(());
    };
    notices::record_sync(
        conn,
        adopter,
        notices::kind::ADOPTED_COPY_RESTORED,
        Some(json!({ "caseIds": [case_id] })),
        Some(json!({ "copies": [name] })),
    )
    .map(|_| ())
}

#[cfg(test)]
#[path = "reach_tests.rs"]
mod tests;
