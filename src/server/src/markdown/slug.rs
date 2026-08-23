//! Spec 012 (research.md §3): title → urlified slug, disambiguated on
//! collision within a world. No general-purpose slugify utility existed
//! anywhere in this codebase before this feature (only
//! `game_systems.slug`, a manually-assigned package identifier,
//! unrelated) — the `slug` crate handles Unicode-to-ASCII transliteration
//! correctly, which a hand-rolled regex would not (FR-012, FR-013).

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::world_lore_entries;

/// Pure title → slug transliteration (no DB access, no disambiguation).
pub fn slugify_title(title: &str) -> String {
    let slug = slug::slugify(title);
    if slug.is_empty() {
        "entry".to_string()
    } else {
        slug
    }
}

/// Computes a slug for `title` that is unique within `world_id`,
/// appending the first free numeric suffix (`-2`, `-3`, ...) on
/// collision (FR-013). `exclude_entry_id` excludes one entry's own
/// existing row from the collision check — pass the entry's own id when
/// regenerating its slug on a title change (FR-014), so an entry never
/// collides with its own prior slug.
///
/// Blocking (issues synchronous Diesel queries) — callers run this
/// inside `tokio::task::spawn_blocking`, matching every other Diesel call
/// site in this codebase.
pub fn unique_slug_for_world(
    conn: &mut PgConnection,
    world_id: Uuid,
    title: &str,
    exclude_entry_id: Option<Uuid>,
) -> Result<String, diesel::result::Error> {
    let base = slugify_title(title);
    let mut candidate = base.clone();
    let mut suffix = 2;

    loop {
        let mut query = world_lore_entries::table
            .filter(world_lore_entries::world_id.eq(world_id))
            .filter(world_lore_entries::slug.eq(&candidate))
            .into_boxed();
        if let Some(exclude_id) = exclude_entry_id {
            query = query.filter(world_lore_entries::id.ne(exclude_id));
        }

        let exists: bool = diesel::select(diesel::dsl::exists(query)).get_result(conn)?;
        if !exists {
            return Ok(candidate);
        }

        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

    #[test]
    fn slugify_title_ascii_folds_and_kebab_cases() {
        assert_eq!(slugify_title("Ancient Ruins of Veldrath"), "ancient-ruins-of-veldrath");
        assert_eq!(slugify_title("Château Noir"), "chateau-noir");
    }

    #[test]
    fn slugify_title_falls_back_for_empty_result() {
        // A title of only symbols/whitespace transliterates to nothing;
        // fall back to a non-empty placeholder rather than an empty slug.
        assert_eq!(slugify_title("!!!"), "entry");
    }

    fn insert_entry_with_slug(
        conn: &mut PgConnection,
        world_id: Uuid,
        created_by: Uuid,
        slug: &str,
    ) -> Uuid {
        let id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_lore_entries::table)
            .values((
                world_lore_entries::id.eq(id),
                world_lore_entries::world_id.eq(world_id),
                world_lore_entries::title.eq("Whatever"),
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

    /// FR-013: a colliding slug is disambiguated with a numeric suffix.
    #[tokio::test]
    async fn disambiguates_colliding_slug_with_numeric_suffix() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_entry_with_slug(&mut conn, world_id, owner_id, "ancient-ruins");

        let slug = unique_slug_for_world(&mut conn, world_id, "Ancient Ruins", None)
            .expect("slug generation should succeed");
        assert_eq!(slug, "ancient-ruins-2");

        insert_entry_with_slug(&mut conn, world_id, owner_id, "ancient-ruins-2");
        let slug = unique_slug_for_world(&mut conn, world_id, "Ancient Ruins", None)
            .expect("slug generation should succeed");
        assert_eq!(slug, "ancient-ruins-3");
    }

    /// FR-014: regenerating an entry's own slug excludes its own existing
    /// row from the collision check, so re-saving the same title doesn't
    /// spuriously disambiguate against itself.
    #[tokio::test]
    async fn excludes_own_entry_from_collision_check() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let entry_id = insert_entry_with_slug(&mut conn, world_id, owner_id, "ancient-ruins");

        let slug =
            unique_slug_for_world(&mut conn, world_id, "Ancient Ruins", Some(entry_id))
                .expect("slug generation should succeed");
        assert_eq!(slug, "ancient-ruins", "an entry's own slug must not collide with itself");
    }

    /// Slugs are scoped per-world — the same title in two different
    /// worlds does not disambiguate against each other.
    #[tokio::test]
    async fn slugs_are_scoped_per_world() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_a = insert_test_world(&mut conn, owner_id);
        let world_b = insert_test_world(&mut conn, owner_id);
        insert_entry_with_slug(&mut conn, world_a, owner_id, "ancient-ruins");

        let slug = unique_slug_for_world(&mut conn, world_b, "Ancient Ruins", None)
            .expect("slug generation should succeed");
        assert_eq!(slug, "ancient-ruins", "a different world's same title must not collide");
    }
}
