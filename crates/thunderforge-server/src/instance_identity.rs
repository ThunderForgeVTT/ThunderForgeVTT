//! This deployment's stable identity.
//!
//! # What it is for
//!
//! Spec 034's binding record names which world *and which instance* is writing
//! to a repository (FR-036g). The instance half is what makes "this world, but
//! somewhere else" a conflict rather than a match — a world restored onto a
//! second deployment carries its own id, and without a second discriminator
//! two deployments writing to one repository would each conclude the binding
//! was theirs.
//!
//! # Where it lives, and the hole that leaves
//!
//! In the database, generated on first read. That is the right home for what
//! it is used for: the binding is about a world's data, the identity travels
//! with that data, and replacing a container or moving a deployment keeps it.
//!
//! **It does not survive being copied honestly.** Restore a database backup
//! onto a second machine and both deployments hold the same identity, so both
//! believe they own the same binding and neither detects the other. That is a
//! real hole and it is written down rather than papered over, because the
//! alternatives are worse: an identity in a config file has exactly the same
//! problem once the file is copied, and one generated per process changes on
//! every restart, which would make a deployment lose its own binding every
//! time it was redeployed.
//!
//! What closes it is a person: the binding issue names the world and the
//! instance, so two mirrors from one backup produce two claims a human can
//! see. FR-036i already says this is advisory rather than a lock, and this is
//! one of the reasons it has to be.
//!
//! # Why v4 rather than v7
//!
//! This value is published into an issue on a repository that may be public. A
//! v7 UUID front-loads a timestamp, so it would tell anyone reading when the
//! instance was first started. ADR-049 makes the same call about share codes,
//! and for the same reason: an identifier that leaks a fact nobody chose to
//! publish is a worse identifier.

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::instance_identity;

/// This deployment's identity, generating it on first call.
///
/// Idempotent under concurrency by construction rather than by locking: the
/// insert is `ON CONFLICT DO NOTHING` against a singleton primary key, so two
/// callers racing at first launch produce one row and both then read it. A
/// check-then-insert would have a window between the two, and first launch is
/// exactly when several requests arrive at once.
pub fn instance_id(conn: &mut PgConnection) -> Result<Uuid, String> {
    if let Some(existing) = instance_identity::table
        .select(instance_identity::instance_id)
        .first::<Uuid>(conn)
        .optional()
        .map_err(|e| format!("Could not read the instance identity: {e}"))?
    {
        return Ok(existing);
    }

    diesel::insert_into(instance_identity::table)
        .values((
            instance_identity::id.eq(1),
            instance_identity::instance_id.eq(Uuid::new_v4()),
        ))
        .on_conflict(instance_identity::id)
        .do_nothing()
        .execute(conn)
        .map_err(|e| format!("Could not create the instance identity: {e}"))?;

    instance_identity::table
        .select(instance_identity::instance_id)
        .first::<Uuid>(conn)
        .map_err(|e| format!("Could not read the instance identity back: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_app_state;

    #[test]
    fn the_identity_is_stable_across_calls() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("a connection");

        let first = instance_id(&mut conn).expect("an identity");
        let second = instance_id(&mut conn).expect("an identity");

        assert_eq!(first, second, "the instance changed identity between calls");
    }

    /// v4, not v7. This value is published into an issue on a repository that
    /// may be public, and a v7 would tell a reader when the instance was
    /// started.
    #[test]
    fn the_identity_does_not_carry_a_timestamp() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("a connection");

        let id = instance_id(&mut conn).expect("an identity");
        assert_eq!(
            id.get_version_num(),
            4,
            "the instance id leaks its creation time"
        );
    }

    /// The singleton constraint is the database's, not a convention. A second
    /// row would be a second identity, and a deployment with two identities
    /// cannot be told apart from two deployments.
    #[test]
    fn a_second_identity_row_is_refused() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("a connection");
        instance_id(&mut conn).expect("an identity");

        let second = diesel::insert_into(instance_identity::table)
            .values((
                instance_identity::id.eq(2),
                instance_identity::instance_id.eq(Uuid::new_v4()),
            ))
            .execute(&mut conn);

        assert!(second.is_err(), "a deployment took a second identity");
    }
}
