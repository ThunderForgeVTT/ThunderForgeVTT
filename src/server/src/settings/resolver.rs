//! One precedence rule, for every declared setting: **the environment beats
//! the instance's own store beats the declared default** (ADR-088, extending
//! ADR-041 from OAuth to the product).
//!
//! # Why a resolver and not a materialisation
//!
//! Spec 007/ADR-041 implemented "the environment wins" by *materialisation*:
//! `admin::materialize_env_oauth_providers` parses `OAUTH_*` at startup and
//! writes rows with `config_source = "env"`. That works, and it is kept — but
//! it cannot satisfy FR-007, which requires a change to take effect without a
//! redeploy or a restart, and once the row exists it cannot tell "the
//! environment set this" from "an administrator typed the same value".
//!
//! So this resolves per request instead. The cost is honest and recorded in
//! research.md § R5: there are now two mechanisms implementing one rule. The
//! alternative was a data migration of every deployment's provider rows for a
//! refactor, which FR-024 exists to prevent. Both surfaces report a source
//! from the same vocabulary, which is what stops them drifting.
//!
//! # One load, not one query per setting
//!
//! [`resolve_all`] reads the whole settings row-set once, the manifest file
//! once and the access policy once, and answers for every declaration from
//! that. Callers hold the returned [`Settings`] for the life of a request —
//! that *is* the memoisation. There is deliberately no process-wide cache: a
//! cached setting is a setting that keeps its old value after somebody changes
//! it, which is the failure FR-007 is about.

use std::collections::BTreeMap;

use diesel::prelude::*;

use super::registry::{
    Backing, Capability, Kind, RESERVED_PREFIX, Requirement, SettingDeclaration, declaration,
    declarations, read_env,
};
use crate::models::InstanceAccessSetting;
use crate::schema::{instance_access_settings, instance_settings};
use crate::state::AppState;

/// Which of the three tiers a live value came from.
///
/// The same three words the OAuth surface's `config_source` column says in a
/// different place, so an operator reading either one reads one vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Environment,
    Instance,
    Default,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Environment => "environment",
            Source::Instance => "instance",
            Source::Default => "default",
        }
    }
}

/// One setting, resolved.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub key: &'static str,
    /// Decrypted, for the server's own use. **This never leaves the process
    /// for a declaration marked secret** — the read surfaces render `set` or
    /// `not set` and nothing else.
    pub value: Option<String>,
    pub source: Source,
    /// The variable that fixed this value, when the source is the environment.
    /// FR-009's "and where it comes from": a field greyed out with no stated
    /// reason is the thing this feature exists to stop.
    pub fixed_by: Option<&'static str>,
    /// True when the stored ciphertext could not be read back. The value then
    /// resolves as unset — never as an empty string, and never as a panic.
    /// `crypto.rs` documents the cause: rotating `THUNDERFORGE_SECRET` makes
    /// every stored ciphertext unreadable.
    pub undecryptable: bool,
}

impl Resolved {
    pub fn declaration(&self) -> &'static SettingDeclaration {
        declaration(self.key).expect("a resolved setting is a declared setting")
    }

    pub fn is_set(&self) -> bool {
        self.value.as_ref().is_some_and(|v| !v.trim().is_empty())
    }

    /// Whether the administration surface may offer a field for this.
    ///
    /// Two reasons it may not, and they are different: the environment has
    /// fixed the value (FR-009), or the value is somebody else's to write —
    /// the access policy keeps spec 035's own mutation and its own audit
    /// trail, and a second write path to it would produce policy changes that
    /// its `instance_access_events` record never saw.
    pub fn editable(&self) -> bool {
        self.source != Source::Environment
            && !matches!(self.declaration().backing, Backing::AccessPolicy)
    }
}

/// Every declared setting, resolved once.
#[derive(Debug, Clone, Default)]
pub struct Settings {
    resolved: BTreeMap<&'static str, Resolved>,
    /// Rows whose key no declaration claims. Retained, never deleted, and
    /// reported in readiness — an operator who downgraded and upgraded again
    /// keeps their values (the posture ADR-041 took for provider rows).
    unrecognised: Vec<String>,
}

impl Settings {
    pub fn get(&self, key: &str) -> Option<&Resolved> {
        self.resolved.get(key)
    }

    /// The live value, or nothing. The ordinary read path for the rest of the
    /// server: `settings.value("notice.contact_email")`.
    pub fn value(&self, key: &str) -> Option<&str> {
        self.get(key)
            .and_then(|r| r.value.as_deref())
            .filter(|v| !v.trim().is_empty())
    }

    pub fn is_set(&self, key: &str) -> bool {
        self.value(key).is_some()
    }

    pub fn all(&self) -> impl Iterator<Item = &Resolved> {
        self.resolved.values()
    }

    pub fn unrecognised(&self) -> &[String] {
        &self.unrecognised
    }
}

/// The stored halves of the three backing stores, read once.
struct Stored {
    rows: BTreeMap<String, String>,
    manifest: BTreeMap<String, String>,
    shipped_manifest: BTreeMap<String, String>,
    access_policy: Option<String>,
}

/// Resolve every declared setting.
///
/// Reads the row-set, the manifest and the access policy once each. Fails only
/// when the database is unreachable: a missing manifest, an unset value and an
/// unreadable secret are all ordinary answers, not errors.
pub async fn resolve_all(state: &AppState) -> Result<Settings, String> {
    let stored = load_stored(state).await?;
    Ok(assemble(state, &stored))
}

/// One setting, resolved. A convenience over [`resolve_all`] for a caller that
/// genuinely wants one — it still costs the same load, which is why anything
/// asking for several should hold a [`Settings`] instead.
pub async fn resolve(state: &AppState, key: &str) -> Result<Resolved, String> {
    let settings = resolve_all(state).await?;
    settings
        .get(key)
        .cloned()
        .ok_or_else(|| format!("`{key}` is not a setting this instance declares"))
}

async fn load_stored(state: &AppState) -> Result<Stored, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    let (rows, access_policy) = tokio::task::spawn_blocking(move || {
        let rows = instance_settings::table
            .select((instance_settings::key, instance_settings::value))
            .load::<(String, String)>(&mut conn)?;
        let policy = instance_access_settings::table
            .filter(instance_access_settings::id.eq(1))
            .select(InstanceAccessSetting::as_select())
            .first::<InstanceAccessSetting>(&mut conn)
            .optional()?;
        Ok::<_, diesel::result::Error>((rows, policy.map(|p| p.access_policy)))
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to load instance settings".to_string())?;

    // A manifest that cannot be read is not an error here. It means the six
    // keys it backs are unset, which readiness reports; refusing to answer for
    // the other twenty-six because a file on a volume is missing would be the
    // wrong trade for FR-028.
    let manifest = crate::admin::read_system_manifest(state)
        .map(|m| m.metadata)
        .unwrap_or_default();

    Ok(Stored {
        rows: rows.into_iter().collect(),
        manifest,
        shipped_manifest: crate::admin::shipped_manifest_defaults(),
        access_policy,
    })
}

fn assemble(state: &AppState, stored: &Stored) -> Settings {
    let mut resolved = BTreeMap::new();
    for d in declarations() {
        resolved.insert(d.key, resolve_one(state, d, stored));
    }

    // A row under the reserved prefix is the instance's own bookkeeping, not a
    // setting somebody left behind, so it is not reported as unrecognised.
    let unrecognised = stored
        .rows
        .keys()
        .filter(|key| !key.starts_with(RESERVED_PREFIX) && declaration(key).is_none())
        .cloned()
        .collect();

    Settings {
        resolved,
        unrecognised,
    }
}

fn resolve_one(state: &AppState, d: &'static SettingDeclaration, stored: &Stored) -> Resolved {
    // 1. The environment. Checked before anything is read back from storage,
    //    because the whole rule is that nothing overrides it.
    if let Some(name) = d.env_name_in_use() {
        return Resolved {
            key: d.key,
            value: read_env(name),
            source: Source::Environment,
            fixed_by: Some(name),
            undecryptable: false,
        };
    }

    // 2. The instance's own store, whichever of the three it is for this
    //    declaration.
    let (stored_value, undecryptable) = match d.backing {
        Backing::Row => match stored.rows.get(d.key) {
            None => (None, false),
            Some(raw) if d.secret => match decrypt(state, raw) {
                Ok(plain) => (Some(plain), false),
                // The value is present and unreadable, which is not the same
                // as absent — but it must resolve as unset, because the only
                // other options are an empty string that reads as configured
                // and a panic. Readiness names the key.
                Err(_) => (None, true),
            },
            Some(raw) => (Some(raw.clone()), false),
        },
        Backing::ManifestFile(manifest_key) => {
            let value = stored.manifest.get(manifest_key);
            // A manifest value identical to the one this build ships is not
            // the instance's answer, it is the seed showing through. Saying so
            // is what lets readiness notice a support address nobody changed.
            match (value, stored.shipped_manifest.get(manifest_key)) {
                (Some(v), Some(shipped)) if v == shipped => (None, false),
                (v, _) => (v.cloned(), false),
            }
        }
        Backing::AccessPolicy => (stored.access_policy.clone(), false),
    };

    if let Some(value) = stored_value.filter(|v| !v.trim().is_empty()) {
        return Resolved {
            key: d.key,
            value: Some(value),
            source: Source::Instance,
            fixed_by: None,
            undecryptable,
        };
    }

    // 3. The declared default — including for a manifest key, where the
    //    shipped seed *is* the default and was recognised as such above.
    let default = match d.backing {
        Backing::ManifestFile(manifest_key) => stored
            .shipped_manifest
            .get(manifest_key)
            .cloned()
            .or_else(|| d.default.map(str::to_string)),
        _ => d.default.map(str::to_string),
    };

    Resolved {
        key: d.key,
        value: default.filter(|v| !v.trim().is_empty()),
        source: Source::Default,
        fixed_by: None,
        undecryptable,
    }
}

/// Read a stored secret back with the instance's key. One implementation, in
/// `crypto.rs`, for the reason that module's own docs give.
pub(crate) fn decrypt(state: &AppState, ciphertext: &str) -> Result<String, String> {
    let key = crate::crypto::encryption_key_from_config_secret(&state.config.secret)?;
    crate::crypto::decrypt_secret(ciphertext, &key)
}

/// Encrypt a secret for storage. Here rather than in the write path so that
/// encryption and decryption are read together.
pub(crate) fn encrypt(state: &AppState, plaintext: &str) -> Result<String, String> {
    let key = crate::crypto::encryption_key_from_config_secret(&state.config.secret)?;
    crate::crypto::encrypt_secret(plaintext, &key)
}

/// Whether a declaration is one setup must collect (FR-002, FR-003).
pub fn required_at_setup() -> impl Iterator<Item = &'static SettingDeclaration> {
    declarations().iter().filter(|d| d.is_required_at_setup())
}

/// Every declaration that must be set for one capability.
pub fn required_for(capability: Capability) -> impl Iterator<Item = &'static SettingDeclaration> {
    declarations()
        .iter()
        .filter(move |d| d.required_for() == Some(capability))
}

/// Whether a declaration's kind means its value is long-form prose. Used by
/// the read surface, which renders those differently.
pub fn is_prose(d: &SettingDeclaration) -> bool {
    matches!(d.kind, Kind::Prose)
}

/// Whether a declaration must be set before setup may complete.
pub fn setup_is_satisfied_by(settings: &Settings) -> bool {
    required_at_setup().all(|d| settings.is_set(d.key))
}

/// Whether a requirement is one the server may refuse to start over.
///
/// It is not, ever, for any of them. The function exists so the answer is
/// written down somewhere a change has to walk past: FR-028 and SC-009 say the
/// server always boots, and `repo_host` already behaves this way for `git` —
/// "says so at startup if it is missing, and does not refuse to boot."
pub fn is_a_startup_failure(_requirement: Requirement) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::registry::{Backing, declarations};
    use crate::settings::test_env::temp_env;
    use crate::test_support::test_app_state;

    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(f)
    }

    fn write_row(state: &AppState, key: &str, value: &str) {
        let mut conn = state.db_pool.get().expect("conn");
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(instance_settings::table)
            .values((
                instance_settings::key.eq(key),
                instance_settings::value.eq(value),
                instance_settings::updated_at.eq(now),
                instance_settings::created_at.eq(now),
            ))
            .on_conflict(instance_settings::key)
            .do_update()
            .set(instance_settings::value.eq(value))
            .execute(&mut conn)
            .expect("row written");
    }

    fn delete_row(state: &AppState, key: &str) {
        let mut conn = state.db_pool.get().expect("conn");
        diesel::delete(instance_settings::table.filter(instance_settings::key.eq(key)))
            .execute(&mut conn)
            .expect("row removed");
    }

    /// FR-010, asserted for **every** declaration rather than for a sample.
    ///
    /// A precedence rule that holds for the settings somebody remembered to
    /// test is the rule this feature exists to replace.
    #[test]
    fn the_environment_wins_for_every_setting_that_has_one() {
        let state = test_app_state();
        for d in declarations() {
            let Some(name) = d.env_var else { continue };
            // The aliases are cleared so the primary is unambiguously the one
            // in use — a leftover from another case would make `fixed_by`
            // name a variable this case never set.
            let mut vars: Vec<(&str, Option<&str>)> = vec![(name, Some("from-the-environment"))];
            vars.extend(d.env_aliases.iter().map(|alias| (*alias, None)));

            temp_env(&vars, || {
                let settings = block_on(resolve_all(&state)).expect("resolves");
                let r = settings.get(d.key).expect("declared settings resolve");
                assert_eq!(
                    r.source,
                    Source::Environment,
                    "`{}` did not take its value from `{name}`",
                    d.key
                );
                assert_eq!(r.fixed_by, Some(name), "`{}`", d.key);
                assert!(!r.editable(), "`{}` was offered as editable", d.key);
            });
        }
    }

    /// The second and third tiers, again for every setting the row store backs.
    #[test]
    fn a_row_beats_the_default_and_no_row_falls_back_to_it() {
        let state = test_app_state();
        for d in declarations() {
            if !matches!(d.backing, Backing::Row) {
                continue;
            }
            let stored = if d.secret {
                encrypt(&state, "a stored value").expect("encrypts")
            } else {
                "a stored value".to_string()
            };

            let vars: Vec<(&str, Option<&str>)> = d
                .env_var
                .into_iter()
                .chain(d.env_aliases.iter().copied())
                .map(|name| (name, None))
                .collect();

            temp_env(&vars, || {
                write_row(&state, d.key, &stored);
                let settings = block_on(resolve_all(&state)).expect("resolves");
                let r = settings.get(d.key).expect("resolves");
                assert_eq!(r.source, Source::Instance, "`{}`", d.key);
                assert_eq!(r.value.as_deref(), Some("a stored value"), "`{}`", d.key);
                assert!(r.editable(), "`{}` should be editable", d.key);

                delete_row(&state, d.key);
                let settings = block_on(resolve_all(&state)).expect("resolves");
                let r = settings.get(d.key).expect("resolves");
                assert_eq!(r.source, Source::Default, "`{}`", d.key);
                assert_eq!(
                    r.value.as_deref(),
                    d.default,
                    "`{}` did not fall back to its declared default",
                    d.key
                );
            });
        }
    }

    /// `crypto.rs` documents that rotating `THUNDERFORGE_SECRET` makes every
    /// stored ciphertext unreadable. The two wrong answers are an empty string
    /// that reads as configured and a panic that takes the admin surface down;
    /// the right one is "unset", with the key named in readiness.
    #[test]
    fn a_secret_that_will_not_decrypt_resolves_as_unset() {
        let state = test_app_state();
        temp_env(&[("THUNDERFORGE_SMTP_PASSWORD", None)], || {
            write_row(&state, "mail.password", "v1.not-a-real.ciphertext");
            let settings = block_on(resolve_all(&state)).expect("resolves rather than panicking");
            let r = settings.get("mail.password").expect("resolves");
            assert!(!r.is_set());
            assert!(r.undecryptable, "the reason must be visible to readiness");
            delete_row(&state, "mail.password");
        });
    }

    /// A PEM arrives by whichever of three variables the operator used, and
    /// `fixed_by` must name the one they actually set — a plausible answer
    /// here sends somebody to edit a variable that is not in play.
    #[test]
    fn an_alias_is_reported_as_the_variable_that_fixed_the_value() {
        let state = test_app_state();
        temp_env(
            &[
                (crate::repo_host::APP_PRIVATE_KEY_ENV, None),
                (crate::repo_host::APP_PRIVATE_KEY_FILE_ENV, None),
                (crate::repo_host::APP_PRIVATE_KEY_BASE64_ENV, Some("cGVt")),
            ],
            || {
                let settings = block_on(resolve_all(&state)).expect("resolves");
                let r = settings
                    .get("github_app.sync.private_key")
                    .expect("resolves");
                assert_eq!(r.source, Source::Environment);
                assert_eq!(
                    r.fixed_by,
                    Some(crate::repo_host::APP_PRIVATE_KEY_BASE64_ENV)
                );
            },
        );
    }

    /// An exported-but-empty variable is how a container platform says "I did
    /// not set this". Reading it as a configured empty string is how an
    /// instance ends up with a blank operator name it cannot edit.
    #[test]
    fn an_empty_variable_is_not_a_value() {
        let state = test_app_state();
        temp_env(&[("THUNDERFORGE_OPERATOR_NAME", Some("   "))], || {
            let settings = block_on(resolve_all(&state)).expect("resolves");
            let r = settings.get("operator.name").expect("resolves");
            assert_ne!(r.source, Source::Environment);
        });
    }

    /// A row the registry no longer declares is retained and reported, never
    /// resolved and never deleted — ADR-041's posture for provider rows.
    #[test]
    fn an_undeclared_row_is_inert_and_reported() {
        let state = test_app_state();
        write_row(&state, "operator.favourite_colour", "green");
        let settings = block_on(resolve_all(&state)).expect("resolves");
        assert!(settings.get("operator.favourite_colour").is_none());
        assert!(
            settings
                .unrecognised()
                .iter()
                .any(|k| k == "operator.favourite_colour")
        );
        delete_row(&state, "operator.favourite_colour");
    }

    /// The instance's own bookkeeping is not an operator's stale configuration.
    #[test]
    fn a_reserved_row_is_not_reported_as_unrecognised() {
        let state = test_app_state();
        write_row(&state, "system.a_test_marker", "{}");
        let settings = block_on(resolve_all(&state)).expect("resolves");
        assert!(
            !settings
                .unrecognised()
                .iter()
                .any(|k| k.starts_with("system."))
        );
        delete_row(&state, "system.a_test_marker");
    }
}
