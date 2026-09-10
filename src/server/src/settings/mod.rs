//! Spec 040 / ADR-088, ADR-091: what this instance is configured to be, and
//! the one rule that decides where each value came from.
//!
//! # The shape of it
//!
//! - [`registry`] — the declaration list. A setting exists because a
//!   declaration exists. Nothing else creates one.
//! - [`resolver`] — **the environment beats the instance's own store beats the
//!   declared default**, for every declared setting, resolved per request so a
//!   change takes effect with no restart.
//! - [`validate`] — FR-004's refusal rules: blank, unreachable domains, and
//!   the placeholders this product itself ships.
//! - [`changes`] — the write path and the append-only record of having
//!   written. For a secret the record holds the transition, never the value.
//! - [`graphql`] — the administrator's surface over all of it.
//!
//! Readiness — what this instance cannot do, and what to set — is derived from
//! the same declaration list and lives in [`crate::readiness`].
//!
//! # Why five settings mechanisms became one
//!
//! Before this, an instance's configuration was a JSON file on disk (the realm
//! manifest), a singleton table (the access policy), rows with a
//! `config_source` column (OAuth providers), the environment read per use
//! (lore sync's application), and `Config::from_env()` read once at startup.
//! Three storage media, three read cadences, two precedence mechanisms, and
//! nowhere an operator could see the whole of it.
//!
//! This does not migrate any of them. The manifest is still a file, the access
//! policy is still spec 035's table and its own audit trail, and OAuth keeps
//! ADR-041's startup materialisation. What is new is that all of them are
//! *declared*, resolve by one rule, and report a source in one vocabulary —
//! and that everything this feature adds is a row in `instance_settings`
//! rather than a sixth mechanism.

pub mod changes;
pub mod graphql;
pub mod registry;
pub mod resolver;
pub mod validate;

/// The migration's own test: `up` → `down` → `up` leaves the schema clean.
/// Kept beside the module whose migration it is, rather than in a test-only
/// directory nobody reads when they edit the SQL.
#[cfg(test)]
#[path = "settings_migration_tests.rs"]
mod settings_migration_tests;

pub use registry::{Backing, Capability, Kind, Requirement, SettingDeclaration, declarations};
pub use resolver::{Resolved, Settings, Source, resolve, resolve_all};

/// One lock, for every test in this feature that touches process-global state.
///
/// The environment is process-global and `cargo test` is threaded, and so is
/// the settings row-set: a test that sets `THUNDERFORGE_OPERATOR_NAME` and a
/// test that writes the `operator.name` row are in each other's way, in both
/// directions. Serialising them behind one lock is the difference between a
/// suite that passes alone and fails together and one that means something.
///
/// `repo_host_tests.rs` keeps a lock of its own for the first half of this
/// reason; this is that helper, shared across the four modules that need it.
#[cfg(test)]
pub(crate) mod test_env {
    use std::sync::{Mutex, MutexGuard};

    static LOCK: Mutex<()> = Mutex::new(());

    /// Hold this for the length of a test that writes settings rows.
    pub fn lock() -> MutexGuard<'static, ()> {
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Take some settings **rows** away for the length of a test, and put them
    /// back afterwards.
    ///
    /// # Why this is needed as well as `temp_env`
    ///
    /// Because a declaration resolves from the environment *or* from a row, and
    /// clearing only the environment leaves whatever is in the table. That is
    /// exactly what happened when spec 039's T005 seeded a notice contact into
    /// the shared development database: three spec-040 tests that assert "an
    /// instance with no notice contact refuses to publish" could no longer
    /// create the condition they were about, and failed with the gate wide open.
    ///
    /// The lesson is the one `instance_access`'s `PolicyGuard` learned the same
    /// evening: **a test that depends on global state has to establish it**, not
    /// assume the database was empty. Nothing else in the tree may assume the
    /// absence of a row.
    ///
    /// The caller must already hold [`lock`] — this takes no lock of its own,
    /// because its callers are async and hold theirs across an `.await`.
    pub struct AbsentRows {
        pool: crate::state::DbPool,
        previous: Vec<(String, String)>,
    }

    /// Remove the rows for `keys`, remembering what they held.
    pub fn without_rows(state: &crate::state::AppState, keys: &[&str]) -> AbsentRows {
        use crate::schema::instance_settings;
        use diesel::prelude::*;

        let owned: Vec<String> = keys.iter().map(|k| (*k).to_string()).collect();
        let mut conn = state
            .db_pool
            .get()
            .expect("a connection to clear the settings rows");

        let previous: Vec<(String, String)> = instance_settings::table
            .filter(instance_settings::key.eq_any(&owned))
            .select((instance_settings::key, instance_settings::value))
            .load(&mut conn)
            .expect("read the settings rows");

        diesel::delete(instance_settings::table.filter(instance_settings::key.eq_any(&owned)))
            .execute(&mut conn)
            .expect("clear the settings rows");

        AbsentRows {
            pool: state.db_pool.clone(),
            previous,
        }
    }

    impl Drop for AbsentRows {
        fn drop(&mut self) {
            use crate::schema::instance_settings;
            use diesel::prelude::*;

            // Best-effort: a test that has already failed must not be reported
            // as a panic in its own cleanup.
            let Ok(mut conn) = self.pool.get() else {
                return;
            };
            for (key, value) in &self.previous {
                let _ = diesel::insert_into(instance_settings::table)
                    .values((
                        instance_settings::key.eq(key),
                        instance_settings::value.eq(value),
                    ))
                    .on_conflict(instance_settings::key)
                    .do_update()
                    .set(instance_settings::value.eq(value))
                    .execute(&mut conn);
            }
        }
    }

    /// Set some variables, run, restore — with the same lock held.
    pub fn temp_env(vars: &[(&str, Option<&str>)], body: impl FnOnce()) {
        let _guard = lock();

        let previous: Vec<(String, Option<String>)> = vars
            .iter()
            .map(|(k, _)| ((*k).to_string(), std::env::var(k).ok()))
            .collect();
        for (k, v) in vars {
            // SAFETY: serialised by the lock above; nothing in this binary
            // reads these variables outside a helper that takes it.
            unsafe {
                match v {
                    Some(value) => std::env::set_var(k, value),
                    None => std::env::remove_var(k),
                }
            }
        }

        body();

        for (k, v) in previous {
            unsafe {
                match v {
                    Some(value) => std::env::set_var(&k, value),
                    None => std::env::remove_var(&k),
                }
            }
        }
    }
}
