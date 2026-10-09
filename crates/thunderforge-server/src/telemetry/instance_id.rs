//! The instance id (FR-007, R23): one UUIDv4, minted on the first start and
//! never changed, in `instance_settings` under the reserved `system.` prefix.
//!
//! It is written with `INSERT ... ON CONFLICT (key) DO NOTHING` and then read,
//! so two replicas starting together both read the one that won. It is
//! created whatever `TELEMETRY` says, so turning telemetry on later does not
//! make a new instance.

use crate::schema::instance_settings;
use diesel::prelude::*;

/// Under `settings::registry::RESERVED_PREFIX`: not a setting, not editable,
/// not reported as unrecognised.
pub const INSTANCE_ID_KEY: &str = "system.telemetry_instance_id";

/// The stored id, creating it if there is none. A stored value that is not a
/// UUID is returned as it is, with a warning: only deleting the row replaces it.
pub fn ensure_instance_id(conn: &mut PgConnection) -> QueryResult<String> {
    let minted = uuid::Uuid::new_v4().to_string();
    diesel::insert_into(instance_settings::table)
        .values((
            instance_settings::key.eq(INSTANCE_ID_KEY),
            instance_settings::value.eq(&minted),
        ))
        .on_conflict(instance_settings::key)
        .do_nothing()
        .execute(conn)?;
    let value: String = instance_settings::table
        .filter(instance_settings::key.eq(INSTANCE_ID_KEY))
        .select(instance_settings::value)
        .first(conn)?;
    if uuid::Uuid::parse_str(&value).is_err() {
        tracing::warn!(
            key = INSTANCE_ID_KEY,
            "the stored telemetry instance id is not a UUID; delete the row to mint a new one"
        );
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::test_env;
    use crate::test_support::test_app_state;

    fn clear(conn: &mut PgConnection) {
        diesel::delete(instance_settings::table.filter(instance_settings::key.eq(INSTANCE_ID_KEY)))
            .execute(conn)
            .expect("clear the instance id row");
    }

    #[test]
    fn created_once_as_a_uuid_v4_and_then_kept() {
        let _lock = test_env::lock();
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("conn");
        clear(&mut conn);

        let first = ensure_instance_id(&mut conn).expect("first");
        let parsed = uuid::Uuid::parse_str(&first).expect("a UUID");
        assert_eq!(parsed.get_version_num(), 4);
        assert_eq!(first, parsed.hyphenated().to_string());

        let second = ensure_instance_id(&mut conn).expect("second");
        assert_eq!(first, second);
    }

    #[test]
    fn a_value_already_present_is_not_touched() {
        let _lock = test_env::lock();
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("conn");
        clear(&mut conn);
        diesel::insert_into(instance_settings::table)
            .values((
                instance_settings::key.eq(INSTANCE_ID_KEY),
                instance_settings::value.eq("not-a-uuid"),
            ))
            .execute(&mut conn)
            .expect("seed");

        assert_eq!(ensure_instance_id(&mut conn).expect("read"), "not-a-uuid");
        clear(&mut conn);
    }

    #[test]
    fn two_concurrent_calls_return_one_value() {
        let _lock = test_env::lock();
        let state = test_app_state();
        clear(&mut state.db_pool.get().expect("conn"));

        let handles: Vec<_> = (0..2)
            .map(|_| {
                let pool = state.db_pool.clone();
                std::thread::spawn(move || {
                    ensure_instance_id(&mut pool.get().expect("conn")).expect("ensure")
                })
            })
            .collect();
        let values: Vec<String> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(values[0], values[1]);
    }

    #[tokio::test]
    async fn the_row_is_not_reported_as_unrecognised() {
        let _lock = test_env::lock();
        let state = test_app_state();
        ensure_instance_id(&mut state.db_pool.get().expect("conn")).expect("ensure");
        let settings = crate::settings::resolve_all(&state).await.expect("resolve");
        assert!(
            !settings.unrecognised().iter().any(|k| k == INSTANCE_ID_KEY),
            "{:?}",
            settings.unrecognised()
        );
    }
}
