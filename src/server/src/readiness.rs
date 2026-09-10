//! What this instance can and cannot do, given how it is configured.
//!
//! # Derived, never stored
//!
//! There is no `is_ready` column, no cache and no flag. Readiness is a pure
//! function of the declaration list and the resolver, computed per request. A
//! stored flag would be a second source of truth for a question that has one,
//! and it goes stale in exactly the direction that hurts: reporting ready
//! after a credential was revoked.
//!
//! # It names variables, never values
//!
//! A gap names a setting, the variable that would also set it, what to set and
//! what is limited. It never names a value, a fragment of one, or its length —
//! **including for settings that are set**, where the temptation is a masked
//! preview. There is no `sk-…3f9` anywhere in this feature (FR-027, SC-007).
//!
//! # The vocabulary is borrowed, not reinvented
//!
//! `repo_host::RegistrationProblem::guidance()` already answers "why can this
//! instance not synchronise lore, and what should I set" — every variant names
//! something an operator can act on, and `registration_from_env` returns every
//! problem rather than the first. Those strings are used here verbatim. A
//! second vocabulary for the same subsystem is how two diagnostics come to
//! disagree about the same misconfiguration.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use diesel::prelude::*;

use crate::repo_host::{RegistrationProblem, registration_from_env};
use crate::schema::instance_settings;
use crate::settings::registry::{Capability, SettingDeclaration, declarations};
use crate::settings::resolver::{Settings, Source, resolve_all};
use crate::settings::validate::placeholder_problem;
use crate::state::AppState;

/// One thing this instance cannot do, and what would fix it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gap {
    pub setting_key: String,
    /// The variable that would also set it. `None` for a prose setting, which
    /// has no environment form.
    pub env_var: Option<String>,
    pub what_to_set: String,
    pub what_is_limited: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityReport {
    pub key: &'static str,
    pub label: &'static str,
    pub available: bool,
    pub gaps: Vec<Gap>,
}

/// A value whose source changed since the last time this instance started.
///
/// The spec's edge case is "a container is redeployed with a fresh environment
/// and an existing database": the value silently reverts from the environment
/// to the stored row. This is how the operator is told rather than finding out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFlip {
    pub setting_key: String,
    pub was: Source,
    pub now: Source,
}

#[derive(Debug, Clone)]
pub struct ReadinessReport {
    pub capabilities: Vec<CapabilityReport>,
    /// True only when every capability is available. A positive assertion, not
    /// the absence of complaints — an empty gap list reads as "we did not
    /// check" (FR-025 scenario 3).
    pub fully_configured: bool,
    pub unrecognised_settings: Vec<String>,
    pub source_flips: Vec<SourceFlip>,
}

/// The whole report, for one request.
pub async fn report(state: &AppState) -> Result<ReadinessReport, String> {
    let settings = resolve_all(state).await?;
    let sync_problems = registration_from_env().err().unwrap_or_default();
    let capabilities = assess(&settings, &sync_problems);
    let source_flips = flips_since_last_boot(state, &settings).await;

    Ok(ReadinessReport {
        fully_configured: capabilities.iter().all(|c| c.available),
        capabilities,
        unrecognised_settings: settings.unrecognised().to_vec(),
        source_flips,
    })
}

/// The report's pure half: what the configuration says, with no I/O.
///
/// `sync_problems` is what `repo_host` found in the environment. Only the
/// problems it can see that a resolved setting cannot — an unreadable key, a
/// missing `git` — are used, because presence is answered by the resolver and
/// answering it twice would report a value set in a row as missing.
pub fn assess(settings: &Settings, sync_problems: &[RegistrationProblem]) -> Vec<CapabilityReport> {
    Capability::all()
        .iter()
        .map(|capability| {
            let mut gaps: Vec<Gap> = declarations()
                .iter()
                .filter(|d| belongs_to(d, *capability))
                .filter_map(|d| gap_for(d, settings))
                .collect();

            if *capability == Capability::SyncLore {
                gaps.extend(quality_gaps(sync_problems));
            }

            CapabilityReport {
                key: capability.key(),
                label: capability.label(),
                available: gaps.is_empty(),
                gaps,
            }
        })
        .collect()
}

/// Whether a declaration's absence is what stops a capability working.
///
/// `RequiredAtSetup` settings belong to identifying the operator: they are the
/// ones setup refuses to finish without, and an instance that cannot say who
/// runs it is exactly what they are for.
fn belongs_to(d: &SettingDeclaration, capability: Capability) -> bool {
    if d.is_required_at_setup() {
        return capability == Capability::IdentifyOperator;
    }
    d.required_for() == Some(capability)
}

/// A gap, if this setting is not usable as it stands. Three reasons, and they
/// are different enough that an operator needs to be told which.
fn gap_for(d: &SettingDeclaration, settings: &Settings) -> Option<Gap> {
    let resolved = settings.get(d.key)?;

    // A stored ciphertext that will not read back. `crypto.rs` documents the
    // cause — the instance secret was rotated — and an operator told only
    // "unset" would set it again and be no wiser.
    if resolved.undecryptable {
        return Some(Gap {
            setting_key: d.key.to_string(),
            env_var: d.env_var.map(str::to_string),
            what_to_set: format!(
                "The stored value for `{}` could not be read back, which happens when the \
                 instance secret has been rotated. Set it again. {}",
                d.key, d.what_to_set
            ),
            what_is_limited: d.what_is_limited.to_string(),
        });
    }

    let Some(value) = resolved.value.as_deref().filter(|v| !v.trim().is_empty()) else {
        return Some(Gap {
            setting_key: d.key.to_string(),
            env_var: d.env_var.map(str::to_string),
            what_to_set: d.what_to_set.to_string(),
            what_is_limited: d.what_is_limited.to_string(),
        });
    };

    // Research.md § D6: a value that *is* set can still be a placeholder.
    // `support_email` has shipped as `stewards@thunderforge.local` since before
    // this feature existed, and FR-004 only runs at setup — which does not run
    // again on an instance that already exists. Something has to ask the
    // question afterwards, and FR-028 forbids the something being a refusal to
    // start. So it is asked here.
    //
    // The refusal message is deliberately NOT reproduced: it is validation
    // prose about a value, and this report is read by more surfaces than the
    // editor is.
    placeholder_problem(d, value).map(|_| Gap {
        setting_key: d.key.to_string(),
        env_var: d.env_var.map(str::to_string),
        what_to_set: format!(
            "`{}` is still set to a value this instance would refuse at setup — blank, an \
             unreachable domain, or the placeholder it shipped with. {}",
            d.key, d.what_to_set
        ),
        what_is_limited: d.what_is_limited.to_string(),
    })
}

/// The problems the resolver cannot see: a key that is present and unreadable,
/// and a `git` that is not installed. Presence problems are left to the
/// resolver, which knows about rows as well as variables.
fn quality_gaps(problems: &[RegistrationProblem]) -> Vec<Gap> {
    problems
        .iter()
        .filter(|p| {
            !matches!(
                p,
                RegistrationProblem::MissingAppId
                    | RegistrationProblem::MissingAppSlug
                    | RegistrationProblem::MissingPrivateKey
            )
        })
        .map(|p| Gap {
            setting_key: "github_app.sync.private_key".to_string(),
            env_var: None,
            // Verbatim. Every variant names something an operator can act on
            // and none of them ever carries a key, a fragment or a length.
            what_to_set: p.guidance(),
            what_is_limited: "A world cannot be connected to a repository.".to_string(),
        })
        .collect()
}

/// The single predicate FR-026 and spec 039's FR-053 are both written against.
///
/// One function, called by every operation that publishes content beyond a
/// world, so there is one place to read and one place to change. It is a
/// **refusal**, not a warning: spec 039 depends on it being a gate, and a
/// client-side check is a warning.
///
/// Deliberately not consulted by anything that *reads* an existing share.
/// Removing the notice contact must not break links already issued — that
/// would be a data-loss event triggered by a configuration change. Only
/// creating a new one is refused.
pub async fn may_publish_beyond_world(state: &AppState) -> Result<(), String> {
    let settings = resolve_all(state).await?;
    if publishing_gaps(&settings).is_empty() {
        return Ok(());
    }
    Err(publish_refusal())
}

fn publishing_gaps(settings: &Settings) -> Vec<Gap> {
    declarations()
        .iter()
        .filter(|d| belongs_to(d, Capability::PublishBeyondWorld))
        .filter_map(|d| gap_for(d, settings))
        .collect()
}

/// The refusal, in one place so every gated mutation says the same thing.
///
/// It names what is missing and where to set it. It does not name a value,
/// because there is not one.
pub fn publish_refusal() -> String {
    "This instance cannot publish content outside a world until a contact for copyright \
     notices is set. An administrator sets it in Admin → Readiness. Playing, editing and \
     sharing inside a world are unaffected."
        .to_string()
}

// ---------------------------------------------------------------------------
// Source flips
// ---------------------------------------------------------------------------

/// Where the sources this instance saw at its last start are kept.
///
/// A row under the reserved prefix rather than a table: it is one small
/// bookkeeping value, it belongs with the settings it describes, and the
/// resolver already knows not to treat a reserved key as a setting or as an
/// operator's stale configuration.
const SNAPSHOT_KEY: &str = "system.source_snapshot";

static BOOT_FLIPS: OnceLock<Vec<SourceFlip>> = OnceLock::new();

/// What changed source since the last start, computed once per process.
///
/// The comparison happens the first time anybody asks for readiness, and the
/// snapshot is rewritten at that moment — so the answer is "since the last
/// start", not "since the last time you looked", and a flip stays reported for
/// the whole life of the process that observed it.
///
/// Two concurrent first calls may both compute and both write. That is benign:
/// they compute the same answer from the same snapshot and write the same new
/// one. It is not worth a lock to prevent a duplicate write of an identical
/// value.
///
/// A failure to read or write the snapshot is not a failure of the report.
/// Readiness answering "what have you not configured" must not itself be
/// something that can be unavailable.
async fn flips_since_last_boot(state: &AppState, settings: &Settings) -> Vec<SourceFlip> {
    if let Some(flips) = BOOT_FLIPS.get() {
        return flips.clone();
    }

    let previous = load_snapshot(state).await.unwrap_or_default();
    let current = current_sources(settings);
    let flips = compare(&previous, &current);
    let _ = store_snapshot(state, &current).await;
    let _ = BOOT_FLIPS.set(flips.clone());
    flips
}

fn current_sources(settings: &Settings) -> BTreeMap<String, String> {
    settings
        .all()
        .map(|r| (r.key.to_string(), r.source.as_str().to_string()))
        .collect()
}

fn compare(
    previous: &BTreeMap<String, String>,
    current: &BTreeMap<String, String>,
) -> Vec<SourceFlip> {
    current
        .iter()
        .filter_map(|(key, now)| {
            let was = previous.get(key)?;
            if was == now {
                return None;
            }
            Some(SourceFlip {
                setting_key: key.clone(),
                was: source_from_str(was),
                now: source_from_str(now),
            })
        })
        .collect()
}

fn source_from_str(value: &str) -> Source {
    match value {
        "environment" => Source::Environment,
        "instance" => Source::Instance,
        _ => Source::Default,
    }
}

async fn load_snapshot(state: &AppState) -> Result<BTreeMap<String, String>, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    let raw = tokio::task::spawn_blocking(move || {
        instance_settings::table
            .filter(instance_settings::key.eq(SNAPSHOT_KEY))
            .select(instance_settings::value)
            .first::<String>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to read the source snapshot".to_string())?;

    Ok(raw
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default())
}

async fn store_snapshot(
    state: &AppState,
    sources: &BTreeMap<String, String>,
) -> Result<(), String> {
    let json = serde_json::to_string(sources).map_err(|_| "Failed to encode".to_string())?;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(instance_settings::table)
            .values((
                instance_settings::key.eq(SNAPSHOT_KEY),
                instance_settings::value.eq(&json),
                instance_settings::updated_at.eq(now),
                instance_settings::created_at.eq(now),
            ))
            .on_conflict(instance_settings::key)
            .do_update()
            .set((
                instance_settings::value.eq(&json),
                instance_settings::updated_at.eq(now),
            ))
            .execute(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map(|_| ())
    .map_err(|_| "Failed to write the source snapshot".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::registry::{Kind, RESERVED_PREFIX, declaration};
    use crate::settings::test_env::temp_env;
    use crate::test_support::test_app_state;

    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(f)
    }

    /// A value this declaration would accept, so "everything configured" means
    /// configured *well* — a port that is not a port would otherwise leave the
    /// capability unavailable for a reason the test did not intend.
    fn a_valid_value(d: &SettingDeclaration) -> &'static str {
        match d.kind {
            Kind::Email => "operations@realdomain.org",
            Kind::Port => "587",
            Kind::Bool => "true",
            Kind::Enum(allowed) => allowed[0],
            Kind::Url => "https://realdomain.org",
            Kind::Text | Kind::Prose => "A value the operator supplied",
        }
    }

    /// Every declaration, set by the environment to a value it accepts, so a
    /// report can be examined with everything configured as well as with
    /// nothing.
    fn everything_set(body: impl FnOnce(Settings)) {
        let state = test_app_state();
        let vars: Vec<(&str, Option<&str>)> = declarations()
            .iter()
            .filter_map(|d| d.env_var.map(|name| (name, Some(a_valid_value(d)))))
            .collect();
        temp_env(&vars, || {
            let settings = block_on(resolve_all(&state)).expect("resolves");
            body(settings);
        });
    }

    /// The snapshot must live under the reserved prefix, or an operator would
    /// be told the instance's own bookkeeping is a stale setting of theirs.
    #[test]
    fn the_snapshot_is_kept_under_the_reserved_prefix() {
        assert!(SNAPSHOT_KEY.starts_with(RESERVED_PREFIX));
    }

    fn nothing_set(body: impl FnOnce(Settings)) {
        let state = test_app_state();
        let vars: Vec<(&str, Option<&str>)> = declarations()
            .iter()
            .flat_map(|d| d.env_var.into_iter().chain(d.env_aliases.iter().copied()))
            .map(|name| (name, None))
            .collect();
        // "Nothing set" has to mean the rows as well as the variables. A
        // declaration resolves from either, so clearing only the environment
        // leaves whatever the database happens to hold — and the shared
        // development database now holds a seeded notice contact (spec 039
        // T005), which made this helper quietly describe a *configured*
        // instance while claiming the opposite.
        let keys: Vec<&str> = declarations().iter().map(|d| d.key).collect();
        temp_env(&vars, || {
            let _rows = crate::settings::test_env::without_rows(&state, &keys);
            let settings = block_on(resolve_all(&state)).expect("resolves");
            body(settings);
        });
    }

    /// FR-027, and it is asserted for settings that are **set** as well as
    /// unset, because a masked preview of a configured credential is the
    /// tempting version of this mistake.
    #[test]
    fn no_gap_names_a_value_a_fragment_or_a_length() {
        let secret = "correct-horse-battery-staple";
        let state = test_app_state();
        let vars: Vec<(&str, Option<&str>)> = declarations()
            .iter()
            .filter_map(|d| d.env_var.map(|name| (name, Some(secret))))
            .collect();

        temp_env(&vars, || {
            let settings = block_on(resolve_all(&state)).expect("resolves");
            let reports = assess(&settings, &[]);
            for report in &reports {
                for gap in &report.gaps {
                    let rendered = format!("{gap:?}");
                    assert!(
                        !rendered.contains(secret),
                        "a gap named a configured value: {rendered}"
                    );
                    for fragment in ["correct", "horse", "staple"] {
                        assert!(!rendered.contains(fragment), "a gap named a fragment");
                    }
                    assert!(
                        !rendered.contains(&secret.len().to_string()),
                        "a gap named a length"
                    );
                }
            }
        });
    }

    /// Nothing configured is the state FR-028 and SC-009 are about: the report
    /// answers, every capability is unavailable rather than absent, and every
    /// gap says what to set.
    #[test]
    fn an_unconfigured_instance_is_told_exactly_what_to_set() {
        nothing_set(|settings| {
            let reports = assess(&settings, &[]);
            assert_eq!(reports.len(), Capability::all().len());
            for report in &reports {
                for gap in &report.gaps {
                    assert!(!gap.what_to_set.trim().is_empty(), "{gap:?}");
                    assert!(!gap.what_is_limited.trim().is_empty(), "{gap:?}");
                    assert!(
                        declaration(&gap.setting_key).is_some(),
                        "a gap named an undeclared setting: {gap:?}"
                    );
                }
            }
            let publishing = reports
                .iter()
                .find(|r| r.key == "publish_beyond_world")
                .expect("reported");
            assert!(!publishing.available);
            assert!(
                publishing
                    .gaps
                    .iter()
                    .any(|g| g.setting_key == "notice.contact_email")
            );
        });
    }

    /// FR-025 scenario 3: a fully configured instance says so positively. An
    /// empty list of complaints is not the same statement.
    #[test]
    fn a_fully_configured_instance_says_so() {
        everything_set(|settings| {
            let reports = assess(&settings, &[]);
            assert!(
                reports.iter().all(|r| r.available),
                "still unavailable: {:?}",
                reports
                    .iter()
                    .filter(|r| !r.available)
                    .map(|r| (r.key, r.gaps.clone()))
                    .collect::<Vec<_>>()
            );
            assert!(reports.iter().all(|r| r.gaps.is_empty()));
        });
    }

    /// § D6, made observable: a support address left at the shipped default is
    /// a gap, on an instance that has been running for a year and will never
    /// see setup again.
    #[test]
    fn a_shipped_placeholder_that_is_set_is_still_a_gap() {
        let state = test_app_state();
        temp_env(
            &[(
                "THUNDERFORGE_SUPPORT_EMAIL",
                Some("stewards@thunderforge.local"),
            )],
            || {
                let settings = block_on(resolve_all(&state)).expect("resolves");
                let reports = assess(&settings, &[]);
                let identity = reports
                    .iter()
                    .find(|r| r.key == "identify_operator")
                    .expect("reported");
                assert!(
                    identity
                        .gaps
                        .iter()
                        .any(|g| g.setting_key == "support_email"),
                    "a set-but-placeholder address was reported as configured"
                );
            },
        );
    }

    /// The gate refuses, and the refusal names what is missing and nothing
    /// else. Reading an existing share is deliberately not gated — see the
    /// function's own documentation.
    #[test]
    fn publishing_beyond_a_world_is_refused_while_the_notice_contact_is_unset() {
        nothing_set(|settings| {
            assert!(!publishing_gaps(&settings).is_empty());
            let refusal = publish_refusal();
            assert!(refusal.contains("copyright"));
            assert!(refusal.contains("inside a world are unaffected"));
        });
        everything_set(|settings| {
            assert!(publishing_gaps(&settings).is_empty());
        });
    }

    /// `repo_host`'s vocabulary, unchanged. A second set of words for the same
    /// misconfiguration is how two diagnostics come to disagree.
    #[test]
    fn a_repository_problem_is_reported_in_repo_hosts_own_words() {
        everything_set(|settings| {
            let problems = [RegistrationProblem::GitBinaryMissing];
            let reports = assess(&settings, &problems);
            let sync = reports
                .iter()
                .find(|r| r.key == "sync_lore")
                .expect("reported");
            assert!(!sync.available);
            assert_eq!(
                sync.gaps[0].what_to_set,
                RegistrationProblem::GitBinaryMissing.guidance()
            );
        });
    }

    /// Presence is the resolver's answer, not `repo_host`'s. Reporting a
    /// missing variable while a row holds the value would tell an operator to
    /// set something they have already set.
    #[test]
    fn a_presence_problem_is_not_reported_twice() {
        everything_set(|settings| {
            let problems = [
                RegistrationProblem::MissingAppId,
                RegistrationProblem::MissingAppSlug,
                RegistrationProblem::MissingPrivateKey,
            ];
            let reports = assess(&settings, &problems);
            let sync = reports
                .iter()
                .find(|r| r.key == "sync_lore")
                .expect("reported");
            assert!(sync.available, "{:?}", sync.gaps);
        });
    }

    /// The redeploy case: an environment variable that was in play at the last
    /// start and is not now.
    #[test]
    fn a_source_that_changed_since_the_last_start_is_reported() {
        let previous = BTreeMap::from([
            ("operator.name".to_string(), "environment".to_string()),
            ("support_email".to_string(), "instance".to_string()),
        ]);
        let current = BTreeMap::from([
            ("operator.name".to_string(), "instance".to_string()),
            ("support_email".to_string(), "instance".to_string()),
        ]);
        let flips = compare(&previous, &current);
        assert_eq!(
            flips,
            vec![SourceFlip {
                setting_key: "operator.name".to_string(),
                was: Source::Environment,
                now: Source::Instance,
            }]
        );
    }

    /// A setting that did not exist at the last start is not a flip. An
    /// upgrade that adds a declaration would otherwise report every new
    /// setting as having changed.
    #[test]
    fn a_newly_declared_setting_is_not_a_flip() {
        let previous = BTreeMap::new();
        let current = BTreeMap::from([("mail.host".to_string(), "instance".to_string())]);
        assert!(compare(&previous, &current).is_empty());
    }

    /// The whole report, against a real database, with nothing configured.
    /// This is SC-009's shape: an instance with every setting unset answers
    /// rather than failing.
    #[test]
    fn the_report_answers_with_every_setting_unset() {
        let state = test_app_state();
        let vars: Vec<(&str, Option<&str>)> = declarations()
            .iter()
            .flat_map(|d| d.env_var.into_iter().chain(d.env_aliases.iter().copied()))
            .map(|name| (name, None))
            .collect();
        temp_env(&vars, || {
            let report = block_on(report(&state)).expect("the report answers");
            assert!(!report.fully_configured);
            assert_eq!(report.capabilities.len(), Capability::all().len());
            assert!(
                !report
                    .unrecognised_settings
                    .iter()
                    .any(|k| k.starts_with(RESERVED_PREFIX)),
                "the instance's own bookkeeping was reported as an operator's leftover"
            );
        });
    }

    /// Spec 040 T039, FR-028 and SC-009: an instance with **every** setting
    /// unset still starts and still serves.
    ///
    /// The report answering is the test above. This is the other half, and the
    /// distinction matters: a readiness report that answers "nothing is
    /// configured" is worthless if the thing reporting it could not come up.
    /// The failure this guards against is a required setting introduced by an
    /// upgrade that makes an existing deployment refuse to start — the single
    /// worst outcome available to this feature, because the operator cannot
    /// even reach the screen that would tell them what to set.
    ///
    /// Three things are exercised, chosen because each is a place a missing
    /// setting could plausibly become a panic rather than a gap:
    ///
    /// 1. **The schema builds.** Every root is constructed, which is what the
    ///    binary does before it binds a port.
    /// 2. **An anonymous query is answered.** `publishedOperatorValues` is the
    ///    one the legal pages read through `/api/graphql/public`, and it is
    ///    the query most entitled to fail here, since with nothing set it has
    ///    nothing to report. Answering with unset markers is correct; erroring
    ///    is not.
    /// 3. **The mail seam resolves.** It is the only subsystem that builds a
    ///    live client out of settings, so it is the one that would panic on an
    ///    absent host rather than degrade.
    #[test]
    fn the_server_builds_and_serves_with_every_setting_unset() {
        let state = test_app_state();
        let vars: Vec<(&str, Option<&str>)> = declarations()
            .iter()
            .flat_map(|d| d.env_var.into_iter().chain(d.env_aliases.iter().copied()))
            .map(|name| (name, None))
            .collect();

        temp_env(&vars, || {
            let schema = async_graphql::Schema::build(
                crate::graphql::QueryRoot::default(),
                crate::graphql::MutationRoot::default(),
                crate::graphql::SubscriptionRoot,
            )
            .data(state.clone())
            .finish();

            let response =
                block_on(schema.execute(
                    "query { publishedOperatorValues { operatorName noticeContactEmail } }",
                ));
            assert!(
                response.errors.is_empty(),
                "an instance with nothing configured could not answer the anonymous \
                 operator query: {:?}",
                response.errors
            );

            let settings = block_on(resolve_all(&state)).expect("resolves");
            let transport = state.mail.transport(&settings);
            assert!(
                !transport.availability().is_ready(),
                "mail reported itself ready with no settings at all"
            );

            // Every declaration resolves to something — a default or nothing —
            // and none of them is an error. A declaration that could fail to
            // resolve is a declaration that could stop the server.
            for d in declarations() {
                assert!(
                    settings.get(d.key).is_some(),
                    "`{}` did not resolve with everything unset",
                    d.key
                );
            }
        });
    }
}
