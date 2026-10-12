//! Spec 088's server counters (`specs/088-first-session-feedback/contracts/
//! telemetry.md`), against spec 086's contract
//! (`specs/086-full-telemetry/contracts/server-instruments.md`).
//!
//! Each records through spec 086's [`Recorders`]. Before the meter provider
//! is installed, or with `TELEMETRY=false`, there are no recorders and each
//! call records nothing. Every attribute value comes from a closed set
//! below; nothing carries a link code, a world, a map or a person.

use async_graphql::{Error, Value};
use opentelemetry::KeyValue;

use crate::map_import::MapImportError;
use crate::telemetry::instruments::{Recorders, recorders};

pub const JOINS: &str = "thunderforge.world_links.joins";
pub const OAUTH_REFUSED: &str = "thunderforge.world_links.oauth_refused";
pub const BASE_MAPS_APPLIED: &str = "thunderforge.base_maps.applied";
pub const ROLLS_CLEARED: &str = "thunderforge.rolls.cleared";
pub const PERIMETER_WALLS: &str = "thunderforge.map_import.perimeter_walls";

/// `outcome` on `thunderforge.world_links.joins`. The browser's
/// `signed_out` never reaches the server, so it is not one of them.
pub const JOIN_OUTCOMES: &[&str] = &[
    "joined",
    "already_member",
    "revoked",
    "expired",
    "used_up",
    "unknown",
];
/// `outcome` on `thunderforge.base_maps.applied`.
pub const BASE_MAP_OUTCOMES: &[&str] = &["ok", "upload_failed", "import_failed"];
/// The perimeter histogram's buckets: one per count, 8 and over in the last.
pub const PERIMETER_BOUNDARIES: &[f64] = &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
/// The count recorded for an import that walled more edges than that.
pub const PERIMETER_CAP: usize = 8;

/// A join's outcome, from what `joinWorld` answered. A failure that is not
/// a link refusal (a missing session, a database error) is not counted.
pub fn join_outcome<T>(result: &Result<T, Error>) -> Option<&'static str> {
    let error = match result {
        Ok(_) => return Some("joined"),
        Err(error) => error,
    };
    let code = error.extensions.as_ref().and_then(|e| e.get("code"));
    let Some(Value::String(code)) = code else {
        return None;
    };
    match code.as_str() {
        "ALREADY_MEMBER" => Some("already_member"),
        "LINK_REVOKED" => Some("revoked"),
        "LINK_EXPIRED" => Some("expired"),
        "LINK_USED_UP" => Some("used_up"),
        "LINK_UNKNOWN" => Some("unknown"),
        _ => None,
    }
}

/// One settled `joinWorld`, when its outcome is one of [`JOIN_OUTCOMES`].
pub fn record_join<T>(result: &Result<T, Error>) {
    if let (Some(r), Some(outcome)) = (recorders(), join_outcome(result)) {
        count_join(r, outcome);
    }
}

fn count_join(r: &Recorders, outcome: &'static str) {
    r.world_link_joins
        .add(1, &[KeyValue::new("outcome", outcome)]);
}

/// An OAuth sign-in from a join page, refused for want of an account.
pub fn record_oauth_refused() {
    if let Some(r) = recorders() {
        r.world_link_oauth_refusals.add(1, &[]);
    }
}

/// How applying a new world's starting map ended.
pub fn base_map_outcome(result: &Result<(), MapImportError>) -> &'static str {
    match result {
        Ok(()) => "ok",
        Err(MapImportError::Storage(_)) => "upload_failed",
        Err(_) => "import_failed",
    }
}

pub fn record_base_map(result: &Result<(), MapImportError>) {
    if let Some(r) = recorders() {
        count_base_map(r, base_map_outcome(result));
    }
}

fn count_base_map(r: &Recorders, outcome: &'static str) {
    r.base_maps_applied
        .add(1, &[KeyValue::new("outcome", outcome)]);
}

/// A Game Master's clear of the roll feed that committed.
pub fn record_rolls_cleared() {
    if let Some(r) = recorders() {
        r.rolls_cleared.add(1, &[]);
    }
}

/// How many edge walls an import added, 8 and over counted as 8.
pub fn record_perimeter_walls(count: usize) {
    if let Some(r) = recorders() {
        observe_perimeter(r, count);
    }
}

fn observe_perimeter(r: &Recorders, count: usize) {
    let capped = u32::try_from(count.min(PERIMETER_CAP)).unwrap_or(8);
    r.map_import_perimeter_walls.record(f64::from(capped), &[]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::instruments::testing::TestMeter;
    use async_graphql::ErrorExtensions;

    fn refusal(code: &str) -> Result<(), Error> {
        Err(Error::new("no").extend_with(|_, ext| ext.set("code", code)))
    }

    #[test]
    fn a_join_counts_by_its_refusal_and_nothing_else() {
        assert_eq!(join_outcome(&Ok::<(), Error>(())), Some("joined"));
        assert_eq!(join_outcome(&refusal("LINK_USED_UP")), Some("used_up"));
        assert_eq!(
            join_outcome(&refusal("ALREADY_MEMBER")),
            Some("already_member")
        );
        assert_eq!(join_outcome(&refusal("UNAUTHENTICATED")), None);
        assert_eq!(
            join_outcome(&Err::<(), _>(Error::new("Database error"))),
            None
        );
        for code in ["LINK_REVOKED", "LINK_EXPIRED", "LINK_UNKNOWN"] {
            let outcome = join_outcome(&refusal(code)).expect("a refusal");
            assert!(JOIN_OUTCOMES.contains(&outcome));
        }
    }

    #[test]
    fn a_base_map_failure_says_which_half_failed() {
        assert_eq!(base_map_outcome(&Ok(())), "ok");
        assert_eq!(
            base_map_outcome(&Err(MapImportError::Storage("down".into()))),
            "upload_failed"
        );
        assert_eq!(
            base_map_outcome(&Err(MapImportError::Io("gone".into()))),
            "import_failed"
        );
    }

    #[test]
    fn each_counter_records_under_its_policy_name() {
        let m = TestMeter::new();
        let r = Recorders::new(&m.meter);
        count_join(&r, "used_up");
        count_base_map(&r, "upload_failed");
        r.world_link_oauth_refusals.add(1, &[]);
        r.rolls_cleared.add(1, &[]);
        observe_perimeter(&r, 4);
        observe_perimeter(&r, 40);
        let seen = m.collect();
        assert_eq!(seen.get(&format!("{JOINS}{{outcome=used_up}}")), Some(&1.0));
        assert_eq!(
            seen.get(&format!("{BASE_MAPS_APPLIED}{{outcome=upload_failed}}")),
            Some(&1.0)
        );
        assert_eq!(seen.get(OAUTH_REFUSED), Some(&1.0));
        assert_eq!(seen.get(ROLLS_CLEARED), Some(&1.0));
        assert_eq!(seen.get(PERIMETER_WALLS), Some(&2.0));
    }
}
