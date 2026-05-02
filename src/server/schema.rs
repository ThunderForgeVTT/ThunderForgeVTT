// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "PolicyEffect"))]
    pub struct PolicyEffect;
}

diesel::table! {
    auth_security_settings (id) {
        id -> Int4,
        two_factor_required_for_all_users -> Bool,
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
    }
}

diesel::table! {
    world_events (id) {
        id -> Int8,
        world_id -> Uuid,
        event_code -> Int4,
        token_event -> Nullable<Jsonb>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    worlds (id) {
        id -> Uuid,
        name -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::joinable!(login_two_factor_challenges -> users (user_id));
diesel::joinable!(oauth_authorization_sessions -> oauth_providers (provider_id));
diesel::joinable!(oauth_link_challenges -> oauth_providers (provider_id));
diesel::joinable!(oauth_link_challenges -> users (user_id));
diesel::joinable!(user_oauth_accounts -> oauth_providers (provider_id));
diesel::joinable!(user_oauth_accounts -> users (user_id));
diesel::joinable!(user_sessions -> users (user_id));
diesel::joinable!(world_events -> worlds (world_id));

diesel::allow_tables_to_appear_in_same_query!(
    auth_security_settings,
    login_two_factor_challenges,
    oauth_authorization_sessions,
    oauth_link_challenges,
    oauth_providers,
    policies,
    user_oauth_accounts,
    user_sessions,
    users,
    world_events,
    worlds,
);
