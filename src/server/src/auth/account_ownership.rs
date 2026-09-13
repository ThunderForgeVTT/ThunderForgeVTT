//! Spec 049 / 050: does this account own this row?
//!
//! The account-scope counterpart to [`super::world_membership`], and net-new:
//! the server has had a well-used world-scope guard since spec 002 and
//! **nothing at all** for account scope (research §11). Mutations that operate
//! on a caller's own rows have compared `authenticated_user(ctx)?.user_id`
//! against the row's user column inline, which is fine when there is one row
//! and one column.
//!
//! A compendium is not that. FR-044's removal, FR-047's hash check, the
//! library view and every route Phase 7 has to refuse all ask the same
//! question — *does this account own this compendium?* — and asking it six
//! times inline is six chances to write it slightly differently, in the
//! feature where getting it wrong means showing one person another person's
//! imported book.
//!
//! # Why this takes a row and not an owner
//!
//! [`require_account_owner`] is given the thing being acted on, and looks the
//! owner up itself. Handing it an owner id the caller had already fetched
//! would be the inline comparison again, wearing a function's name: the
//! lookup is the part that gets written differently, not the `==`.
//!
//! # There is no admin bypass, and no parameter to add one with
//!
//! `is_dm_of_world` takes `is_admin` and treats a site admin as a DM, because
//! a world is a shared space an operator may have to act in. An account's
//! library is not: FR-055a says an uploaded book must not become reachable by
//! any other account, and an operator is another account. So this function has
//! no `is_admin` argument — not "and ignores it", but no argument at all, so a
//! call site cannot pass one and a future edit cannot quietly honour it.
//!
//! Operator action against imported content still has a route, and it is the
//! moderation one: a takedown removes content, which is a different act from
//! reading somebody's shelf.
//!
//! # Fail-closed
//!
//! Every unhappy answer is the same answer. A row that does not exist, a row
//! owned by somebody else, and a lookup that failed all refuse. `is_dm_of_scene`
//! is written the same way for the same reason: a database hiccup must only
//! ever be able to deny a request, never to grant one.

use diesel::prelude::*;
use uuid::Uuid;

/// Why an account-scoped request was refused.
///
/// Callers MUST treat **any** variant as "refuse the request". The two exist
/// so a server log can tell a wrong caller from a broken database, not so a
/// call site can decide one of them is survivable.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AccountOwnershipError {
    #[error("this does not belong to your account")]
    NotTheOwner,
    #[error("database error: {0}")]
    Database(String),
}

impl From<diesel::result::Error> for AccountOwnershipError {
    fn from(e: diesel::result::Error) -> Self {
        AccountOwnershipError::Database(e.to_string())
    }
}

/// A row that belongs to an account rather than to a world.
///
/// One variant today. It is an enum rather than a bare `compendium_id` so
/// that the next account-owned thing — spec 050's collections on the same
/// shelf — arrives as an arm in this match, beside a test that already covers
/// the shape, instead of as a second helper that answers the same question a
/// second way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountOwned {
    Compendium(Uuid),
}

/// Which account owns this row, or `None` if there is no such row.
///
/// Private on purpose: a caller that can ask "who owns this?" without being
/// told "and it is not you" is a caller that can forget the second half.
fn owner_of(conn: &mut PgConnection, owned: AccountOwned) -> QueryResult<Option<Uuid>> {
    match owned {
        AccountOwned::Compendium(id) => {
            use crate::schema::compendiums;
            compendiums::table
                .filter(compendiums::id.eq(id))
                .select(compendiums::owner_user_id)
                .first::<Uuid>(conn)
                .optional()
        }
    }
}

/// Refuse unless `user_id` is the account that owns `owned`.
///
/// The single gate for everything account-scoped in this arc. Synchronous
/// over a borrowed connection for the same reason [`super::world_membership::require_world_member`]
/// is: every call site is inside a `spawn_blocking` closure already holding a
/// `&mut PgConnection`, with no async available to it.
///
/// A row that does not exist refuses with [`AccountOwnershipError::NotTheOwner`]
/// rather than with a distinct "not found". That is deliberate and it is the
/// no-enumeration property in miniature: a caller who can tell "there is no
/// such compendium" from "there is one and it is not yours" can walk an id
/// space and learn what other people have on their shelves.
pub fn require_account_owner(
    conn: &mut PgConnection,
    user_id: Uuid,
    owned: AccountOwned,
) -> Result<(), AccountOwnershipError> {
    match owner_of(conn, owned)? {
        Some(owner) if owner == user_id => Ok(()),
        // Somebody else's, or nobody's. The same refusal, on purpose.
        _ => Err(AccountOwnershipError::NotTheOwner),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compendium::store::{self, NewBook};
    use crate::test_support::{insert_test_user, test_app_state};

    fn a_book() -> NewBook {
        NewBook {
            book_title: "A Book Somebody Owns".to_string(),
            source_hash: format!("{:0>64}", Uuid::now_v7().simple()),
            system_id: "test-system".to_string(),
            parser_version: "test".to_string(),
            page_count: 10,
            silent_page_count: 0,
        }
    }

    /// The happy answer, and the only one.
    #[test]
    fn the_importing_account_owns_what_it_imported() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let book = store::import_book(&mut conn, owner, a_book(), &[]).unwrap();

        assert!(
            require_account_owner(&mut conn, owner, AccountOwned::Compendium(book.id)).is_ok(),
            "the account that imported a book must own it"
        );
    }

    /// The failure this helper exists to make impossible to write six
    /// different ways: another account's book is not yours to read.
    #[test]
    fn another_account_is_refused() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let stranger = insert_test_user(&mut conn);
        let book = store::import_book(&mut conn, owner, a_book(), &[]).unwrap();

        let refused = require_account_owner(&mut conn, stranger, AccountOwned::Compendium(book.id));

        assert!(matches!(refused, Err(AccountOwnershipError::NotTheOwner)));
    }

    /// Fail-closed: an id that matches nothing is refused, and refused with
    /// the *same* error as somebody else's row. A distinguishable "no such
    /// compendium" would let an id space be walked for what other people hold.
    #[test]
    fn a_row_that_does_not_exist_is_refused_indistinguishably() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let caller = insert_test_user(&mut conn);
        let owner = insert_test_user(&mut conn);
        let somebody_elses = store::import_book(&mut conn, owner, a_book(), &[]).unwrap();

        let missing =
            require_account_owner(&mut conn, caller, AccountOwned::Compendium(Uuid::now_v7()));
        let not_mine = require_account_owner(
            &mut conn,
            caller,
            AccountOwned::Compendium(somebody_elses.id),
        );

        assert!(missing.is_err() && not_mine.is_err());
        assert_eq!(
            missing.unwrap_err().to_string(),
            not_mine.unwrap_err().to_string(),
            "a missing row and somebody else's row must refuse identically"
        );
    }

    /// A database failure denies. This exercises the conversion the fail-closed
    /// property rests on: any `diesel::result::Error` becomes a refusal, never
    /// an `Ok`.
    #[test]
    fn a_lookup_failure_is_a_refusal() {
        let refusal: AccountOwnershipError = diesel::result::Error::NotFound.into();
        assert!(matches!(refusal, AccountOwnershipError::Database(_)));
    }

    /// Regression guard, matching `world_membership`'s: call sites turn this
    /// error into a GraphQL refusal and its wording is shown to a person, so
    /// it should not change shape without somebody meaning it to.
    #[test]
    fn not_the_owner_error_message_is_stable() {
        assert_eq!(
            AccountOwnershipError::NotTheOwner.to_string(),
            "this does not belong to your account"
        );
    }
}
