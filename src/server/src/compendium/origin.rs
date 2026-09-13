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
//!
//! # Where "may not leave" is enforced
//!
//! Not here, and not in any route (FR-054a, ADR-097). The database refuses
//! any row that would put uploaded content in a collection or behind a share
//! link — see the migration `2026-09-13-120000-0000_origin_invariant` — so a
//! route added next year is held to it without knowing it exists. What lives
//! here is the vocabulary: [`origin_of`], the one per-entry lookup that the
//! guard and the application both ask, and the refusal a person is shown.

use diesel::prelude::*;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// What every refusal of uploaded content says, wherever it is refused.
///
/// One sentence for every route, so a person meeting it at a collection, at a
/// share link or in an export reads the same reason and the same two ways
/// forward (FR-053, FR-056a). It names no licence, because none was consulted
/// (FR-056).
const UPLOADED_REFUSAL: &str = "This content was read out of a book you uploaded, and uploaded \
     content stays with the account that uploaded it. To share something like it, author it \
     with the content tools, or propose it as a system pack.";

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
            ContentOrigin::Uploaded => Some(UPLOADED_REFUSAL),
        }
    }
}

/// The content types [`origin_of`] answers for that are not world artifacts.
///
/// A collection's member types live in `collections::MEMBER_TYPES`; these are
/// the names the database's `content_origin` function knows for everything
/// else, so a caller asking about a book entry spells it the way the guard
/// does.
pub mod content_type {
    /// One entry read out of a book, asked about by its own id.
    pub const COMPENDIUM_ENTRY: &str = "compendium_entry";
    /// A whole book on a shelf.
    pub const COMPENDIUM: &str = "compendium";
}

diesel::define_sql_function! {
    /// The database's answer, from the migration
    /// `2026-09-13-120000-0000_origin_invariant`.
    fn content_origin(
        content_type: diesel::sql_types::Text,
        content_id: diesel::sql_types::Uuid
    ) -> diesel::sql_types::Nullable<crate::schema::sql_types::ContentOrigin>;
}

/// Where one piece of content came from, asked of that one entry.
///
/// **The single lookup every sharing rule in this arc goes through**, and a
/// thin one on purpose: it asks the same database function the collection and
/// share triggers ask, rather than restating which table holds which origin.
/// Two statements of that mapping — one here and one in SQL — would be two
/// answers that can drift, and the one that drifted would be whichever a
/// route happened to call.
///
/// `None` is "not established": the content does not exist, or its type is
/// one nobody has said the origin of. It is never a pass — the guard refuses
/// it — and a caller should read it as "not found" rather than as authored.
///
/// A new kind of content (spec 050's deltas, for one) becomes subject to the
/// invariant by gaining an arm in the SQL function; nothing here changes.
pub fn origin_of(
    conn: &mut PgConnection,
    content_type: &str,
    content_id: Uuid,
) -> QueryResult<Option<ContentOrigin>> {
    diesel::select(content_origin(content_type, content_id)).get_result(conn)
}

/// The marker the database's guard puts in every refusal it raises, so a
/// refusal can be told apart from any other failed write.
const GUARD_MARKER: &str = "(spec 049 FR-054a)";

/// Why the database refused to let something leave, when that is why a write
/// failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveRefusal {
    /// The content was uploaded. The reason a person is shown is
    /// [`ContentOrigin::refusal_reason`], and nothing else.
    Uploaded,
    /// Its origin could not be established — it is gone, or of a type the
    /// guard does not know. A route that checked existence first only meets
    /// this in a race with a deletion.
    NotEstablished,
}

impl LeaveRefusal {
    /// In the words shown to the person refused.
    pub fn message(self) -> &'static str {
        match self {
            LeaveRefusal::Uploaded => UPLOADED_REFUSAL,
            LeaveRefusal::NotEstablished => "That content could not be found",
        }
    }
}

/// Recognise the guard's refusal in a failed write.
///
/// So that a route which never thought to ask about origin — the case the
/// guard exists for — can still hand its caller FR-053's reason instead of a
/// database error, by passing its error through here.
pub fn refusal_from_database(error: &diesel::result::Error) -> Option<LeaveRefusal> {
    let diesel::result::Error::DatabaseError(_, info) = error else {
        return None;
    };
    let message = info.message();
    if !message.contains(GUARD_MARKER) {
        return None;
    }
    if message.starts_with("uploaded content") {
        Some(LeaveRefusal::Uploaded)
    } else {
        Some(LeaveRefusal::NotEstablished)
    }
}

#[cfg(test)]
#[path = "origin_tests.rs"]
mod guard_tests;

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
