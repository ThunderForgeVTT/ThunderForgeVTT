//! Writing a setting, and the append-only record of having written it.
//!
//! # Why the record stores a transition and not always a value
//!
//! FR-008 wants "what it was before". FR-023 forbids ever printing a
//! credential. For a secret those are in direct conflict, and the resolution
//! is that the record captures the **transition**: the literal previous
//! address for a notice contact, and the words `set` / `not set` for an SMTP
//! password. An operator investigating a stale notice address needs the old
//! address; nobody needs the old password, and storing it doubles the number
//! of places it can leak from.
//!
//! `redacted` is stored rather than recomputed at read time, so a row stays
//! truthful if a declaration's `secret` flag is ever changed. A row that says
//! it was redacted was redacted when it was written.

use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use super::registry::{Backing, SettingDeclaration, declaration};
use super::resolver::{Resolved, Settings, encrypt, resolve_all};
use super::validate::validate;
use crate::models::{InstanceSettingChange, NewInstanceSettingChange};
use crate::schema::{instance_setting_changes, instance_settings};
use crate::state::AppState;

/// Which surface a change came through. The `CHECK` constraint in the
/// migration holds the same three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeSource {
    Setup,
    Admin,
    System,
}

impl ChangeSource {
    pub fn as_db_str(self) -> &'static str {
        match self {
            ChangeSource::Setup => "setup",
            ChangeSource::Admin => "admin",
            ChangeSource::System => "system",
        }
    }
}

/// The two words a secret is ever rendered as, anywhere in this product.
const SET: &str = "set";
const NOT_SET: &str = "not set";

/// Write one setting, and record having written it.
///
/// `value: None` clears a stored value; the setting then resolves from its
/// default, and the record shows that transition. There is deliberately no
/// delete that leaves no trace — FR-008 requires the trail, so clearing leaves
/// one too.
///
/// Refused, by name and never silently, when:
///
/// - the key is not declared — an undeclared key has no validators, no
///   redaction rule and no capability, so writing one would be writing a
///   setting that opted out of all three;
/// - the environment has fixed the value (FR-009). ADR-041 records what the
///   silent no-op cost the OAuth surface: an administration screen that
///   rendered environment-sourced rows as editable, and edits that quietly did
///   not persist;
/// - the value fails the declaration's validators (FR-004). The refusal says
///   what would be right and never repeats what was submitted.
pub async fn write_setting(
    state: &AppState,
    key: &str,
    value: Option<&str>,
    actor: Option<Uuid>,
    source: ChangeSource,
) -> Result<Resolved, String> {
    let d = declaration(key)
        .ok_or_else(|| format!("`{key}` is not a setting this instance declares."))?;

    let before = resolve_all(state).await?;
    let current = before.get(d.key).cloned();

    if let Some(name) = d.env_name_in_use() {
        return Err(format!(
            "`{}` is fixed by the environment variable `{name}` and cannot be changed here. \
             Unset that variable to make the setting editable again.",
            d.key
        ));
    }

    if matches!(d.backing, Backing::AccessPolicy) {
        return Err(format!(
            "`{}` is changed with `setInstanceAccessPolicy`, which records the change in the \
             instance's access log. Writing it here would leave that log incomplete.",
            d.key
        ));
    }

    let accepted = match value {
        Some(raw) => Some(validate(d, raw)?),
        None => None,
    };

    let previous = current.as_ref().and_then(|r| r.value.clone());
    let (previous_recorded, new_recorded) =
        recorded_values(d, previous.as_deref(), accepted.as_deref());

    match d.backing {
        Backing::Row => {
            let to_store = match (&accepted, d.secret) {
                (Some(plain), true) => Some(encrypt(state, plain)?),
                (Some(plain), false) => Some(plain.clone()),
                (None, _) => None,
            };
            persist_row(
                state,
                d.key,
                to_store,
                actor,
                NewInstanceSettingChange {
                    id: Uuid::now_v7(),
                    key: d.key.to_string(),
                    previous_value: previous_recorded,
                    new_value: new_recorded,
                    redacted: d.secret,
                    changed_by: actor,
                    changed_at: Utc::now().naive_utc(),
                    source: source.as_db_str().to_string(),
                },
            )
            .await?;
        }
        Backing::ManifestFile(manifest_key) => {
            // The manifest is a file and the record is a row, so this is two
            // writes and not one transaction. The order is deliberate: the
            // value first, the record second. A record of a change that did
            // not happen is worse than a change with no record, because the
            // first is read as fact and the second is visible as a gap.
            crate::admin::update_manifest_key(
                state,
                manifest_key,
                accepted.as_deref().unwrap_or(""),
            )?;
            append_record(
                state,
                NewInstanceSettingChange {
                    id: Uuid::now_v7(),
                    key: d.key.to_string(),
                    previous_value: previous_recorded,
                    new_value: new_recorded,
                    redacted: d.secret,
                    changed_by: actor,
                    changed_at: Utc::now().naive_utc(),
                    source: source.as_db_str().to_string(),
                },
            )
            .await?;
        }
        Backing::AccessPolicy => unreachable!("refused above"),
    }

    let after = resolve_all(state).await?;
    after
        .get(d.key)
        .cloned()
        .ok_or_else(|| format!("`{}` did not resolve after being written.", d.key))
}

/// What the record says about a change, redacted per the declaration.
fn recorded_values(
    d: &SettingDeclaration,
    previous: Option<&str>,
    new: Option<&str>,
) -> (Option<String>, Option<String>) {
    if d.secret {
        return (
            Some(presence(previous).to_string()),
            Some(presence(new).to_string()),
        );
    }
    (previous.map(str::to_string), new.map(str::to_string))
}

fn presence(value: Option<&str>) -> &'static str {
    match value {
        Some(v) if !v.trim().is_empty() => SET,
        _ => NOT_SET,
    }
}

async fn persist_row(
    state: &AppState,
    key: &'static str,
    to_store: Option<String>,
    actor: Option<Uuid>,
    record: NewInstanceSettingChange,
) -> Result<(), String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let now = Utc::now().naive_utc();
            match to_store {
                Some(value) => {
                    diesel::insert_into(instance_settings::table)
                        .values((
                            instance_settings::key.eq(key),
                            instance_settings::value.eq(&value),
                            instance_settings::updated_by.eq(actor),
                            instance_settings::updated_at.eq(now),
                            instance_settings::created_by.eq(actor),
                            instance_settings::created_at.eq(now),
                        ))
                        .on_conflict(instance_settings::key)
                        .do_update()
                        .set((
                            instance_settings::value.eq(&value),
                            instance_settings::updated_by.eq(actor),
                            instance_settings::updated_at.eq(now),
                        ))
                        .execute(conn)?;
                }
                None => {
                    diesel::delete(instance_settings::table.filter(instance_settings::key.eq(key)))
                        .execute(conn)?;
                }
            }

            diesel::insert_into(instance_setting_changes::table)
                .values(record)
                .execute(conn)?;
            Ok(())
        })
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to write the instance setting".to_string())
}

async fn append_record(state: &AppState, record: NewInstanceSettingChange) -> Result<(), String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(instance_setting_changes::table)
            .values(record)
            .execute(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map(|_| ())
    .map_err(|_| "Failed to record the setting change".to_string())
}

/// What happened to one setting, newest first.
pub async fn history(
    state: &AppState,
    key: &str,
    limit: i64,
) -> Result<Vec<InstanceSettingChange>, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    let key = key.to_string();
    let limit = limit.clamp(1, 200);

    tokio::task::spawn_blocking(move || {
        instance_setting_changes::table
            .filter(instance_setting_changes::key.eq(key))
            .order(instance_setting_changes::changed_at.desc())
            .limit(limit)
            .select(InstanceSettingChange::as_select())
            .load::<InstanceSettingChange>(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to load the setting's history".to_string())
}

/// Every setting an operator has actually configured, for the surfaces that
/// want to show the instance's own answers rather than all thirty-two.
pub fn configured_keys(settings: &Settings) -> Vec<&'static str> {
    settings
        .all()
        .filter(|r| r.source == super::resolver::Source::Instance)
        .map(|r| r.key)
        .collect()
}

/// The lock these tests hold is `settings::test_env`'s, and it is held across
/// `await` deliberately: it exists to stop a test that sets an environment
/// variable and a test that writes the matching row from running at the same
/// time, and both of those are async. Nothing outside a `#[cfg(test)]` module
/// ever takes it, and each test runs on its own current-thread runtime, so
/// there is no task to starve and nothing to deadlock against.
#[cfg(test)]
#[allow(clippy::await_holding_lock)]
mod tests {
    use super::*;
    use crate::settings::resolver::Source;
    use crate::settings::test_env::lock;
    use crate::test_support::{insert_test_user, test_app_state};

    /// Every test here writes real rows, so each one clears the keys it
    /// touches first. There is no per-test database.
    async fn clear(state: &AppState, keys: &[&str]) {
        let mut conn = state.db_pool.get().expect("conn");
        for key in keys {
            diesel::delete(instance_settings::table.filter(instance_settings::key.eq(*key)))
                .execute(&mut conn)
                .expect("cleared");
            diesel::delete(
                instance_setting_changes::table.filter(instance_setting_changes::key.eq(*key)),
            )
            .execute(&mut conn)
            .expect("cleared");
        }
    }

    #[tokio::test]
    async fn a_change_records_who_when_and_what_it_was_before() {
        let _guard = lock();
        let state = test_app_state();
        clear(&state, &["notice.contact_email"]).await;
        let mut conn = state.db_pool.get().expect("conn");
        let actor = insert_test_user(&mut conn);

        write_setting(
            &state,
            "notice.contact_email",
            Some("first@realdomain.org"),
            Some(actor),
            ChangeSource::Setup,
        )
        .await
        .expect("first write");
        write_setting(
            &state,
            "notice.contact_email",
            Some("second@realdomain.org"),
            Some(actor),
            ChangeSource::Admin,
        )
        .await
        .expect("second write");

        let history = history(&state, "notice.contact_email", 10)
            .await
            .expect("history");
        assert_eq!(history.len(), 2);
        let newest = &history[0];
        assert_eq!(
            newest.previous_value.as_deref(),
            Some("first@realdomain.org")
        );
        assert_eq!(newest.new_value.as_deref(), Some("second@realdomain.org"));
        assert_eq!(newest.changed_by, Some(actor));
        assert_eq!(newest.source, "admin");
        assert!(!newest.redacted);

        clear(&state, &["notice.contact_email"]).await;
    }

    /// FR-008 against FR-023. The record must be useful and must not be a
    /// second copy of the credential.
    #[tokio::test]
    async fn a_secret_records_the_transition_and_neither_value() {
        let _guard = lock();
        let state = test_app_state();
        clear(&state, &["mail.password"]).await;

        write_setting(
            &state,
            "mail.password",
            Some("first-password"),
            None,
            ChangeSource::Admin,
        )
        .await
        .expect("first write");
        write_setting(
            &state,
            "mail.password",
            Some("second-password"),
            None,
            ChangeSource::Admin,
        )
        .await
        .expect("second write");

        let history = history(&state, "mail.password", 10).await.expect("history");
        assert_eq!(history.len(), 2);
        for row in &history {
            assert!(row.redacted, "a secret's record must say it is redacted");
            for value in [&row.previous_value, &row.new_value] {
                let rendered = value.as_deref().unwrap_or_default();
                assert!(
                    rendered == "set" || rendered == "not set",
                    "a secret rendered as `{rendered}`"
                );
            }
        }
        assert_eq!(history[1].previous_value.as_deref(), Some("not set"));
        assert_eq!(history[0].previous_value.as_deref(), Some("set"));

        clear(&state, &["mail.password"]).await;
    }

    /// A secret is stored as ciphertext and read back through `crypto.rs`. If
    /// the plaintext were ever in the row, every read surface's redaction
    /// would be the only thing standing between it and a `SELECT`.
    #[tokio::test]
    async fn a_secret_is_stored_encrypted_and_reads_back() {
        let _guard = lock();
        let state = test_app_state();
        clear(&state, &["mail.password"]).await;

        write_setting(
            &state,
            "mail.password",
            Some("hunter2"),
            None,
            ChangeSource::Admin,
        )
        .await
        .expect("write");

        let mut conn = state.db_pool.get().expect("conn");
        let stored = instance_settings::table
            .filter(instance_settings::key.eq("mail.password"))
            .select(instance_settings::value)
            .first::<String>(&mut conn)
            .expect("row");
        assert!(stored.starts_with("v1."), "not the crypto.rs format");
        assert!(!stored.contains("hunter2"));

        let resolved = resolve_all(&state).await.expect("resolves");
        assert_eq!(resolved.value("mail.password"), Some("hunter2"));

        clear(&state, &["mail.password"]).await;
    }

    /// FR-007: the next request sees it, with no restart. The resolver reads
    /// per request precisely so this is true.
    #[tokio::test]
    async fn a_changed_setting_is_visible_to_the_next_read() {
        let _guard = lock();
        let state = test_app_state();
        clear(&state, &["operator.name"]).await;

        assert!(
            !resolve_all(&state)
                .await
                .expect("resolves")
                .is_set("operator.name")
        );
        write_setting(
            &state,
            "operator.name",
            Some("The Thunder Collective"),
            None,
            ChangeSource::Admin,
        )
        .await
        .expect("write");

        let after = resolve_all(&state).await.expect("resolves");
        assert_eq!(after.value("operator.name"), Some("The Thunder Collective"));
        assert_eq!(after.get("operator.name").unwrap().source, Source::Instance);

        clear(&state, &["operator.name"]).await;
    }

    /// Clearing leaves a trail, and the setting falls back to its default.
    #[tokio::test]
    async fn clearing_a_value_is_a_recorded_change_and_not_a_delete() {
        let _guard = lock();
        let state = test_app_state();
        clear(&state, &["mail.from_name"]).await;

        write_setting(
            &state,
            "mail.from_name",
            Some("ThunderForge"),
            None,
            ChangeSource::Admin,
        )
        .await
        .expect("write");
        let cleared = write_setting(&state, "mail.from_name", None, None, ChangeSource::Admin)
            .await
            .expect("clear");

        assert_eq!(cleared.source, Source::Default);
        let history = history(&state, "mail.from_name", 10)
            .await
            .expect("history");
        assert_eq!(history[0].previous_value.as_deref(), Some("ThunderForge"));
        assert_eq!(history[0].new_value, None);

        clear(&state, &["mail.from_name"]).await;
    }

    /// FR-004 refuses before anything is written, so a rejected value leaves
    /// no row and no record of itself.
    #[tokio::test]
    async fn a_refused_value_is_not_written_and_not_recorded() {
        let _guard = lock();
        let state = test_app_state();
        clear(&state, &["operator.contact_email"]).await;

        let refusal = write_setting(
            &state,
            "operator.contact_email",
            Some("nobody@thunderforge.local"),
            None,
            ChangeSource::Admin,
        )
        .await
        .expect_err("refused");
        assert!(refusal.contains("reserved"));
        assert!(!refusal.contains("nobody@thunderforge.local"));

        assert!(
            history(&state, "operator.contact_email", 10)
                .await
                .expect("history")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn an_undeclared_key_cannot_be_written() {
        let _guard = lock();
        let state = test_app_state();
        let refusal = write_setting(
            &state,
            "operator.shoe_size",
            Some("11"),
            None,
            ChangeSource::Admin,
        )
        .await
        .expect_err("refused");
        assert!(refusal.contains("does not declare") || refusal.contains("declares"));
    }

    /// Spec 035 keeps its own mutation and its own audit trail. A second write
    /// path would produce policy changes `instance_access_events` never saw.
    #[tokio::test]
    async fn the_access_policy_is_not_writable_through_this_surface() {
        let _guard = lock();
        let state = test_app_state();
        let refusal = write_setting(
            &state,
            "instance.access_policy",
            Some("open"),
            None,
            ChangeSource::Admin,
        )
        .await
        .expect_err("refused");
        assert!(refusal.contains("setInstanceAccessPolicy"));
    }
}
