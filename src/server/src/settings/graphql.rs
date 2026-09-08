//! The administrator's view of every setting, its source, its history, and
//! what this instance is not ready for.
//!
//! # Why this lives here and not in `graphql/`
//!
//! Every other GraphQL surface is a submodule of `crate::graphql`, declared in
//! `graphql.rs`. This one is declared by `settings/mod.rs` instead, so the
//! declaration list, the resolver, the write path and the surface that renders
//! them are read together — and so that adding a setting touches one
//! directory. The roots merge it by name like any other member; see the note
//! at the bottom of this file for the two lines that do it.
//!
//! # The redaction rule, in one sentence
//!
//! A declaration marked secret has exactly two renderings here: `SET` and
//! `NOT_SET`. Not masked, not truncated, not length-hinted. There is no
//! `sk-…3f9` in this surface, and the test at the bottom of this file asserts
//! it against every declaration rather than against the ones somebody
//! remembered (FR-023, FR-027, SC-007).

use async_graphql::{Context, Enum, Object, Result as GraphQLResult, SimpleObject};

use super::changes::{ChangeSource, history, write_setting};
use super::registry::{Kind, Requirement, declaration};
use super::resolver::{Resolved, Source, resolve_all};
use crate::graphql::{admin_user, app_state};
use crate::readiness;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "SettingSource")]
pub enum GraphQLSettingSource {
    Environment,
    Instance,
    Default,
}

impl From<Source> for GraphQLSettingSource {
    fn from(value: Source) -> Self {
        match value {
            Source::Environment => GraphQLSettingSource::Environment,
            Source::Instance => GraphQLSettingSource::Instance,
            Source::Default => GraphQLSettingSource::Default,
        }
    }
}

/// The only two things a secret is ever said to be.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "SecretState")]
pub enum GraphQLSecretState {
    Set,
    NotSet,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "SettingRequirement")]
pub enum GraphQLSettingRequirement {
    Optional,
    RequiredAtSetup,
    RequiredForCapability,
}

impl From<Requirement> for GraphQLSettingRequirement {
    fn from(value: Requirement) -> Self {
        match value {
            Requirement::Optional => GraphQLSettingRequirement::Optional,
            Requirement::RequiredAtSetup => GraphQLSettingRequirement::RequiredAtSetup,
            Requirement::RequiredFor(_) => GraphQLSettingRequirement::RequiredForCapability,
        }
    }
}

/// The shape of a value, so an editor renders the field the declaration
/// describes rather than one inferred from the key's name. `Enum` carries its
/// choices in `enum_options`; every other kind leaves that empty.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "SettingKind")]
pub enum GraphQLSettingKind {
    Text,
    Email,
    Url,
    Port,
    Bool,
    Enum,
    Prose,
}

impl From<Kind> for GraphQLSettingKind {
    fn from(value: Kind) -> Self {
        match value {
            Kind::Text => GraphQLSettingKind::Text,
            Kind::Email => GraphQLSettingKind::Email,
            Kind::Url => GraphQLSettingKind::Url,
            Kind::Port => GraphQLSettingKind::Port,
            Kind::Bool => GraphQLSettingKind::Bool,
            Kind::Enum(_) => GraphQLSettingKind::Enum,
            Kind::Prose => GraphQLSettingKind::Prose,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "ResolvedSetting")]
pub struct GraphQLResolvedSetting {
    pub key: String,
    /// The shape of the value, so an editor offers the right field.
    pub kind: GraphQLSettingKind,
    /// The permitted values, for `ENUM`. Empty for every other kind.
    pub enum_options: Vec<String>,
    /// Absent for a secret. Present and literal for everything else.
    pub value: Option<String>,
    /// Set / not set, for a secret. Null for everything else.
    pub secret_state: Option<GraphQLSecretState>,
    pub source: GraphQLSettingSource,
    /// The environment variable that fixed this value, when the source is
    /// ENVIRONMENT.
    pub fixed_by: Option<String>,
    /// False when the environment has fixed it, or when the value is written
    /// through another surface that keeps its own audit trail. Do not offer a
    /// field (FR-009).
    pub editable: bool,
    pub requirement: GraphQLSettingRequirement,
    /// What this setting enables, in a sentence a person can act on.
    pub capability: Option<String>,
    /// What to set, in a sentence. Never a value.
    pub what_to_set: String,
    /// Which group an editor should file this under, so a person setting "who
    /// a copyright notice is served on" is not doing it in a list of thirty.
    pub group: String,
    /// True when the stored value could not be read back — the instance secret
    /// was rotated. The setting resolves as unset, and this says why.
    pub undecryptable: bool,
}

impl From<&Resolved> for GraphQLResolvedSetting {
    fn from(resolved: &Resolved) -> Self {
        let d = resolved.declaration();
        // The one place the redaction rule is applied for this surface. It is
        // a `match` on the declaration and not on the value, so a secret with
        // a value and a secret without one take the same branch.
        let (value, secret_state) = if d.secret {
            let state = if resolved.is_set() {
                GraphQLSecretState::Set
            } else {
                GraphQLSecretState::NotSet
            };
            (None, Some(state))
        } else {
            (resolved.value.clone(), None)
        };

        GraphQLResolvedSetting {
            key: d.key.to_string(),
            kind: d.kind.into(),
            enum_options: match d.kind {
                Kind::Enum(options) => options.iter().map(|o| (*o).to_string()).collect(),
                _ => Vec::new(),
            },
            value,
            secret_state,
            source: resolved.source.into(),
            fixed_by: resolved.fixed_by.map(str::to_string),
            editable: resolved.editable(),
            requirement: d.requirement.into(),
            capability: d.capability.map(|c| c.label().to_string()),
            what_to_set: d.what_to_set.to_string(),
            group: d.group.to_string(),
            undecryptable: resolved.undecryptable,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "SettingChange")]
pub struct GraphQLSettingChange {
    pub key: String,
    pub previous_value: Option<String>,
    pub new_value: Option<String>,
    /// True when the values above are `set`/`not set` rather than the real
    /// ones. Stored on the row, not recomputed, so the record stays truthful
    /// if a declaration's secrecy ever changes.
    pub redacted: bool,
    pub changed_by: Option<String>,
    pub changed_at: String,
    pub source: String,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "ReadinessGap")]
pub struct GraphQLReadinessGap {
    pub setting_key: String,
    /// The variable that would also set it. Null when there is no environment
    /// form.
    pub env_var: Option<String>,
    /// What to set, in a sentence. Never a value, never a fragment, never a
    /// length.
    pub what_to_set: String,
    /// What is limited while this is unset.
    pub what_is_limited: String,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "InstanceCapability")]
pub struct GraphQLCapability {
    pub key: String,
    pub label: String,
    pub available: bool,
    pub gaps: Vec<GraphQLReadinessGap>,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "SourceFlip")]
pub struct GraphQLSourceFlip {
    pub setting_key: String,
    pub was: GraphQLSettingSource,
    pub now: GraphQLSettingSource,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "InstanceReadiness")]
pub struct GraphQLInstanceReadiness {
    pub capabilities: Vec<GraphQLCapability>,
    /// True only when every capability is available. Not an absence of
    /// complaints.
    pub fully_configured: bool,
    /// Rows in `instance_settings` the registry no longer declares. Reported,
    /// never deleted.
    pub unrecognised_settings: Vec<String>,
    /// A value whose source changed since the last start — an environment
    /// variable that appeared or vanished.
    pub source_flips: Vec<GraphQLSourceFlip>,
}

#[derive(Default)]
pub struct InstanceSettingsQuery;

#[Object]
impl InstanceSettingsQuery {
    /// Every declared setting, resolved, with its source. Administrators only.
    async fn instance_settings(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<Vec<GraphQLResolvedSetting>> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let settings = resolve_all(state).await?;
        Ok(settings.all().map(GraphQLResolvedSetting::from).collect())
    }

    /// The change history for one setting, newest first. Administrators only.
    async fn instance_setting_changes(
        &self,
        ctx: &Context<'_>,
        key: String,
        limit: Option<i32>,
    ) -> GraphQLResult<Vec<GraphQLSettingChange>> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let rows = history(state, &key, limit.unwrap_or(25).into()).await?;
        Ok(rows
            .into_iter()
            .map(|row| GraphQLSettingChange {
                key: row.key,
                previous_value: row.previous_value,
                new_value: row.new_value,
                redacted: row.redacted,
                changed_by: row.changed_by.map(|id| id.to_string()),
                changed_at: row.changed_at.to_string(),
                source: row.source,
            })
            .collect())
    }

    /// What this instance can and cannot do, given how it is configured.
    /// Derived on every read, never stored. Administrators only.
    async fn instance_readiness(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<GraphQLInstanceReadiness> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let report = readiness::report(state).await?;

        Ok(GraphQLInstanceReadiness {
            capabilities: report
                .capabilities
                .into_iter()
                .map(|c| GraphQLCapability {
                    key: c.key.to_string(),
                    label: c.label.to_string(),
                    available: c.available,
                    gaps: c
                        .gaps
                        .into_iter()
                        .map(|g| GraphQLReadinessGap {
                            setting_key: g.setting_key,
                            env_var: g.env_var,
                            what_to_set: g.what_to_set,
                            what_is_limited: g.what_is_limited,
                        })
                        .collect(),
                })
                .collect(),
            fully_configured: report.fully_configured,
            unrecognised_settings: report.unrecognised_settings,
            source_flips: report
                .source_flips
                .into_iter()
                .map(|f| GraphQLSourceFlip {
                    setting_key: f.setting_key,
                    was: f.was.into(),
                    now: f.now.into(),
                })
                .collect(),
        })
    }
}

#[derive(Default)]
pub struct InstanceSettingsMutation;

#[Object]
impl InstanceSettingsMutation {
    /// Write one setting.
    ///
    /// `value: null` clears the stored value; the setting then resolves from
    /// its default, and the change record shows the transition.
    ///
    /// Refused with a named reason when the environment has fixed it, when the
    /// value fails the declaration's validators, or when the caller is not an
    /// administrator. A refusal on an environment-fixed key names the
    /// variable rather than silently doing nothing — ADR-041 records what the
    /// silent no-op cost the OAuth surface.
    ///
    /// There is deliberately no bulk form and no delete. One key, one change
    /// record, one refusal reason.
    async fn update_instance_setting(
        &self,
        ctx: &Context<'_>,
        key: String,
        value: Option<String>,
    ) -> GraphQLResult<GraphQLResolvedSetting> {
        let state = app_state(ctx)?;
        let admin = admin_user(ctx)?;
        let resolved = write_setting(
            state,
            &key,
            value.as_deref(),
            Some(admin.user_id),
            ChangeSource::Admin,
        )
        .await?;
        Ok(GraphQLResolvedSetting::from(&resolved))
    }
}

/// True when this key is one an editor may offer a field for. Exposed so the
/// setup path asks the same question the admin surface does.
pub fn is_editable(key: &str) -> bool {
    declaration(key)
        .map(|d| d.env_name_in_use().is_none())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::registry::declarations;
    use crate::settings::resolver::Resolved;

    fn resolved(key: &'static str, value: Option<&str>) -> Resolved {
        Resolved {
            key,
            value: value.map(str::to_string),
            source: Source::Instance,
            fixed_by: None,
            undecryptable: false,
        }
    }

    /// SC-007, mechanically: **every** declaration marked secret, rendered by
    /// this surface, with a value present, must produce no trace of it. Not a
    /// sample — the point of one declaration list is that the check can be
    /// exhaustive.
    #[test]
    fn no_secret_reaches_the_read_surface_as_anything_but_set_or_not_set() {
        let credential = "correct-horse-battery-staple";
        for d in declarations().iter().filter(|d| d.secret) {
            let rendered = GraphQLResolvedSetting::from(&resolved(d.key, Some(credential)));
            assert_eq!(rendered.value, None, "`{}` returned a value", d.key);
            assert_eq!(rendered.secret_state, Some(GraphQLSecretState::Set));

            let whole = format!("{rendered:?}");
            assert!(!whole.contains(credential), "`{}` leaked it", d.key);
            assert!(
                !whole.contains(&credential.len().to_string()),
                "`{}` leaked its length",
                d.key
            );
            for fragment in ["correct", "staple", "horse"] {
                assert!(!whole.contains(fragment), "`{}` leaked a fragment", d.key);
            }
        }
    }

    /// An unset secret says so, rather than saying nothing — "not configured"
    /// and "we did not look" must not render alike.
    #[test]
    fn an_unset_secret_says_not_set() {
        let d = declaration("mail.password").expect("declared");
        let rendered = GraphQLResolvedSetting::from(&resolved(d.key, None));
        assert_eq!(rendered.secret_state, Some(GraphQLSecretState::NotSet));
        assert_eq!(rendered.value, None);
    }

    /// A non-secret is shown literally. The redaction rule must not spread by
    /// caution into settings an operator needs to read back.
    #[test]
    fn a_non_secret_is_rendered_literally() {
        let rendered = GraphQLResolvedSetting::from(&resolved(
            "notice.contact_email",
            Some("a@realdomain.org"),
        ));
        assert_eq!(rendered.value.as_deref(), Some("a@realdomain.org"));
        assert_eq!(rendered.secret_state, None);
    }

    /// FR-009: an environment-fixed value is not offered as editable, and it
    /// says which variable fixed it.
    #[test]
    fn an_environment_fixed_setting_is_not_editable_and_names_the_variable() {
        let rendered = GraphQLResolvedSetting::from(&Resolved {
            key: "support_email",
            value: Some("help@realdomain.org".to_string()),
            source: Source::Environment,
            fixed_by: Some("THUNDERFORGE_SUPPORT_EMAIL"),
            undecryptable: false,
        });
        assert!(!rendered.editable);
        assert_eq!(
            rendered.fixed_by.as_deref(),
            Some("THUNDERFORGE_SUPPORT_EMAIL")
        );
        assert_eq!(rendered.source, GraphQLSettingSource::Environment);
    }

    /// A surface that compiles but was never merged into the roots fails for
    /// the first operator who opens the settings screen, not for the suite.
    /// `mutations_instance_access.rs` keeps the same guard for the same
    /// reason.
    ///
    /// **This test fails until `InstanceSettingsQuery` and
    /// `InstanceSettingsMutation` are added to `QueryRoot` and `MutationRoot`
    /// in `src/server/src/graphql.rs`** — see the note at the top of this
    /// file. It is written now rather than after, because a guard added after
    /// registration is a guard nobody watched fail.
    #[test]
    fn the_settings_surface_is_registered_under_the_names_the_client_uses() {
        let schema = async_graphql::Schema::build(
            crate::graphql::QueryRoot::default(),
            crate::graphql::MutationRoot::default(),
            crate::graphql::SubscriptionRoot,
        )
        .finish();
        let sdl = schema.sdl();

        for field in [
            "instanceSettings",
            "instanceSettingChanges(key: String!",
            "instanceReadiness",
            "updateInstanceSetting(key: String!",
        ] {
            assert!(
                sdl.contains(field),
                "`{field}` must be reachable from the root"
            );
        }
    }
}
