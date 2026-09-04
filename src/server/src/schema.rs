// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "CanvasImageAssetKind"))]
    pub struct CanvasImageAssetKind;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "PolicyEffect"))]
    pub struct PolicyEffect;
}

diesel::table! {
    admin_bootstrap_oauth_sessions (id) {
        id -> Uuid,
        provider_id -> Uuid,
        oauth_provider_key -> Varchar,
        oauth_client_id -> Varchar,
        state -> Varchar,
        code_verifier -> Varchar,
        redirect_uri -> Varchar,
        desired_username -> Nullable<Varchar>,
        return_to -> Nullable<Varchar>,
        expires_at -> Timestamp,
        consumed_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    admin_bootstrap_setup (id) {
        id -> Int4,
        setup_completed_at -> Nullable<Timestamp>,
        admin_code_hash -> Nullable<Varchar>,
        admin_code_generated_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    auth_security_settings (id) {
        id -> Int4,
        two_factor_required_for_all_users -> Bool,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::CanvasImageAssetKind;

    canvas_image_assets (asset_id) {
        asset_id -> Uuid,
        world_id -> Uuid,
        scene_id -> Nullable<Uuid>,
        owner_user_id -> Uuid,
        storage_path -> Text,
        original_format -> Text,
        width_px -> Int4,
        height_px -> Int4,
        byte_size -> Int8,
        kind -> CanvasImageAssetKind,
        created_by -> Uuid,
        updated_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        content_hash -> Nullable<Text>,
    }
}

diesel::table! {
    content_moderation_actions (id) {
        id -> Uuid,
        case_id -> Uuid,
        action_type -> Text,
        entity_type -> Text,
        entity_id -> Uuid,
        world_id -> Uuid,
        account_id -> Nullable<Uuid>,
        claimant_name -> Text,
        claimant_contact -> Text,
        copyrighted_work_description -> Text,
        infringing_material_location -> Text,
        good_faith_statement -> Bool,
        accuracy_statement -> Bool,
        signature -> Text,
        validity_result -> Nullable<Text>,
        missing_elements -> Nullable<Array<Nullable<Text>>>,
        counter_notice_id -> Nullable<Uuid>,
        restoration_due_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        created_by -> Nullable<Uuid>,
    }
}

diesel::table! {
    fog_masks (fog_id) {
        fog_id -> Uuid,
        scene_id -> Uuid,
        bitmap_data -> Bytea,
        version -> Int4,
        width -> Int4,
        height -> Int4,
        updated_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    game_systems (id) {
        id -> Uuid,
        slug -> Text,
        title -> Text,
        manifest_url -> Text,
        version -> Text,
        installed_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    interaction_requests (request_id) {
        request_id -> Uuid,
        interactive_id -> Uuid,
        scene_id -> Uuid,
        requested_by -> Uuid,
        state -> Varchar,
        decided_by -> Nullable<Uuid>,
        decided_at -> Nullable<Timestamp>,
        created_by -> Uuid,
        updated_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    interactives (interactive_id) {
        interactive_id -> Uuid,
        scene_id -> Uuid,
        subject_kind -> Varchar,
        subject_ref -> Nullable<Uuid>,
        geometry -> Nullable<Jsonb>,
        effect_id -> Nullable<Varchar>,
        effect_config -> Nullable<Jsonb>,
        trigger -> Varchar,
        activation -> Varchar,
        fire_mode -> Varchar,
        fired_at -> Nullable<Timestamp>,
        created_by -> Uuid,
        updated_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    light_sources (light_id) {
        light_id -> Uuid,
        scene_id -> Uuid,
        x -> Float8,
        y -> Float8,
        radius -> Float8,
        intensity -> Float8,
        color -> Nullable<Text>,
        attached_token_id -> Nullable<Uuid>,
        casts_shadows -> Bool,
        metadata -> Nullable<Jsonb>,
        created_by -> Uuid,
        updated_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    login_two_factor_challenges (id) {
        id -> Uuid,
        user_id -> Uuid,
        expires_at -> Timestamp,
        consumed_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    oauth_authorization_sessions (id) {
        id -> Uuid,
        provider_id -> Uuid,
        oauth_provider_key -> Varchar,
        oauth_client_id -> Varchar,
        state -> Varchar,
        code_verifier -> Varchar,
        redirect_uri -> Varchar,
        return_to -> Nullable<Varchar>,
        expires_at -> Timestamp,
        consumed_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    oauth_link_challenges (id) {
        id -> Uuid,
        user_id -> Uuid,
        provider_id -> Uuid,
        provider_user_id -> Varchar,
        provider_email -> Nullable<Varchar>,
        challenge_code -> Varchar,
        expires_at -> Timestamp,
        consumed_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
        pending_access_token_encrypted -> Nullable<Text>,
        pending_refresh_token_encrypted -> Nullable<Text>,
        pending_token_expires_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    oauth_providers (id) {
        id -> Uuid,
        provider_key -> Varchar,
        display_name -> Varchar,
        authorization_url -> Varchar,
        token_url -> Varchar,
        userinfo_url -> Nullable<Varchar>,
        scopes -> Array<Nullable<Text>>,
        enabled -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        oauth_client_id -> Nullable<Varchar>,
        oauth_client_secret -> Nullable<Varchar>,
        configured -> Bool,
        config_source -> Varchar,
    }
}

diesel::table! {
    players_online (id) {
        id -> Int8,
        player_id -> Uuid,
        world_id -> Uuid,
        scene_id -> Nullable<Uuid>,
        connected_at -> Timestamp,
        last_seen -> Timestamp,
        idle_duration_secs -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::PolicyEffect;

    policies (id) {
        id -> Uuid,
        effect -> PolicyEffect,
        resources -> Array<Nullable<Text>>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        created_by -> Uuid,
        updated_by -> Uuid,
        world_id -> Nullable<Uuid>,
    }
}

diesel::table! {
    scene_preview_images (id) {
        id -> Uuid,
        scene_id -> Uuid,
        byte_size -> Int8,
        created_at -> Timestamp,
    }
}

diesel::table! {
    scene_state_fingerprints (scene_id) {
        scene_id -> Uuid,
        content_hash -> Text,
        canonical_version -> Int4,
        computed_at -> Timestamp,
        updated_by -> Uuid,
    }
}

diesel::table! {
    scenes (scene_id) {
        scene_id -> Uuid,
        world_id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        #[sql_name = "type"]
        type_ -> Text,
        grid_size -> Int4,
        grid_type -> Text,
        width -> Int4,
        height -> Int4,
        metadata -> Nullable<Jsonb>,
        owner_id -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        background_image_path -> Nullable<Text>,
        background_asset_id -> Nullable<Uuid>,
        summary_markdown -> Nullable<Text>,
        summary_rendered_html -> Nullable<Text>,
        hidden -> Bool,
        preview_asset_id -> Nullable<Uuid>,
    }
}

diesel::table! {
    shapes (shape_id) {
        shape_id -> Uuid,
        scene_id -> Uuid,
        kind -> Text,
        geometry -> Jsonb,
        text -> Nullable<Text>,
        style -> Nullable<Jsonb>,
        visible_to_players -> Bool,
        metadata -> Nullable<Jsonb>,
        created_by -> Uuid,
        updated_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    token_resource_disclosure (id) {
        id -> Uuid,
        token_id -> Uuid,
        resource_id -> Varchar,
        state -> Varchar,
        created_by -> Uuid,
        updated_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    tokens (token_id) {
        token_id -> Uuid,
        scene_id -> Uuid,
        actor_id -> Nullable<Uuid>,
        x -> Float8,
        y -> Float8,
        rotation -> Float8,
        scale -> Float8,
        metadata -> Nullable<Jsonb>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        owner_user_id -> Nullable<Uuid>,
        is_primary -> Bool,
        photo_url -> Nullable<Text>,
        health -> Nullable<Int4>,
        max_health -> Nullable<Int4>,
        token_type -> Varchar,
    }
}

diesel::table! {
    user_oauth_accounts (id) {
        id -> Uuid,
        user_id -> Uuid,
        provider_id -> Uuid,
        provider_user_id -> Varchar,
        provider_email -> Nullable<Varchar>,
        access_token_encrypted -> Nullable<Text>,
        refresh_token_encrypted -> Nullable<Text>,
        token_expires_at -> Nullable<Timestamp>,
        linked_at -> Timestamp,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    user_sessions (id) {
        id -> Uuid,
        user_id -> Uuid,
        expires_at -> Timestamp,
        revoked_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    users (id) {
        id -> Uuid,
        username -> Varchar,
        password_hash -> Varchar,
        first_name -> Nullable<Varchar>,
        last_name -> Nullable<Varchar>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        two_factor_enabled -> Bool,
        two_factor_secret_encrypted -> Nullable<Text>,
        two_factor_confirmed_at -> Nullable<Timestamp>,
        two_factor_admin_required -> Bool,
        is_admin -> Bool,
        email -> Varchar,
    }
}

diesel::table! {
    walls (wall_id) {
        wall_id -> Uuid,
        scene_id -> Uuid,
        x1 -> Float8,
        y1 -> Float8,
        x2 -> Float8,
        y2 -> Float8,
        blocks_vision -> Bool,
        blocks_movement -> Bool,
        metadata -> Nullable<Jsonb>,
        created_by -> Uuid,
        updated_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        door_state -> Text,
        locked -> Bool,
        secret -> Bool,
    }
}

diesel::table! {
    world_abilities (id) {
        id -> Uuid,
        world_id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        #[max_length = 16]
        classification -> Varchar,
        gm_only -> Bool,
        created_by -> Uuid,
        updated_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        grade -> Nullable<Int4>,
    }
}

diesel::table! {
    world_ability_effects (id) {
        id -> Uuid,
        ability_id -> Uuid,
        #[max_length = 16]
        effect_type -> Varchar,
        formula -> Text,
        target -> Text,
        #[max_length = 16]
        trigger_kind -> Nullable<Varchar>,
        sort_order -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_ability_permissions (id) {
        id -> Uuid,
        ability_id -> Uuid,
        user_id -> Uuid,
        #[max_length = 16]
        level -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_ability_shares (id) {
        id -> Uuid,
        ability_id -> Uuid,
        #[max_length = 32]
        share_code -> Varchar,
        created_by -> Uuid,
        revoked -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_actor_abilities (id) {
        id -> Uuid,
        actor_id -> Uuid,
        ability_id -> Nullable<Uuid>,
        ability_name_snapshot -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_actor_claims (id) {
        id -> Uuid,
        actor_id -> Uuid,
        world_member_id -> Uuid,
        claimed_at -> Timestamptz,
    }
}

diesel::table! {
    world_actor_images (id) {
        id -> Uuid,
        actor_id -> Uuid,
        role -> Varchar,
        asset_id -> Uuid,
        created_by -> Uuid,
        updated_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_actor_inventory (id) {
        id -> Uuid,
        actor_id -> Uuid,
        item_id -> Nullable<Uuid>,
        item_name_snapshot -> Text,
        quantity -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        created_by -> Nullable<Uuid>,
        updated_by -> Nullable<Uuid>,
    }
}

diesel::table! {
    world_actor_permissions (id) {
        id -> Uuid,
        actor_id -> Uuid,
        user_id -> Uuid,
        #[max_length = 16]
        level -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_actor_shares (id) {
        id -> Uuid,
        actor_id -> Uuid,
        #[max_length = 32]
        share_code -> Varchar,
        created_by -> Uuid,
        revoked -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_actor_system_data (id) {
        id -> Uuid,
        actor_id -> Uuid,
        game_system_id -> Varchar,
        ability_data -> Nullable<Jsonb>,
        resource_data -> Nullable<Jsonb>,
        proficiency_data -> Nullable<Jsonb>,
        trait_data -> Nullable<Jsonb>,
        spell_data -> Nullable<Jsonb>,
        created_by -> Uuid,
        updated_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_actors (id) {
        id -> Uuid,
        world_id -> Uuid,
        scene_id -> Uuid,
        actor_type -> Varchar,
        game_system_id -> Nullable<Varchar>,
        label -> Text,
        created_by -> Uuid,
        owned_by -> Uuid,
        is_public -> Bool,
        is_npc -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        description -> Nullable<Text>,
        available_for_claim -> Bool,
    }
}

diesel::table! {
    world_authoring_tool_grants (id) {
        id -> Uuid,
        world_member_id -> Uuid,
        tool -> Varchar,
        created_by -> Uuid,
        updated_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_chat_messages (id) {
        id -> Uuid,
        world_id -> Uuid,
        scene_id -> Nullable<Uuid>,
        author_user_id -> Uuid,
        author_label -> Text,
        body -> Text,
        gm_only -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_combatants (id) {
        id -> Uuid,
        combat_id -> Uuid,
        actor_id -> Nullable<Uuid>,
        token_id -> Nullable<Uuid>,
        label -> Text,
        initiative -> Int4,
        tiebreak -> Int4,
        is_npc -> Bool,
        active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_combats (id) {
        id -> Uuid,
        world_id -> Uuid,
        scene_id -> Nullable<Uuid>,
        round -> Int4,
        active_combatant_id -> Nullable<Uuid>,
        ended_at -> Nullable<Timestamp>,
        created_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_events (id) {
        id -> Int8,
        world_id -> Uuid,
        event_code -> Int4,
        token_event -> Nullable<Jsonb>,
        created_at -> Timestamp,
        schema_version -> Int4,
        updated_at -> Timestamp,
        created_by -> Uuid,
        updated_by -> Uuid,
    }
}

diesel::table! {
    world_invites (id) {
        id -> Uuid,
        world_id -> Uuid,
        #[max_length = 32]
        invite_code -> Varchar,
        max_uses -> Int4,
        used_count -> Int4,
        expires_at -> Nullable<Timestamp>,
        created_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        revoked -> Bool,
        rotated_from -> Nullable<Uuid>,
    }
}

diesel::table! {
    world_item_abilities (id) {
        id -> Uuid,
        item_id -> Uuid,
        ability_id -> Nullable<Uuid>,
        ability_name_snapshot -> Text,
        created_by -> Uuid,
        updated_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_item_effects (id) {
        id -> Uuid,
        item_id -> Uuid,
        #[max_length = 16]
        effect_type -> Varchar,
        formula -> Text,
        target -> Text,
        #[max_length = 16]
        trigger_kind -> Nullable<Varchar>,
        sort_order -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_item_permissions (id) {
        id -> Uuid,
        item_id -> Uuid,
        user_id -> Uuid,
        #[max_length = 16]
        level -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_item_prices (id) {
        id -> Uuid,
        item_id -> Uuid,
        amount -> Int4,
        currency_label -> Nullable<Text>,
        is_suggested -> Bool,
        created_by -> Uuid,
        updated_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_item_shares (id) {
        id -> Uuid,
        item_id -> Uuid,
        #[max_length = 32]
        share_code -> Varchar,
        created_by -> Uuid,
        revoked -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_items (id) {
        id -> Uuid,
        world_id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        icon_asset_id -> Nullable<Uuid>,
        created_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_lore_entries (id) {
        id -> Uuid,
        world_id -> Uuid,
        title -> Text,
        slug -> Text,
        content -> Text,
        current_revision_id -> Nullable<Uuid>,
        created_by -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        parent_id -> Nullable<Uuid>,
    }
}

diesel::table! {
    world_lore_image_assets (id) {
        id -> Uuid,
        lore_entry_id -> Uuid,
        uploaded_by -> Uuid,
        original_filename -> Nullable<Text>,
        content_type -> Text,
        byte_size -> Int8,
        created_at -> Timestamp,
    }
}

diesel::table! {
    world_lore_links (id) {
        id -> Uuid,
        source_lore_entry_id -> Uuid,
        raw_title -> Text,
        #[max_length = 16]
        target_kind -> Varchar,
        target_lore_entry_id -> Nullable<Uuid>,
        target_actor_id -> Nullable<Uuid>,
        created_at -> Timestamp,
        target_item_id -> Nullable<Uuid>,
        target_ability_id -> Nullable<Uuid>,
    }
}

diesel::table! {
    world_lore_permissions (id) {
        id -> Uuid,
        lore_entry_id -> Uuid,
        world_member_user_id -> Uuid,
        #[max_length = 16]
        level -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_lore_revisions (id) {
        id -> Uuid,
        lore_entry_id -> Uuid,
        content_markdown -> Text,
        author_id -> Uuid,
        restored_from_revision_id -> Nullable<Uuid>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    world_lore_tags (id) {
        id -> Uuid,
        lore_entry_id -> Uuid,
        tag -> Text,
        created_by -> Uuid,
        created_at -> Timestamp,
    }
}

diesel::table! {
    world_members (id) {
        id -> Uuid,
        world_id -> Uuid,
        user_id -> Uuid,
        #[max_length = 32]
        role -> Varchar,
        joined_at -> Timestamp,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    world_roll_records (id) {
        id -> Uuid,
        world_id -> Uuid,
        triggered_by -> Uuid,
        formula -> Text,
        bindings -> Nullable<Jsonb>,
        detail -> Jsonb,
        result_kind -> Text,
        result_value -> Float8,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    world_tokens (id) {
        id -> Text,
        world_id -> Uuid,
        x -> Float8,
        y -> Float8,
        z -> Float8,
        label -> Nullable<Text>,
        health -> Nullable<Int4>,
        max_health -> Nullable<Int4>,
        schema_version -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        created_by -> Uuid,
        updated_by -> Uuid,
    }
}

diesel::table! {
    worlds (id) {
        id -> Uuid,
        name -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        created_by -> Uuid,
        updated_by -> Uuid,
        description -> Nullable<Text>,
        game_system_id -> Nullable<Varchar>,
        interface_pack_id -> Nullable<Varchar>,
        session_notes -> Nullable<Text>,
        allow_player_created_actors -> Bool,
        genie_resource_carryover_enabled -> Bool,
        default_scene_grid_type -> Text,
        active_scene_id -> Nullable<Uuid>,
    }
}

diesel::joinable!(admin_bootstrap_oauth_sessions -> oauth_providers (provider_id));
diesel::joinable!(canvas_image_assets -> worlds (world_id));
diesel::joinable!(fog_masks -> scenes (scene_id));
diesel::joinable!(fog_masks -> users (updated_by));
diesel::joinable!(interaction_requests -> interactives (interactive_id));
diesel::joinable!(interaction_requests -> scenes (scene_id));
diesel::joinable!(interactives -> scenes (scene_id));
diesel::joinable!(light_sources -> scenes (scene_id));
diesel::joinable!(light_sources -> tokens (attached_token_id));
diesel::joinable!(login_two_factor_challenges -> users (user_id));
diesel::joinable!(oauth_authorization_sessions -> oauth_providers (provider_id));
diesel::joinable!(oauth_link_challenges -> oauth_providers (provider_id));
diesel::joinable!(oauth_link_challenges -> users (user_id));
diesel::joinable!(players_online -> scenes (scene_id));
diesel::joinable!(players_online -> users (player_id));
diesel::joinable!(players_online -> worlds (world_id));
diesel::joinable!(policies -> worlds (world_id));
diesel::joinable!(scene_state_fingerprints -> scenes (scene_id));
diesel::joinable!(scene_state_fingerprints -> users (updated_by));
diesel::joinable!(scenes -> users (owner_id));
diesel::joinable!(shapes -> scenes (scene_id));
diesel::joinable!(token_resource_disclosure -> tokens (token_id));
diesel::joinable!(tokens -> scenes (scene_id));
diesel::joinable!(tokens -> users (owner_user_id));
diesel::joinable!(user_oauth_accounts -> oauth_providers (provider_id));
diesel::joinable!(user_oauth_accounts -> users (user_id));
diesel::joinable!(user_sessions -> users (user_id));
diesel::joinable!(walls -> scenes (scene_id));
diesel::joinable!(world_abilities -> worlds (world_id));
diesel::joinable!(world_ability_effects -> world_abilities (ability_id));
diesel::joinable!(world_ability_permissions -> users (user_id));
diesel::joinable!(world_ability_permissions -> world_abilities (ability_id));
diesel::joinable!(world_ability_shares -> users (created_by));
diesel::joinable!(world_ability_shares -> world_abilities (ability_id));
diesel::joinable!(world_actor_abilities -> world_abilities (ability_id));
diesel::joinable!(world_actor_abilities -> world_actors (actor_id));
diesel::joinable!(world_actor_claims -> world_actors (actor_id));
diesel::joinable!(world_actor_claims -> world_members (world_member_id));
diesel::joinable!(world_actor_images -> world_actors (actor_id));
diesel::joinable!(world_actor_inventory -> world_actors (actor_id));
diesel::joinable!(world_actor_inventory -> world_items (item_id));
diesel::joinable!(world_actor_permissions -> users (user_id));
diesel::joinable!(world_actor_permissions -> world_actors (actor_id));
diesel::joinable!(world_actor_shares -> users (created_by));
diesel::joinable!(world_actor_shares -> world_actors (actor_id));
diesel::joinable!(world_actor_system_data -> world_actors (actor_id));
diesel::joinable!(world_actors -> scenes (scene_id));
diesel::joinable!(world_actors -> worlds (world_id));
diesel::joinable!(world_authoring_tool_grants -> world_members (world_member_id));
diesel::joinable!(world_chat_messages -> scenes (scene_id));
diesel::joinable!(world_chat_messages -> users (author_user_id));
diesel::joinable!(world_chat_messages -> worlds (world_id));
diesel::joinable!(world_combatants -> world_actors (actor_id));
diesel::joinable!(world_combats -> scenes (scene_id));
diesel::joinable!(world_combats -> users (created_by));
diesel::joinable!(world_combats -> worlds (world_id));
diesel::joinable!(world_events -> worlds (world_id));
diesel::joinable!(world_invites -> users (created_by));
diesel::joinable!(world_invites -> worlds (world_id));
diesel::joinable!(world_item_abilities -> world_abilities (ability_id));
diesel::joinable!(world_item_abilities -> world_items (item_id));
diesel::joinable!(world_item_effects -> world_items (item_id));
diesel::joinable!(world_item_permissions -> users (user_id));
diesel::joinable!(world_item_permissions -> world_items (item_id));
diesel::joinable!(world_item_prices -> world_items (item_id));
diesel::joinable!(world_item_shares -> users (created_by));
diesel::joinable!(world_item_shares -> world_items (item_id));
diesel::joinable!(world_items -> users (created_by));
diesel::joinable!(world_items -> worlds (world_id));
diesel::joinable!(world_lore_entries -> users (created_by));
diesel::joinable!(world_lore_entries -> worlds (world_id));
diesel::joinable!(world_lore_image_assets -> users (uploaded_by));
diesel::joinable!(world_lore_image_assets -> world_lore_entries (lore_entry_id));
diesel::joinable!(world_lore_links -> world_abilities (target_ability_id));
diesel::joinable!(world_lore_links -> world_actors (target_actor_id));
diesel::joinable!(world_lore_links -> world_items (target_item_id));
diesel::joinable!(world_lore_permissions -> users (world_member_user_id));
diesel::joinable!(world_lore_permissions -> world_lore_entries (lore_entry_id));
diesel::joinable!(world_lore_revisions -> users (author_id));
diesel::joinable!(world_lore_tags -> users (created_by));
diesel::joinable!(world_lore_tags -> world_lore_entries (lore_entry_id));
diesel::joinable!(world_members -> users (user_id));
diesel::joinable!(world_members -> worlds (world_id));
diesel::joinable!(world_roll_records -> users (triggered_by));
diesel::joinable!(world_roll_records -> worlds (world_id));
diesel::joinable!(world_tokens -> worlds (world_id));

diesel::allow_tables_to_appear_in_same_query!(
    admin_bootstrap_oauth_sessions,
    admin_bootstrap_setup,
    auth_security_settings,
    canvas_image_assets,
    content_moderation_actions,
    fog_masks,
    game_systems,
    interaction_requests,
    interactives,
    light_sources,
    login_two_factor_challenges,
    oauth_authorization_sessions,
    oauth_link_challenges,
    oauth_providers,
    players_online,
    policies,
    scene_preview_images,
    scene_state_fingerprints,
    scenes,
    shapes,
    token_resource_disclosure,
    tokens,
    user_oauth_accounts,
    user_sessions,
    users,
    walls,
    world_abilities,
    world_ability_effects,
    world_ability_permissions,
    world_ability_shares,
    world_actor_abilities,
    world_actor_claims,
    world_actor_images,
    world_actor_inventory,
    world_actor_permissions,
    world_actor_shares,
    world_actor_system_data,
    world_actors,
    world_authoring_tool_grants,
    world_chat_messages,
    world_combatants,
    world_combats,
    world_events,
    world_invites,
    world_item_abilities,
    world_item_effects,
    world_item_permissions,
    world_item_prices,
    world_item_shares,
    world_items,
    world_lore_entries,
    world_lore_image_assets,
    world_lore_links,
    world_lore_permissions,
    world_lore_revisions,
    world_lore_tags,
    world_members,
    world_roll_records,
    world_tokens,
    worlds,
);
