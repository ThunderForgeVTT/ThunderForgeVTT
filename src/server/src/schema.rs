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
    }
}

diesel::joinable!(admin_bootstrap_oauth_sessions -> oauth_providers (provider_id));
diesel::joinable!(canvas_image_assets -> worlds (world_id));
diesel::joinable!(fog_masks -> scenes (scene_id));
diesel::joinable!(fog_masks -> users (updated_by));
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
diesel::joinable!(scenes -> users (owner_id));
diesel::joinable!(scenes -> worlds (world_id));
diesel::joinable!(shapes -> scenes (scene_id));
diesel::joinable!(tokens -> scenes (scene_id));
diesel::joinable!(tokens -> users (owner_user_id));
diesel::joinable!(user_oauth_accounts -> oauth_providers (provider_id));
diesel::joinable!(user_oauth_accounts -> users (user_id));
diesel::joinable!(user_sessions -> users (user_id));
diesel::joinable!(walls -> scenes (scene_id));
diesel::joinable!(world_actor_permissions -> users (user_id));
diesel::joinable!(world_actor_permissions -> world_actors (actor_id));
diesel::joinable!(world_actor_shares -> users (created_by));
diesel::joinable!(world_actor_shares -> world_actors (actor_id));
diesel::joinable!(world_actor_system_data -> world_actors (actor_id));
diesel::joinable!(world_actors -> scenes (scene_id));
diesel::joinable!(world_actors -> worlds (world_id));
diesel::joinable!(world_events -> worlds (world_id));
diesel::joinable!(world_invites -> users (created_by));
diesel::joinable!(world_invites -> worlds (world_id));
diesel::joinable!(world_members -> users (user_id));
diesel::joinable!(world_members -> worlds (world_id));
diesel::joinable!(world_tokens -> worlds (world_id));

diesel::allow_tables_to_appear_in_same_query!(
    admin_bootstrap_oauth_sessions,
    admin_bootstrap_setup,
    auth_security_settings,
    canvas_image_assets,
    fog_masks,
    game_systems,
    light_sources,
    login_two_factor_challenges,
    oauth_authorization_sessions,
    oauth_link_challenges,
    oauth_providers,
    players_online,
    policies,
    scenes,
    shapes,
    tokens,
    user_oauth_accounts,
    user_sessions,
    users,
    walls,
    world_actor_permissions,
    world_actor_shares,
    world_actor_system_data,
    world_actors,
    world_events,
    world_invites,
    world_members,
    world_tokens,
    worlds,
);
