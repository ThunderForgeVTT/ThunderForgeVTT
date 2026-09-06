//! Spec 026 FR-012 to FR-019: copying a shared collection into a world.
//!
//! One transaction, all-or-nothing. A failure part-way leaves nothing behind
//! (FR-013, SC-006), and that is a property of the shape rather than of
//! remembering to clean up: every step is a `?` inside
//! `conn.transaction::<_, CopyError, _>`.
//!
//! # What "independent" means here
//!
//! FR-012 forbids any referential link back to the source. So the copies carry
//! no source id, and the receipt this returns is **not stored** — a row naming
//! both the source collection and the records made from it is exactly the link
//! the one-time-deep-copy invariant exists to prevent. The receipt is handed to
//! the person who copied and then forgotten.
//!
//! # Ownership
//!
//! `created_by` and `updated_by` are the copier (FR-017a), and no permission
//! grant rows are created — the destination DM has implicit full control, the
//! same convention `copy_shared_ability_to_world_impl` records. FR-017b
//! (re-sharing what you received) then needs no code at all: the copies are
//! ordinary content in the recipient's world.

use async_graphql::{Error, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::world_membership::is_dm_of_world;
use crate::collections::MAX_MEMBERS;
use crate::collections::resolve::{MemberResolution, resolve_member};
use crate::graphql::mutations_collection_shares::{UNAVAILABLE, load_active_share, load_members};
use crate::models::CollectionMember;
use crate::state::AppState;

/// Newtype so the transaction closure returns one error type — the same
/// orphan-rule workaround `mutations_item_shares.rs` and
/// `mutations_ability_shares.rs` each declare for themselves.
pub struct CopyError(pub String);

impl From<diesel::result::Error> for CopyError {
    fn from(e: diesel::result::Error) -> Self {
        CopyError(e.to_string())
    }
}

impl From<String> for CopyError {
    fn from(s: String) -> Self {
        CopyError(s)
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct CopiedRecord {
    pub member_type: String,
    pub id: Uuid,
    pub name: String,
}

/// What arrived, and what did not.
///
/// Returned to the recipient, never stored — see the module documentation.
#[derive(SimpleObject, Debug, Clone)]
pub struct CopyReceipt {
    pub created: Vec<CopiedRecord>,
    /// FR-015: references that could not be brought across, and members that
    /// were withheld. Declared losses, not silent ones.
    pub fidelity_notes: Vec<String>,
}

/// Testable core of `copySharedCollectionToWorld`.
pub async fn copy_shared_collection_to_world_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    share_code: String,
    destination_world_id: Uuid,
) -> GraphQLResult<CopyReceipt> {
    // FR-009b / FR-016: viewing and copying diverge here. Viewing needed no
    // account; copying needs one, and authority in the destination.
    if !is_dm_of_world(state, user_id, is_admin, destination_world_id).await? {
        return Err(Error::new(
            "You must be the DM (Owner or GM) of the destination world to copy into it",
        ));
    }

    // Resolve membership outside the transaction, because moderation and
    // restriction checks are async. The transaction re-reads the share, so a
    // revocation between here and there still stops the copy.
    let collection_id = {
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let code = share_code.clone();
        tokio::task::spawn_blocking(move || {
            load_active_share(&mut conn, &code).map(|s| s.collection_id)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?
    };

    let members = load_members(state, collection_id).await?;

    let mut copyable: Vec<CollectionMember> = Vec::new();
    let mut fidelity_notes: Vec<String> = Vec::new();
    let mut withheld = 0usize;
    for member in members {
        match resolve_member(state, &member).await? {
            MemberResolution::Visible { .. } => copyable.push(member),
            // Unnamed, per FR-022. The recipient learns that something was
            // withheld, never what.
            MemberResolution::Withheld | MemberResolution::Gone => withheld += 1,
        }
    }
    if withheld > 0 {
        fidelity_notes.push(format!(
            "{withheld} item{} in this collection {} unavailable and {} not copied.",
            if withheld == 1 { "" } else { "s" },
            if withheld == 1 { "was" } else { "were" },
            if withheld == 1 { "was" } else { "were" },
        ));
    }

    // FR-024: nothing available is a refusal, not an empty success.
    if copyable.is_empty() {
        return Err(Error::new(
            "Nothing in this collection is available to copy",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        conn.transaction::<_, CopyError, _>(|conn| {
            // Re-validate inside the transaction: the link may have been
            // revoked between the preview and the confirm. The shipped copy
            // paths do this and say why.
            let share = load_active_share(conn, &share_code)?;
            if share.collection_id != collection_id {
                return Err(CopyError(UNAVAILABLE.to_string()));
            }

            // FR-005a re-asserted: a concurrent add must not push a collection
            // past the limit between the preview and the copy.
            if copyable.len() as i64 > MAX_MEMBERS {
                return Err(CopyError(format!(
                    "This collection holds more than the maximum of {MAX_MEMBERS} items"
                )));
            }

            let mut ctx = CopyContext {
                destination_world_id,
                user_id,
                ability_map: HashMap::new(),
                item_map: HashMap::new(),
                actor_map: HashMap::new(),
                lore_map: HashMap::new(),
                scene_map: HashMap::new(),
                created: Vec::new(),
                notes: fidelity_notes,
            };

            // Order matters: abilities and items first, so an actor copied
            // afterwards can point at the copies rather than the originals
            // (FR-014). A scene is copied before actors so a copied actor
            // could be placed in it, though actors are not re-parented today.
            for member in copyable.iter().filter(|m| m.member_type == "ability") {
                copy_ability(conn, &mut ctx, member.member_id)?;
            }
            for member in copyable.iter().filter(|m| m.member_type == "item") {
                copy_item(conn, &mut ctx, member.member_id)?;
            }
            for member in copyable.iter().filter(|m| m.member_type == "scene") {
                super::scene_copy::copy_scene(conn, &mut ctx, member.member_id)?;
            }
            for member in copyable.iter().filter(|m| m.member_type == "actor") {
                copy_actor(conn, &mut ctx, member.member_id)?;
            }
            for member in copyable.iter().filter(|m| m.member_type == "lore") {
                copy_lore(conn, &mut ctx, member.member_id)?;
            }

            Ok(CopyReceipt {
                created: ctx.created,
                fidelity_notes: ctx.notes,
            })
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| Error::new(e.0))
}

/// Carried through the copy so each step can re-point references at the copies
/// made by the steps before it.
pub struct CopyContext {
    pub destination_world_id: Uuid,
    pub user_id: Uuid,
    pub ability_map: HashMap<Uuid, Uuid>,
    pub item_map: HashMap<Uuid, Uuid>,
    pub actor_map: HashMap<Uuid, Uuid>,
    pub lore_map: HashMap<Uuid, Uuid>,
    pub scene_map: HashMap<Uuid, Uuid>,
    pub created: Vec<CopiedRecord>,
    pub notes: Vec<String>,
}

impl CopyContext {
    pub fn record(&mut self, member_type: &str, id: Uuid, name: &str) {
        self.created.push(CopiedRecord {
            member_type: member_type.to_string(),
            id,
            name: name.to_string(),
        });
    }

    /// FR-015: a reference to something outside the collection is a **declared
    /// loss**, never a silent drop.
    pub fn note_missing_reference(&mut self, owner: &str, referenced: &str) {
        self.notes.push(format!(
            "\"{owner}\" referred to \"{referenced}\", which was not in this collection and was not copied."
        ));
    }
}

fn copy_ability(
    conn: &mut PgConnection,
    ctx: &mut CopyContext,
    source_id: Uuid,
) -> Result<(), CopyError> {
    use crate::models::{AbilityEffect, NewAbilityEffect, NewWorldAbility, WorldAbility};
    use crate::schema::{world_abilities, world_ability_effects};

    let source = world_abilities::table
        .filter(world_abilities::id.eq(source_id))
        .select(WorldAbility::as_select())
        .first::<WorldAbility>(conn)?;

    let copy = diesel::insert_into(world_abilities::table)
        .values(&NewWorldAbility {
            world_id: ctx.destination_world_id,
            name: source.name.clone(),
            description: source.description.clone(),
            classification: source.classification.clone(),
            grade: source.grade,
            // Preserved rather than reset — fail closed. A copy arriving
            // un-hidden would silently expose content hidden at the source.
            // (A GM-only ability cannot be a collection member, but it can be
            // reached as an actor's known ability.)
            gm_only: source.gm_only,
            created_by: ctx.user_id,
            updated_by: ctx.user_id,
        })
        .returning(WorldAbility::as_returning())
        .get_result::<WorldAbility>(conn)?;

    let effects = world_ability_effects::table
        .filter(world_ability_effects::ability_id.eq(source_id))
        .order(world_ability_effects::sort_order.asc())
        .select(AbilityEffect::as_select())
        .load::<AbilityEffect>(conn)?;

    for effect in effects {
        // Re-validate rather than trusting the source's validity, which is an
        // assumption rather than a guarantee — the divergence
        // `copy_shared_ability_to_world_impl` documents.
        if effect.formula.trim().is_empty()
            || !effect.formula.chars().any(|c| c.is_ascii_alphanumeric())
        {
            return Err(CopyError(format!(
                "Source ability has an invalid effect formula: {:?}",
                effect.formula
            )));
        }
        diesel::insert_into(world_ability_effects::table)
            .values(&NewAbilityEffect {
                ability_id: copy.id,
                effect_type: effect.effect_type,
                formula: effect.formula,
                target: effect.target,
                trigger_kind: effect.trigger_kind,
                sort_order: effect.sort_order,
            })
            .execute(conn)?;
    }

    ctx.ability_map.insert(source_id, copy.id);
    ctx.record("ability", copy.id, &copy.name);
    Ok(())
}

fn copy_item(
    conn: &mut PgConnection,
    ctx: &mut CopyContext,
    source_id: Uuid,
) -> Result<(), CopyError> {
    use crate::schema::{world_item_effects, world_items};

    let (name, description, source_icon) = world_items::table
        .filter(world_items::id.eq(source_id))
        .select((
            world_items::name,
            world_items::description,
            world_items::icon_asset_id,
        ))
        .first::<(String, Option<String>, Option<Uuid>)>(conn)?;

    let new_id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_items::table)
        .values((
            world_items::id.eq(new_id),
            world_items::world_id.eq(ctx.destination_world_id),
            world_items::name.eq(&name),
            world_items::description.eq(&description),
            // FR-018: the icon travels. `icon_asset_id` is a bare object-storage
            // identifier with no foreign key and no world scoping (see the
            // header of the `world_actor_images` migration, which says so for
            // both tables), so pointing at it costs a column and not a stored
            // byte. This is the same sharing FR-019 asks for on scene
            // backgrounds, and it needs no second asset row because there is
            // no per-world asset row to duplicate.
            world_items::icon_asset_id.eq(source_icon),
            world_items::created_by.eq(ctx.user_id),
            world_items::created_at.eq(now),
            world_items::updated_at.eq(now),
        ))
        .execute(conn)?;

    let effects = world_item_effects::table
        .filter(world_item_effects::item_id.eq(source_id))
        .order(world_item_effects::sort_order.asc())
        .select((
            world_item_effects::effect_type,
            world_item_effects::formula,
            world_item_effects::target,
            world_item_effects::trigger_kind,
            world_item_effects::sort_order,
        ))
        .load::<(String, String, String, Option<String>, i32)>(conn)?;

    for (effect_type, formula, target, trigger_kind, sort_order) in effects {
        diesel::insert_into(world_item_effects::table)
            .values((
                world_item_effects::id.eq(Uuid::now_v7()),
                world_item_effects::item_id.eq(new_id),
                world_item_effects::effect_type.eq(effect_type),
                world_item_effects::formula.eq(formula),
                world_item_effects::target.eq(target),
                world_item_effects::trigger_kind.eq(trigger_kind),
                world_item_effects::sort_order.eq(sort_order),
                world_item_effects::created_at.eq(now),
                world_item_effects::updated_at.eq(now),
            ))
            .execute(conn)?;
    }

    ctx.item_map.insert(source_id, new_id);
    ctx.record("item", new_id, &name);
    Ok(())
}

fn copy_actor(
    conn: &mut PgConnection,
    ctx: &mut CopyContext,
    source_id: Uuid,
) -> Result<(), CopyError> {
    use crate::schema::{
        scenes, world_actor_abilities, world_actor_images, world_actor_inventory, world_actors,
        worlds,
    };

    let (label, description, actor_type, game_system_id, is_npc, source_scene) =
        world_actors::table
            .filter(world_actors::id.eq(source_id))
            .select((
                world_actors::label,
                world_actors::description,
                world_actors::actor_type,
                world_actors::game_system_id,
                world_actors::is_npc,
                world_actors::scene_id,
            ))
            .first::<(String, Option<String>, String, Option<String>, bool, Uuid)>(conn)?;

    // An actor needs a scene. If its own scene came along in the collection, it
    // lands there; otherwise FR-015a puts it in the destination world's
    // **active** scene and declares the displacement.
    //
    // The active scene rather than any scene, because "any" was what this did
    // first and it meant whichever row the database happened to return —
    // making the same copy into the same world land somewhere different on
    // different runs. The active scene is the one its new owner is looking at,
    // so a displaced actor turns up where they will see it rather than
    // somewhere they have to go hunting.
    let destination_scene = match ctx.scene_map.get(&source_scene) {
        Some(copied) => *copied,
        None => {
            let active: Option<Uuid> = worlds::table
                .filter(worlds::id.eq(ctx.destination_world_id))
                .select(worlds::active_scene_id)
                .first::<Option<Uuid>>(conn)
                .optional()?
                .flatten();

            // A world with no active scene is ordinary — nothing has been
            // launched yet — so fall back to its oldest scene rather than
            // refusing. Oldest rather than arbitrary keeps the result
            // repeatable, which is the half of FR-015a that survives even
            // when there is no active scene to honour.
            let landing = match active {
                Some(scene_id) => Some(scene_id),
                None => scenes::table
                    .filter(scenes::world_id.eq(ctx.destination_world_id))
                    .order(scenes::created_at.asc())
                    .select(scenes::scene_id)
                    .first::<Uuid>(conn)
                    .optional()?,
            };

            match landing {
                Some(scene_id) => {
                    ctx.notes.push(format!(
                        "\"{label}\" was placed in this world's current scene, because the scene it came from was not in this collection."
                    ));
                    scene_id
                }
                None => {
                    return Err(CopyError(format!(
                        "\"{label}\" needs a scene to live in, and the destination world has none. \
                         Create a scene there first, or include one in the collection."
                    )));
                }
            }
        }
    };

    let new_id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_actors::table)
        .values((
            world_actors::id.eq(new_id),
            world_actors::world_id.eq(ctx.destination_world_id),
            world_actors::scene_id.eq(destination_scene),
            world_actors::actor_type.eq(actor_type),
            world_actors::game_system_id.eq(game_system_id),
            world_actors::label.eq(&label),
            world_actors::description.eq(&description),
            world_actors::created_by.eq(ctx.user_id),
            world_actors::owned_by.eq(ctx.user_id),
            world_actors::is_public.eq(false),
            world_actors::is_npc.eq(is_npc),
            world_actors::created_at.eq(now),
            world_actors::updated_at.eq(now),
        ))
        .execute(conn)?;

    // FR-018: the actor's imagery travels with it.
    //
    // `world_actor_images.asset_id` is a bare object-storage identifier —
    // no foreign key, no world scoping, deliberately (see that table's
    // migration header). So the copy points at the same stored bytes and adds
    // no new object, exactly as FR-019 asks of a scene's background. Every
    // role comes across rather than a chosen one: `role` is open text by
    // ADR-054, so picking "portrait" here would silently drop whatever roles a
    // pack introduced later.
    let images = world_actor_images::table
        .filter(world_actor_images::actor_id.eq(source_id))
        .select((world_actor_images::role, world_actor_images::asset_id))
        .load::<(String, Uuid)>(conn)?;

    for (role, asset_id) in images {
        diesel::insert_into(world_actor_images::table)
            .values((
                world_actor_images::id.eq(Uuid::now_v7()),
                world_actor_images::actor_id.eq(new_id),
                world_actor_images::role.eq(role),
                world_actor_images::asset_id.eq(asset_id),
                world_actor_images::created_by.eq(ctx.user_id),
                world_actor_images::updated_by.eq(ctx.user_id),
                world_actor_images::created_at.eq(now),
                world_actor_images::updated_at.eq(now),
            ))
            .execute(conn)?;
    }

    // FR-014: an actor that knows an included ability must know the **copy**.
    let known = world_actor_abilities::table
        .filter(world_actor_abilities::actor_id.eq(source_id))
        .select((
            world_actor_abilities::ability_id,
            world_actor_abilities::ability_name_snapshot,
        ))
        .load::<(Option<Uuid>, String)>(conn)?;

    for (ability_id, snapshot) in known {
        let mapped = ability_id.and_then(|id| ctx.ability_map.get(&id).copied());
        if ability_id.is_some() && mapped.is_none() {
            // FR-015: the reference is kept as a name snapshot — which is what
            // the column is for — and declared as a loss.
            ctx.note_missing_reference(&label, &snapshot);
        }
        diesel::insert_into(world_actor_abilities::table)
            .values((
                world_actor_abilities::id.eq(Uuid::now_v7()),
                world_actor_abilities::actor_id.eq(new_id),
                world_actor_abilities::ability_id.eq(mapped),
                world_actor_abilities::ability_name_snapshot.eq(snapshot),
                world_actor_abilities::created_at.eq(now),
                world_actor_abilities::updated_at.eq(now),
            ))
            .execute(conn)?;
    }

    let inventory = world_actor_inventory::table
        .filter(world_actor_inventory::actor_id.eq(source_id))
        .select((
            world_actor_inventory::item_id,
            world_actor_inventory::item_name_snapshot,
            world_actor_inventory::quantity,
        ))
        .load::<(Option<Uuid>, String, i32)>(conn)?;

    for (item_id, snapshot, quantity) in inventory {
        let mapped = item_id.and_then(|id| ctx.item_map.get(&id).copied());
        if item_id.is_some() && mapped.is_none() {
            ctx.note_missing_reference(&label, &snapshot);
        }
        diesel::insert_into(world_actor_inventory::table)
            .values((
                world_actor_inventory::id.eq(Uuid::now_v7()),
                world_actor_inventory::actor_id.eq(new_id),
                world_actor_inventory::item_id.eq(mapped),
                world_actor_inventory::item_name_snapshot.eq(snapshot),
                world_actor_inventory::quantity.eq(quantity),
                world_actor_inventory::created_at.eq(now),
                world_actor_inventory::updated_at.eq(now),
                world_actor_inventory::created_by.eq(Some(ctx.user_id)),
                world_actor_inventory::updated_by.eq(Some(ctx.user_id)),
            ))
            .execute(conn)?;
    }

    ctx.actor_map.insert(source_id, new_id);
    ctx.record("actor", new_id, &label);
    Ok(())
}

fn copy_lore(
    conn: &mut PgConnection,
    ctx: &mut CopyContext,
    source_id: Uuid,
) -> Result<(), CopyError> {
    use crate::schema::world_lore_entries;

    let (title, slug, content) = world_lore_entries::table
        .filter(world_lore_entries::id.eq(source_id))
        .select((
            world_lore_entries::title,
            world_lore_entries::slug,
            world_lore_entries::content,
        ))
        .first::<(String, String, String)>(conn)?;

    // `UNIQUE (world_id, slug)`: copying twice into one world, or into the
    // world it came from, must produce two entries rather than a conflict
    // (FR-017, and the "copied into its own world" edge case).
    let slug = unique_lore_slug(conn, ctx.destination_world_id, &slug)?;

    let new_id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_lore_entries::table)
        .values((
            world_lore_entries::id.eq(new_id),
            world_lore_entries::world_id.eq(ctx.destination_world_id),
            world_lore_entries::title.eq(&title),
            world_lore_entries::slug.eq(&slug),
            world_lore_entries::content.eq(&content),
            // The revision history stays with the original. A copy is a new
            // document, not a fork of an edit history that belongs to someone
            // else's world.
            world_lore_entries::current_revision_id.eq(None::<Uuid>),
            world_lore_entries::created_by.eq(ctx.user_id),
            world_lore_entries::created_at.eq(now),
            world_lore_entries::updated_at.eq(now),
            // A copied entry is a root: its parent belongs to the source
            // world's tree.
            world_lore_entries::parent_id.eq(None::<Uuid>),
        ))
        .execute(conn)?;

    ctx.lore_map.insert(source_id, new_id);
    ctx.record("lore", new_id, &title);
    Ok(())
}

/// A slug free in this world, derived from the source's.
fn unique_lore_slug(
    conn: &mut PgConnection,
    world_id: Uuid,
    desired: &str,
) -> Result<String, CopyError> {
    use crate::schema::world_lore_entries;

    let mut candidate = desired.to_string();
    let mut suffix = 1;
    loop {
        let taken: i64 = world_lore_entries::table
            .filter(world_lore_entries::world_id.eq(world_id))
            .filter(world_lore_entries::slug.eq(&candidate))
            .count()
            .get_result(conn)?;
        if taken == 0 {
            return Ok(candidate);
        }
        suffix += 1;
        candidate = format!("{desired}-{suffix}");
        if suffix > 1000 {
            return Err(CopyError(
                "Could not find a free name for a copied lore entry".to_string(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "copy_tests.rs"]
mod tests;
