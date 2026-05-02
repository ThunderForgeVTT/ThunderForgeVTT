// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "PolicyEffect"))]
    pub struct PolicyEffect;
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
    users (id) {
        id -> Uuid,
        username -> Varchar,
        password -> Varchar,
        first_name -> Nullable<Varchar>,
        last_name -> Nullable<Varchar>,
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

diesel::joinable!(world_events -> worlds (world_id));

diesel::allow_tables_to_appear_in_same_query!(policies, users, world_events, worlds,);
