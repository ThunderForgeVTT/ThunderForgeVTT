//! Where a piece of content came from, and therefore whether it may be shared
//! (spec 049 FR-050, decision 4).
//!
//! Two values, and the line between them is mechanical: content either came
//! through the authoring tools or came out of a document somebody supplied.
//! Nobody has to judge whether a work is commercial, so nobody can get that
//! judgment wrong — which is the whole reason this replaced a rule that asked
//! a Game Master to declare it.
//!
//! # Why this lives beside its table rather than in `db_types`
//!
//! The other two Postgres enums are shared vocabulary used from several
//! modules. This one is the value every sharing rule in this arc is enforced
//! against, and keeping it next to the only code that writes it means a
//! reader of [`super::store`] can see in one place that there is no path
//! here that sets it to anything but [`ContentOrigin::Uploaded`].

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};

/// The two ways a piece of content can have come to exist.
///
/// A **system pack's** content is a third thing and is deliberately not a
/// variant: the platform distributes it under the pack's own `legal` block
/// (FR-050b), it has no row in `compendiums`, and giving it a value here
/// would invite code to treat "shipped by us" as a kind of upload.
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[ExistingTypePath = "crate::schema::sql_types::ContentOrigin"]
// The migration writes 'Authored'/'Uploaded'; `diesel-derive-enum` would
// otherwise send snake_case and every insert would fail at the database with
// "invalid input value for enum" — invisible to the type checker, and the
// reason `CanvasImageAssetKindEnum` carries the same attribute.
#[DbValueStyle = "PascalCase"]
pub enum ContentOrigin {
    /// Made in ThunderForge by a person, through the authoring tools.
    /// Shareable, and the raw material of a collection (spec 026).
    Authored,
    /// Read out of a document somebody supplied. Never shareable, published,
    /// exported, or adopted into a collection — whatever licence the document
    /// carried (FR-052, FR-056).
    Uploaded,
}

impl ContentOrigin {
    /// Whether content of this origin may leave the account that holds it.
    ///
    /// The single sentence every refusal in this arc is derived from, so that
    /// "may this be shared?" is answered in one place rather than re-derived
    /// at each route out. A new route that forgets to ask is a bug; a new
    /// route that asks and gets a different answer would be worse.
    pub fn may_be_shared(self) -> bool {
        match self {
            ContentOrigin::Authored => true,
            ContentOrigin::Uploaded => false,
        }
    }

    /// Why an upload was refused, in the words shown to the person refused.
    ///
    /// FR-053 asks each refusal to say why, and FR-056a asks it to name the
    /// two routes that remain open — because the refusal will land most often
    /// on somebody who has done nothing wrong, including somebody who wrote
    /// the document themselves.
    pub fn refusal_reason(self) -> Option<&'static str> {
        match self {
            ContentOrigin::Authored => None,
            ContentOrigin::Uploaded => Some(
                "This content was read out of a book you uploaded, and uploaded \
                 content stays with the account that uploaded it. To share \
                 something like it, author it with the content tools, or propose \
                 it as a system pack.",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule, stated as a test so that widening it is a deliberate edit
    /// rather than a plausible-looking refactor. FR-052: the licence of the
    /// uploaded document makes no difference, because no licence is consulted.
    #[test]
    fn uploaded_content_is_never_shareable_and_authored_always_is() {
        assert!(!ContentOrigin::Uploaded.may_be_shared());
        assert!(ContentOrigin::Authored.may_be_shared());
    }

    /// FR-053 and FR-056a: a refusal names the origin as the reason and names
    /// the two routes that remain.
    #[test]
    fn the_refusal_names_the_origin_and_the_two_routes_that_remain() {
        let reason = ContentOrigin::Uploaded
            .refusal_reason()
            .expect("uploaded content must carry a reason for its refusal");
        assert!(reason.contains("uploaded"));
        assert!(reason.contains("author it"));
        assert!(reason.contains("system pack"));
        assert!(ContentOrigin::Authored.refusal_reason().is_none());
    }
}
