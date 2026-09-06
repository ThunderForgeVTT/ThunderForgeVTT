use super::*;

/// Record a takedown against one artifact, the way spec 015 does.
///
/// Written directly rather than through `submit_takedown_notice_impl` because
/// these tests are about what a *collection* does with a disabled member, not
/// about notice validation — and going through the notice path would make them
/// fail for reasons that belong to spec 015.
fn disable(conn: &mut PgConnection, entity_type: &str, entity_id: Uuid, world_id: Uuid) -> Uuid {
    use crate::schema::content_moderation_actions;
    let case_id = Uuid::now_v7();
    diesel::insert_into(content_moderation_actions::table)
        .values((
            content_moderation_actions::id.eq(Uuid::now_v7()),
            content_moderation_actions::case_id.eq(case_id),
            content_moderation_actions::action_type
                .eq(crate::moderation::action_type::CONTENT_DISABLED),
            content_moderation_actions::entity_type.eq(entity_type),
            content_moderation_actions::entity_id.eq(entity_id),
            content_moderation_actions::world_id.eq(world_id),
            content_moderation_actions::claimant_name.eq("A Rights Holder"),
            content_moderation_actions::claimant_contact.eq("rights@example.test"),
            content_moderation_actions::copyrighted_work_description.eq("The work"),
            content_moderation_actions::infringing_material_location.eq("here"),
            content_moderation_actions::good_faith_statement.eq(true),
            content_moderation_actions::accuracy_statement.eq(true),
            content_moderation_actions::signature.eq("A Rights Holder"),
            content_moderation_actions::created_at.eq(chrono::Utc::now()),
        ))
        .execute(conn)
        .expect("record the takedown");
    case_id
}

/// Undo one, the way a reversed takedown does.
fn restore(
    conn: &mut PgConnection,
    case_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
    world_id: Uuid,
) {
    use crate::schema::content_moderation_actions;
    diesel::insert_into(content_moderation_actions::table)
        .values((
            content_moderation_actions::id.eq(Uuid::now_v7()),
            content_moderation_actions::case_id.eq(case_id),
            content_moderation_actions::action_type
                .eq(crate::moderation::action_type::CONTENT_RESTORED),
            content_moderation_actions::entity_type.eq(entity_type),
            content_moderation_actions::entity_id.eq(entity_id),
            content_moderation_actions::world_id.eq(world_id),
            content_moderation_actions::claimant_name.eq("A Rights Holder"),
            content_moderation_actions::claimant_contact.eq("rights@example.test"),
            content_moderation_actions::copyrighted_work_description.eq("The work"),
            content_moderation_actions::infringing_material_location.eq("here"),
            content_moderation_actions::good_faith_statement.eq(true),
            content_moderation_actions::accuracy_statement.eq(true),
            content_moderation_actions::signature.eq("A Rights Holder"),
            content_moderation_actions::created_at.eq(chrono::Utc::now()),
        ))
        .execute(conn)
        .expect("record the restoration");
}

/// T039, FR-021/FR-023: a takedown reaches the copy path, not just the preview.
///
/// Asserted on the copy rather than inferred from the preview sharing a
/// resolver with it. The two call `resolve_member` independently, and "the
/// preview hides it" is not the claim FR-021 makes — the claim is that a copy
/// taken afterwards does not create it.
#[tokio::test]
async fn a_moderated_member_is_withheld_from_the_copy_and_the_rest_still_arrive() {
    let s = source();
    {
        let mut conn = s.state.db_pool.get().expect("connection");
        disable(&mut conn, "world_item", s.item_id, s.world_id);
    }

    let code = share_of(
        &s,
        &[
            ("item", s.item_id),
            ("ability", s.ability_id),
            ("lore", s.lore_id),
        ],
        "One taken down, two not",
    )
    .await;

    let receipt = copy_shared_collection_to_world_impl(
        &s.state,
        s.recipient_id,
        false,
        code,
        s.destination_world_id,
    )
    .await
    .expect("the collection still copies");

    assert!(
        !receipt.created.iter().any(|c| c.member_type == "item"),
        "FR-021: a disabled member must not be created by a copy: {:?}",
        receipt.created
    );
    assert_eq!(
        receipt.created.len(),
        2,
        "FR-023: disabling one member must not withhold the others: {:?}",
        receipt.created
    );

    // FR-022: the absence is declared, and the artifact is never named.
    let notes = receipt.fidelity_notes.join(" ");
    assert!(
        notes.contains("unavailable"),
        "the withholding must be declared: {notes}"
    );
    let name = {
        let mut conn = s.state.db_pool.get().expect("connection");
        use crate::schema::world_items;
        world_items::table
            .filter(world_items::id.eq(s.item_id))
            .select(world_items::name)
            .first::<String>(&mut conn)
            .expect("the item still exists")
    };
    assert!(
        !notes.contains(&name),
        "FR-022 forbids naming the withheld artifact, but the notes say: {notes}"
    );
}

/// T042, FR-025: reversing a takedown returns the member with nothing rebuilt.
///
/// This passes only if no status was cached anywhere — `effective_status`
/// resolves from the latest event every time. That is the design working, and
/// it is worth asserting precisely because the obvious optimisation (a
/// `moderated` flag on the member row) would break it silently.
#[tokio::test]
async fn a_reversed_takedown_returns_the_member_without_rebuilding_the_collection() {
    let s = source();
    let case_id = {
        let mut conn = s.state.db_pool.get().expect("connection");
        disable(&mut conn, "world_item", s.item_id, s.world_id)
    };

    let code = share_of(&s, &[("item", s.item_id)], "Taken down then restored").await;

    let while_disabled = crate::graphql::mutations_collection_shares::shared_collection_impl(
        &s.state,
        "203.0.113.7",
        code.clone(),
    )
    .await;
    assert!(
        while_disabled.is_err(),
        "FR-024: a collection whose every member is withheld reports nothing available"
    );

    {
        let mut conn = s.state.db_pool.get().expect("connection");
        restore(&mut conn, case_id, "world_item", s.item_id, s.world_id);
    }

    // The same code, the same collection, nothing touched in between.
    let after = crate::graphql::mutations_collection_shares::shared_collection_impl(
        &s.state,
        "203.0.113.7",
        code,
    )
    .await
    .expect("the member returns without the owner rebuilding anything");
    assert_eq!(after.members.len(), 1);
    assert_eq!(after.withheld_count, 0);
}

/// T044, FR-001b, the direction the add-time check cannot cover.
///
/// `resolve.rs` already proves that restricting an ability after adding it
/// withholds it. The other half matters as much and is easier to get wrong:
/// lifting the restriction must return it, with no rebuild — which it does
/// only because nothing about the restriction was ever written down on the
/// member row.
#[tokio::test]
async fn lifting_a_restriction_returns_the_member_to_the_collection() {
    use crate::schema::world_abilities;
    let s = source();

    let set_gm_only = |flag: bool| {
        let mut conn = s.state.db_pool.get().expect("connection");
        diesel::update(world_abilities::table.filter(world_abilities::id.eq(s.ability_id)))
            .set(world_abilities::gm_only.eq(flag))
            .execute(&mut conn)
            .expect("toggle gm_only");
    };

    let code = share_of(
        &s,
        &[("ability", s.ability_id), ("lore", s.lore_id)],
        "A restriction that comes and goes",
    )
    .await;

    set_gm_only(true);
    let restricted = crate::graphql::mutations_collection_shares::shared_collection_impl(
        &s.state,
        "203.0.113.8",
        code.clone(),
    )
    .await
    .expect("the collection still opens");
    assert_eq!(restricted.members.len(), 1, "the ability is withheld");
    assert_eq!(restricted.withheld_count, 1);

    set_gm_only(false);
    let lifted = crate::graphql::mutations_collection_shares::shared_collection_impl(
        &s.state,
        "203.0.113.8",
        code,
    )
    .await
    .expect("the collection still opens");
    assert_eq!(
        lifted.members.len(),
        2,
        "FR-001b's other direction: lifting the restriction returns the member"
    );
    assert_eq!(lifted.withheld_count, 0);
}
