//! Feature flags: a feature that is merged and can still be switched off
//! (spec 068, Story 2).
//!
//! # A flag is a setting
//!
//! There is no second mechanism here. A flag is a `Kind::Bool` declaration in
//! the registry's `Features` group, so it already has everything spec 040
//! gives a setting: an environment variable that beats the instance's stored
//! value, which beats the declared default; a change record saying who
//! switched it and when; a place in the administrator's settings page. It is
//! resolved per request, so switching one needs no rebuild and no restart.
//!
//! What this file adds is the two things a flag needs that a setting does
//! not: one question to ask at the point of enforcement ([`flag_on`]), and a
//! read the web app can make without being an administrator
//! ([`FeatureFlagsQuery`]).
//!
//! # Adding one
//!
//! Declare it in `registry/declarations.rs` under `group: FEATURES_GROUP`,
//! with a default; list it in [`FEATURES`], saying whether somebody who is
//! not signed in may know it; ask [`flag_on`] where the server enforces the
//! rule. Hiding the control in the web app is a courtesy, not the rule. The
//! tests below refuse a flag that is declared and not listed, or listed and
//! not declared.

use async_graphql::{Context, Object, Result as GraphQLResult, SimpleObject};

use super::resolver::{Settings, resolve_all};
use super::validate::parse_bool;
use crate::graphql::{app_state, authenticated_user};
use crate::state::AppState;

/// The registry group every flag is declared in.
pub const FEATURES_GROUP: &str = "Features";

/// Importing a source book into an account's library (spec 049).
pub const BOOK_IMPORT: &str = "feature.book_import";

/// Offering the demo at `/demo` to anyone who asks (spec 074).
pub const DEMO: &str = "feature.demo";

/// Fetching a large file in resumable parts (spec 080).
pub const DOWNLOAD_IN_PARTS: &str = "feature.download_in_parts";

/// Bringing a character in from an exported sheet (spec 048).
pub const SHEET_IMPORT: &str = "feature.sheet_import";

/// One flag, and who may be told how it is set.
#[derive(Debug, Clone, Copy)]
pub struct Feature {
    pub key: &'static str,
    /// Told to a visitor who has not signed in. For a flag that decides what
    /// a signed-out page shows; everything else is for members only.
    pub public: bool,
}

/// Every flag this instance has.
pub const FEATURES: &[Feature] = &[
    Feature {
        key: BOOK_IMPORT,
        public: false,
    },
    // Public: the sign-in page draws the link, and a stranger is who it is for.
    Feature {
        key: DEMO,
        public: true,
    },
    // Public: a visitor to the demo downloads the engine too.
    Feature {
        key: DOWNLOAD_IN_PARTS,
        public: true,
    },
    Feature {
        key: SHEET_IMPORT,
        public: false,
    },
];

/// Whether a flag is on, given everything already resolved.
///
/// A flag nobody has set reads as its declared default. A key that is not a
/// flag is off: asking about a feature that does not exist is a mistake in
/// the caller, and the safe answer to a mistake is "no".
pub fn is_on(settings: &Settings, key: &str) -> bool {
    FEATURES.iter().any(|feature| feature.key == key)
        && settings.value(key).and_then(parse_bool).unwrap_or(false)
}

/// Whether a flag is on for this request. The one question server code asks.
///
/// Fails only when the database cannot be reached, which the action being
/// guarded would not have survived either.
pub async fn flag_on(state: &AppState, key: &str) -> Result<bool, String> {
    Ok(is_on(&resolve_all(state).await?, key))
}

#[derive(SimpleObject, Debug, Clone, PartialEq, Eq)]
#[graphql(name = "FeatureFlag")]
pub struct GraphQLFeatureFlag {
    pub key: String,
    pub on: bool,
}

/// The flags a caller may know: the public ones for a visitor, all of them
/// for a signed-in member.
pub fn flags_for(settings: &Settings, signed_in: bool) -> Vec<GraphQLFeatureFlag> {
    FEATURES
        .iter()
        .filter(|feature| signed_in || feature.public)
        .map(|feature| GraphQLFeatureFlag {
            key: feature.key.to_string(),
            on: is_on(settings, feature.key),
        })
        .collect()
}

#[derive(Default)]
pub struct FeatureFlagsQuery;

#[Object]
impl FeatureFlagsQuery {
    /// Which features are switched on. Anyone may ask; a visitor who has not
    /// signed in is told only the flags declared public.
    async fn feature_flags(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLFeatureFlag>> {
        let state = app_state(ctx)?;
        let signed_in = authenticated_user(ctx).is_ok();
        Ok(flags_for(&resolve_all(state).await?, signed_in))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::registry::{Kind, declaration, declarations};

    #[test]
    fn every_flag_is_a_declared_boolean_with_a_default() {
        for feature in FEATURES {
            let d = declaration(feature.key)
                .unwrap_or_else(|| panic!("`{}` is listed and not declared", feature.key));
            assert_eq!(d.kind, Kind::Bool, "`{}`", feature.key);
            assert_eq!(d.group, FEATURES_GROUP, "`{}`", feature.key);
            assert!(!d.secret, "`{}`", feature.key);
            assert!(
                d.default.and_then(parse_bool).is_some(),
                "`{}` has no default, so nobody can say what it is before it is set",
                feature.key
            );
            assert!(feature.key.starts_with("feature."), "`{}`", feature.key);
        }
    }

    #[test]
    fn everything_declared_under_features_is_listed() {
        for d in declarations().iter().filter(|d| d.group == FEATURES_GROUP) {
            assert!(
                FEATURES.iter().any(|feature| feature.key == d.key),
                "`{}` is declared under Features and not listed in FEATURES",
                d.key
            );
        }
    }

    #[test]
    fn a_key_that_is_not_a_flag_is_off() {
        assert!(!is_on(&Settings::default(), "feature.nothing_by_this_name"));
        // A real setting that happens to be a boolean is still not a flag.
        assert!(!is_on(&Settings::default(), "mail.enabled"));
    }

    #[test]
    fn a_visitor_is_told_only_the_public_flags() {
        let settings = Settings::default();
        let visitor = flags_for(&settings, false);
        let member = flags_for(&settings, true);
        assert_eq!(member.len(), FEATURES.len());
        assert_eq!(
            visitor.len(),
            FEATURES.iter().filter(|feature| feature.public).count()
        );
        assert!(!visitor.iter().any(|flag| flag.key == BOOK_IMPORT));
    }
}
