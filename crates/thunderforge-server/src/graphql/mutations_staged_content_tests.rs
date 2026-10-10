//! Spec 048 T054: deciding on what players brought. Adopt writes one world
//! row and repoints every link (FR-034); adopt all is a snapshot (FR-033a);
//! decline and revisit move between the three states (FR-036b, FR-036c); a
//! Player is refused, even on their own (FR-033b); each decision records
//! event 41.
//!
//! The pieces are staged directly, as an import leaves them; the one case
//! that needs the importer's own resolution (the same piece from two
//! characters) applies the generated fixture twice.

use diesel::prelude::*;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::compendium::origin::ContentOrigin;
use crate::sheet_import::apply::{ApplyInput, apply_sheet_import_impl, read_natively};
use crate::sheet_import::preview::sheet_import_preview_impl;
use crate::staged_content::StagedState;
use crate::staged_content::decide::{adopt, adopt_all, decline, list, revisit};
use crate::state::AppState;
use crate::test_support::{
    insert_test_actor, insert_test_scene, insert_test_user, insert_test_world,
    insert_test_world_member, test_app_state,
};
use crate::world_events::EVENT_CODE_STAGED_CONTENT_DECIDED;

pub(crate) struct World {
    pub(crate) state: AppState,
    pub(crate) gm: Uuid,
    pub(crate) trusted: Uuid,
    pub(crate) player: Uuid,
    pub(crate) other: Uuid,
    pub(crate) stranger: Uuid,
    pub(crate) world: Uuid,
    pub(crate) scene: Uuid,
}

pub(crate) fn world() -> World {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let gm = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, gm);
    insert_test_world_member(&mut conn, world, gm, "GM");
    let mut member = |role: &str| {
        let id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world, id, role);
        id
    };
    let trusted = member("TrustedPlayer");
    let player = member("Player");
    let other = member("Player");
    let stranger = insert_test_user(&mut conn);
    let scene = insert_test_scene(&mut conn, world, gm);
    drop(conn);
    World {
        state,
        gm,
        trusted,
        player,
        other,
        stranger,
        world,
        scene,
    }
}

impl World {
    pub(crate) fn conn(
        &self,
    ) -> diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>> {
        self.state.db_pool.get().expect("conn")
    }

    pub(crate) fn actor(&self) -> Uuid {
        insert_test_actor(&mut self.conn(), self.world, self.scene, self.gm)
    }

    /// A pending piece `by` brought, with the given fields.
    pub(crate) fn stage(&self, by: Uuid, kind: &str, name: &str, fields: Value) -> Uuid {
        use crate::schema::world_staged_content as staged;
        let hash = format!("{:0>64}", Uuid::now_v7().simple());
        diesel::insert_into(staged::table)
            .values(crate::staged_content::NewStagedContent {
                world_id: self.world,
                player_user_id: by,
                kind,
                name,
                normalized_name: &name.to_lowercase(),
                content_hash: &hash,
                field_values: &fields,
                origin: ContentOrigin::Uploaded,
                differs_from: None,
                first_actor_id: None,
                created_by: by,
                updated_by: by,
            })
            .returning(staged::id)
            .get_result(&mut self.conn())
            .expect("staged")
    }

    /// The actor holds the piece as an ability link.
    pub(crate) fn link_ability(&self, actor: Uuid, staged: Uuid, name: &str) -> Uuid {
        use crate::schema::world_actor_abilities as abilities;
        diesel::insert_into(abilities::table)
            .values((
                abilities::actor_id.eq(actor),
                abilities::staged_id.eq(Some(staged)),
                abilities::ability_name_snapshot.eq(name),
            ))
            .returning(abilities::id)
            .get_result(&mut self.conn())
            .expect("linked")
    }

    pub(crate) fn link_item(&self, actor: Uuid, staged: Uuid, name: &str) -> Uuid {
        use crate::schema::world_actor_inventory as inventory;
        diesel::insert_into(inventory::table)
            .values((
                inventory::actor_id.eq(actor),
                inventory::staged_id.eq(Some(staged)),
                inventory::item_name_snapshot.eq(name),
                inventory::quantity.eq(1),
            ))
            .returning(inventory::id)
            .get_result(&mut self.conn())
            .expect("carried")
    }

    pub(crate) fn ability_link(&self, link: Uuid) -> (Option<Uuid>, Option<Uuid>) {
        use crate::schema::world_actor_abilities as abilities;
        abilities::table
            .find(link)
            .select((abilities::ability_id, abilities::staged_id))
            .first(&mut self.conn())
            .unwrap()
    }

    pub(crate) fn state_of(&self, staged: Uuid) -> StagedState {
        use crate::schema::world_staged_content as s;
        s::table
            .find(staged)
            .select(s::state)
            .first(&mut self.conn())
            .unwrap()
    }

    /// The event-41 payloads naming this piece.
    pub(crate) fn decisions(&self, staged: Uuid) -> Vec<Value> {
        use crate::schema::world_events as events;
        events::table
            .filter(events::world_id.eq(self.world))
            .filter(events::event_code.eq(EVENT_CODE_STAGED_CONTENT_DECIDED))
            .order(events::id.asc())
            .select(events::token_event)
            .load::<Option<Value>>(&mut self.conn())
            .unwrap()
            .into_iter()
            .flatten()
            .filter(|payload| payload["stagedId"] == json!(staged))
            .collect()
    }

    pub(crate) fn abilities_named(
        &self,
        name: &str,
    ) -> Vec<(Uuid, ContentOrigin, Uuid, String, Option<i32>)> {
        use crate::schema::world_abilities as a;
        a::table
            .filter(a::world_id.eq(self.world))
            .filter(a::name.eq(name))
            .select((a::id, a::origin, a::created_by, a::classification, a::grade))
            .load(&mut self.conn())
            .unwrap()
    }
}

#[test]
fn an_adopt_writes_one_uploaded_row_and_repoints_every_link() {
    let w = world();
    let (first, second) = (w.actor(), w.actor());
    let piece = w.stage(
        w.player,
        "spell",
        "Ember Lance",
        json!({ "level": 2, "description": "A line of fire." }),
    );
    let a = w.link_ability(first, piece, "Ember Lance");
    let b = w.link_ability(second, piece, "Ember Lance");

    let adopted = adopt(&mut w.conn(), w.gm, piece).expect("the GM adopts");
    assert_eq!(adopted.state, StagedState::Adopted);
    assert_eq!(adopted.decided_by, Some(w.gm));
    assert!(adopted.decided_at.is_some());

    let rows = w.abilities_named("Ember Lance");
    assert_eq!(rows.len(), 1, "one world row, no duplicate");
    let (id, origin, made_by, classification, grade) = rows[0].clone();
    assert_eq!(origin, ContentOrigin::Uploaded, "adoption keeps the origin");
    assert_eq!(made_by, w.player, "made by the player who brought it");
    assert_eq!(classification, "spell");
    assert_eq!(grade, Some(2));
    assert_eq!(adopted.adopted_ability_id, Some(id));
    for link in [a, b] {
        assert_eq!(
            w.ability_link(link),
            (Some(id), None),
            "the link uses the world's"
        );
    }

    let events = w.decisions(piece);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["state"], "adopted");
    let mut actors = vec![json!(first), json!(second)];
    actors.sort_by_key(|v| v.as_str().unwrap().to_string());
    assert_eq!(events[0]["actorIds"], Value::Array(actors));

    // Adopting again changes nothing and writes no second row.
    adopt(&mut w.conn(), w.gm, piece).expect("idempotent");
    assert_eq!(w.abilities_named("Ember Lance").len(), 1);
    assert_eq!(w.decisions(piece).len(), 1);
}

#[test]
fn an_adopted_item_is_a_world_item_and_the_inventory_uses_it() {
    use crate::schema::{world_actor_inventory as inventory, world_items as items};
    let w = world();
    let actor = w.actor();
    let piece = w.stage(w.player, "item", "Rope of Knots", json!({ "weight": 5 }));
    let link = w.link_item(actor, piece, "Rope of Knots");

    let adopted = adopt(&mut w.conn(), w.gm, piece).expect("adopted");
    let item = adopted.adopted_item_id.expect("an item");
    let (origin, weight): (ContentOrigin, Option<f64>) = items::table
        .find(item)
        .select((items::origin, items::weight))
        .first(&mut w.conn())
        .unwrap();
    assert_eq!(origin, ContentOrigin::Uploaded);
    assert_eq!(weight, Some(5.0));
    let held: (Option<Uuid>, Option<Uuid>) = inventory::table
        .find(link)
        .select((inventory::item_id, inventory::staged_id))
        .first(&mut w.conn())
        .unwrap();
    assert_eq!(held, (Some(item), None));
}

#[test]
fn adopt_all_takes_what_is_pending_now_and_nothing_staged_after() {
    let w = world();
    let one = w.stage(w.player, "feat", "Keen Watch", json!({}));
    let two = w.stage(w.player, "feature", "Second Wind Echo", json!({}));
    let declined = w.stage(w.player, "feat", "Odd Luck", json!({}));
    decline(&mut w.conn(), w.gm, declined).expect("declined");
    let someone_else = w.stage(w.other, "feat", "Quiet Step", json!({}));

    let adopted = adopt_all(&mut w.conn(), w.gm, w.world, w.player).expect("adopt all");
    let mut ids: Vec<Uuid> = adopted.iter().map(|p| p.id).collect();
    ids.sort();
    let mut expected = vec![one, two];
    expected.sort();
    assert_eq!(ids, expected, "only that player's pending pieces");
    assert_eq!(w.state_of(declined), StagedState::Declined);
    assert_eq!(w.state_of(someone_else), StagedState::Pending);

    let later = w.stage(w.player, "feat", "Late Arrival", json!({}));
    assert_eq!(w.state_of(later), StagedState::Pending, "a snapshot");
    assert_eq!(w.decisions(one).len(), 1);
    assert_eq!(w.decisions(two).len(), 1);
}

#[test]
fn decline_and_revisit_move_between_the_three_states() {
    let w = world();
    let piece = w.stage(w.player, "feat", "Grim Resolve", json!({}));

    assert_eq!(
        decline(&mut w.conn(), w.gm, piece).unwrap().state,
        StagedState::Declined
    );
    assert_eq!(
        revisit(&mut w.conn(), w.gm, piece, StagedState::Pending)
            .unwrap()
            .state,
        StagedState::Pending
    );
    // Only a declined piece is revisited.
    assert_eq!(
        revisit(&mut w.conn(), w.gm, piece, StagedState::Adopted)
            .unwrap_err()
            .code(),
        "VALIDATION_FAILED"
    );
    decline(&mut w.conn(), w.gm, piece).unwrap();
    let adopted = revisit(&mut w.conn(), w.gm, piece, StagedState::Adopted).unwrap();
    assert_eq!(adopted.state, StagedState::Adopted);
    assert_eq!(w.abilities_named("Grim Resolve").len(), 1);
    // An adopted piece is the world's now.
    assert_eq!(
        decline(&mut w.conn(), w.gm, piece).unwrap_err().code(),
        "VALIDATION_FAILED"
    );

    let states: Vec<Value> = w
        .decisions(piece)
        .into_iter()
        .map(|e| e["state"].clone())
        .collect();
    assert_eq!(
        states,
        vec![
            json!("declined"),
            json!("pending"),
            json!("declined"),
            json!("adopted")
        ]
    );
}

#[test]
fn a_player_is_refused_even_on_their_own_and_a_stranger_is_too() {
    let w = world();
    let piece = w.stage(w.player, "feat", "Bold Claim", json!({}));
    for user in [w.player, w.other, w.stranger] {
        let codes = [
            adopt(&mut w.conn(), user, piece).unwrap_err().code(),
            adopt_all(&mut w.conn(), user, w.world, w.player)
                .unwrap_err()
                .code(),
            decline(&mut w.conn(), user, piece).unwrap_err().code(),
            revisit(&mut w.conn(), user, piece, StagedState::Adopted)
                .unwrap_err()
                .code(),
        ];
        assert_eq!(codes, ["FORBIDDEN"; 4]);
    }
    assert_eq!(w.state_of(piece), StagedState::Pending);
    assert!(w.abilities_named("Bold Claim").is_empty());
    assert!(w.decisions(piece).is_empty());
}

#[test]
fn a_trusted_player_may_decide() {
    let w = world();
    let piece = w.stage(w.player, "feat", "Steady Hand", json!({}));
    let declined = w.stage(w.player, "feat", "Shaky Hand", json!({}));
    adopt(&mut w.conn(), w.trusted, piece).expect("a Trusted Player adopts");
    decline(&mut w.conn(), w.trusted, declined).expect("and declines");
    assert_eq!(w.state_of(piece), StagedState::Adopted);
    assert_eq!(w.state_of(declined), StagedState::Declined);
}

#[test]
fn the_same_name_with_different_content_is_two_pieces_and_each_is_decided_alone() {
    use crate::schema::world_staged_content as staged;
    let w = world();
    let first = w.stage(w.player, "spell", "Frost Bloom", json!({ "level": 1 }));
    let second = w.stage(w.other, "spell", "Frost Bloom", json!({ "level": 3 }));
    diesel::update(staged::table.find(second))
        .set(staged::differs_from.eq(Some(first)))
        .execute(&mut w.conn())
        .unwrap();

    let seen = list(&mut w.conn(), w.gm, w.world, None, None).unwrap();
    let named: Vec<_> = seen.iter().filter(|p| p.name == "Frost Bloom").collect();
    assert_eq!(named.len(), 2);
    assert_eq!(
        named.iter().find(|p| p.id == second).unwrap().differs_from,
        Some(first)
    );
    adopt(&mut w.conn(), w.gm, first).unwrap();
    assert_eq!(w.state_of(second), StagedState::Pending);
}

#[test]
fn the_queue_shows_everything_to_the_gm_and_a_player_only_their_own() {
    let w = world();
    let mine = w.stage(w.player, "feat", "Mine", json!({}));
    let theirs = w.stage(w.other, "feat", "Theirs", json!({}));
    decline(&mut w.conn(), w.gm, theirs).unwrap();

    let ids = |user: Uuid, state: Option<StagedState>, player: Option<Uuid>| -> Vec<Uuid> {
        list(&mut w.conn(), user, w.world, state, player)
            .unwrap()
            .into_iter()
            .map(|p| p.id)
            .collect()
    };
    assert_eq!(ids(w.gm, None, None), vec![mine, theirs]);
    assert_eq!(ids(w.trusted, None, None), vec![mine, theirs]);
    assert_eq!(ids(w.gm, Some(StagedState::Declined), None), vec![theirs]);
    assert_eq!(ids(w.gm, None, Some(w.player)), vec![mine]);
    assert_eq!(ids(w.player, None, None), vec![mine]);
    assert_eq!(ids(w.player, None, Some(w.other)), Vec::<Uuid>::new());
    assert_eq!(
        list(&mut w.conn(), w.stranger, w.world, None, None)
            .unwrap_err()
            .code(),
        "FORBIDDEN"
    );
}

const FIGHTER_WIZARD: &[u8] =
    include_bytes!("../../../../packs/systems/dnd5e/sheet/tests/fixtures/fighter3-wizard2.pdf");

/// The same unknown piece from two characters is one staged row (FR-035),
/// and adopting it serves both.
#[tokio::test]
async fn the_same_piece_from_two_characters_is_one_row_and_one_adoption_serves_both() {
    use crate::schema::{
        instance_settings, world_actor_abilities as abilities, world_staged_content,
    };
    let w = world();
    {
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(instance_settings::table)
            .values((
                instance_settings::key.eq(crate::settings::features::SHEET_IMPORT),
                instance_settings::value.eq("true"),
                instance_settings::updated_at.eq(now),
                instance_settings::created_at.eq(now),
            ))
            .on_conflict(instance_settings::key)
            .do_update()
            .set(instance_settings::value.eq("true"))
            .execute(&mut w.conn())
            .expect("flag on");
    }
    let systems = format!("{}/../../packs/systems", env!("CARGO_MANIFEST_DIR"));
    let (_, reading) = read_natively("dnd5e", FIGHTER_WIZARD).expect("reads");
    let actors = [w.actor(), w.actor()];
    for actor in actors {
        let plan =
            sheet_import_preview_impl(&w.state, &systems, w.gm, false, actor, &reading, None)
                .await
                .expect("a plan");
        apply_sheet_import_impl(
            &w.state,
            &systems,
            w.gm,
            false,
            ApplyInput {
                actor_id: actor,
                bytes: FIGHTER_WIZARD.to_vec(),
                corrections: None,
                overwrite_play_state: Vec::new(),
                plan_hash: thunderforge_sheet_import::plan_hash(&plan),
            },
        )
        .await
        .expect("applied");
    }

    let pieces: Vec<(Uuid, String)> = world_staged_content::table
        .filter(world_staged_content::world_id.eq(w.world))
        .filter(world_staged_content::kind.ne("item"))
        .select((world_staged_content::id, world_staged_content::name))
        .load(&mut w.conn())
        .unwrap();
    let mut names: Vec<&String> = pieces.iter().map(|(_, n)| n).collect();
    let all = names.len();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), all, "each piece is staged once");
    let (piece, name) = pieces.first().expect("the fixture stages content").clone();
    let holders: Vec<Uuid> = abilities::table
        .filter(abilities::staged_id.eq(piece))
        .select(abilities::actor_id)
        .load(&mut w.conn())
        .unwrap();
    assert_eq!(holders.len(), 2, "both characters link the one row");

    let adopted = adopt(&mut w.conn(), w.gm, piece).unwrap();
    let id = adopted.adopted_ability_id.expect("an ability");
    for actor in actors {
        let linked: Vec<Option<Uuid>> = abilities::table
            .filter(abilities::actor_id.eq(actor))
            .filter(abilities::ability_name_snapshot.eq(&name))
            .select(abilities::ability_id)
            .load(&mut w.conn())
            .unwrap();
        assert_eq!(linked, vec![Some(id)]);
    }
}
