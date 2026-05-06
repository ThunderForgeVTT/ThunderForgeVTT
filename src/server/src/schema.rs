// @generated automatically by Diesel CLI.

pub mod sql_types {
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
        email -> Varchar,
        two_factor_enabled -> Bool,
        two_factor_secret_encrypted -> Nullable<Text>,
        two_factor_confirmed_at -> Nullable<Timestamp>,
        two_factor_admin_required -> Bool,
        is_admin -> Bool,
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

diesel::joinable!(admin_bootstrap_oauth_sessions -> oauth_providers (provider_id));
diesel::joinable!(fog_masks -> scenes (scene_id));
diesel::joinable!(fog_masks -> users (updated_by));
diesel::joinable!(login_two_factor_challenges -> users (user_id));
diesel::joinable!(oauth_authorization_sessions -> oauth_providers (provider_id));
diesel::joinable!(oauth_link_challenges -> oauth_providers (provider_id));
diesel::joinable!(oauth_link_challenges -> users (user_id));
diesel::joinable!(policies -> worlds (world_id));
diesel::joinable!(scenes -> users (owner_id));
diesel::joinable!(scenes -> worlds (world_id));
diesel::joinable!(tokens -> scenes (scene_id));
diesel::joinable!(user_oauth_accounts -> oauth_providers (provider_id));
diesel::joinable!(user_oauth_accounts -> users (user_id));
diesel::joinable!(user_sessions -> users (user_id));
diesel::joinable!(world_actors -> scenes (scene_id));
diesel::joinable!(world_actors -> worlds (world_id));
// Note: world_actors has two foreign keys to users (created_by, owned_by)
// Diesel doesn't support multiple joinables for the same table pair,
// so joins with users must be written manually

diesel::joinable!(world_actor_system_data -> world_actors (actor_id));
// Note: world_actor_system_data has two foreign keys to users (created_by, updated_by)
// Diesel doesn't support multiple joinables for the same table pair,
// so joins with users must be written manually
diesel::joinable!(world_events -> worlds (world_id));
diesel::joinable!(world_tokens -> worlds (world_id));
diesel::joinable!(players_online -> users (player_id));
diesel::joinable!(players_online -> worlds (world_id));
diesel::joinable!(players_online -> scenes (scene_id));

diesel::allow_tables_to_appear_in_same_query!(
    admin_bootstrap_oauth_sessions,
    admin_bootstrap_setup,
    auth_security_settings,
    fog_masks,
    game_systems,
    login_two_factor_challenges,
    oauth_authorization_sessions,
    oauth_link_challenges,
    oauth_providers,
    policies,
    players_online,
    scenes,
    tokens,
    user_oauth_accounts,
    user_sessions,
    users,
    world_actors,
    world_actor_system_data,
    world_events,
    world_tokens,
    worlds,
);
