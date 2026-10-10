//! The five server points of spec 048 research R16, against spec 086's
//! contract (`specs/086-full-telemetry/contracts/server-instruments.md`).
//!
//! Each point records through spec 086's [`Recorders`], whose names and
//! units are the policy crate's `INSTRUMENTS`. Before the meter provider is
//! installed, or with `TELEMETRY=false`, there are no recorders and each
//! call records nothing.
//!
//! Every attribute value comes from a closed set below. `system` and
//! `reader` are pack-declared ids, so they are bounded too. Nothing here
//! carries a name, an id or a piece of content.

use std::time::Duration;

use opentelemetry::KeyValue;
use thunderforge_sheet_import::{ImportPlan, PlanCertainty};

use super::error::SheetImportError;
use crate::telemetry::instruments::{Recorders, recorders};

pub const SHEET_IMPORTS: &str = "thunderforge.sheet_imports";
pub const READ_DURATION: &str = "thunderforge.sheet_import.read_duration";
pub const FIELDS: &str = "thunderforge.sheet_import.fields";
pub const DECISIONS: &str = "thunderforge.staged_content.decisions";
pub const UNADOPTED_USE_ATTEMPTS: &str = "thunderforge.unadopted_use_attempts";

/// `outcome` on `thunderforge.sheet_imports`.
pub const OUTCOMES: &[&str] = &[
    "applied",
    "refused_flag",
    "refused_permission",
    "refused_bounds",
    "refused_unrecognised",
    "refused_unmapped_system",
    "refused_plan_changed",
    "failed",
];
/// `certainty` on `thunderforge.sheet_import.fields`.
pub const CERTAINTIES: &[&str] = &["read", "uncertain", "unread", "corrected"];
/// `decision` on `thunderforge.staged_content.decisions`.
pub const DECISION_KINDS: &[&str] = &["adopt", "adopt_all", "decline", "revisit"];
/// `result` on `thunderforge.unadopted_use_attempts`.
pub const ATTEMPT_RESULTS: &[&str] = &["reported", "suppressed_stale", "rate_limited"];
/// `system` or `reader` before the server knows it: a refusal that came
/// before the actor's system or the reading was known.
pub const UNKNOWN: &str = "none";

/// The outcome an apply ended in.
pub fn outcome(result: &Result<(), &SheetImportError>) -> &'static str {
    let Err(error) = result else {
        return "applied";
    };
    match error {
        SheetImportError::Disabled => "refused_flag",
        SheetImportError::Forbidden(_) | SheetImportError::Refused { .. } => "refused_permission",
        SheetImportError::Sheet { code, .. } => match *code {
            "SHEET_NOT_RECOGNISED" => "refused_unrecognised",
            "SHEET_ENCRYPTED" | "SHEET_TOO_LARGE" | "SHEET_TOO_MANY_PAGES" => "refused_bounds",
            _ => "failed",
        },
        SheetImportError::NoMapping => "refused_unmapped_system",
        SheetImportError::PlanChanged => "refused_plan_changed",
        SheetImportError::Invalid(_)
        | SheetImportError::Storage(_)
        | SheetImportError::Database(_) => "failed",
    }
}

fn certainty(c: &PlanCertainty) -> &'static str {
    match c {
        PlanCertainty::Read => "read",
        PlanCertainty::Uncertain => "uncertain",
        PlanCertainty::Unread => "unread",
        PlanCertainty::Corrected => "corrected",
    }
}

/// One apply, whatever it ended in.
pub fn record_import(system: &str, reader: &str, outcome: &'static str) {
    if let Some(r) = recorders() {
        count_import(r, system, reader, outcome);
    }
}

fn count_import(r: &Recorders, system: &str, reader: &str, outcome: &'static str) {
    r.sheet_imports.add(
        1,
        &[
            KeyValue::new("system", system.to_owned()),
            KeyValue::new("reader", reader.to_owned()),
            KeyValue::new("outcome", outcome),
        ],
    );
}

/// How long the server's own reading took.
pub fn record_read_duration(reader: &str, took: Duration) {
    if let Some(r) = recorders() {
        time_read(r, reader, took);
    }
}

fn time_read(r: &Recorders, reader: &str, took: Duration) {
    r.sheet_import_read_duration.record(
        took.as_secs_f64(),
        &[KeyValue::new("reader", reader.to_owned())],
    );
}

/// How many fields of an applied plan were read, uncertain, unread or
/// corrected: one add per certainty that occurred, by its count.
pub fn record_fields(plan: &ImportPlan) {
    if let Some(r) = recorders() {
        count_fields(r, plan);
    }
}

fn count_fields(r: &Recorders, plan: &ImportPlan) {
    for (name, count) in field_counts(plan) {
        r.sheet_import_fields
            .add(count, &[KeyValue::new("certainty", name)]);
    }
}

fn field_counts(plan: &ImportPlan) -> Vec<(&'static str, u64)> {
    CERTAINTIES
        .iter()
        .map(|&name| {
            let fields = plan.fields.iter().map(|f| &f.certainty);
            let content = plan.content.iter().map(|c| &c.certainty);
            let count = fields
                .chain(content)
                .filter(|c| certainty(c) == name)
                .count();
            (name, count as u64)
        })
        .filter(|(_, count)| *count > 0)
        .collect()
}

/// A GM's decision on staged content, once it has committed: one add by
/// the number of pieces it decided.
pub fn record_decision(decision: &'static str, pieces: usize) {
    debug_assert!(DECISION_KINDS.contains(&decision));
    if let Some(r) = recorders() {
        count_decision(r, decision, pieces);
    }
}

fn count_decision(r: &Recorders, decision: &'static str, pieces: usize) {
    r.staged_content_decisions
        .add(pieces as u64, &[KeyValue::new("decision", decision)]);
}

/// One use of content the GM has not adopted, by what became of it.
pub fn record_attempt(result: &'static str) {
    debug_assert!(ATTEMPT_RESULTS.contains(&result));
    if let Some(r) = recorders() {
        count_attempt(r, result);
    }
}

fn count_attempt(r: &Recorders, result: &'static str) {
    r.unadopted_use_attempts
        .add(1, &[KeyValue::new("result", result)]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_outcome_is_in_the_closed_set() {
        let errors = [
            SheetImportError::Disabled,
            SheetImportError::Forbidden("no".into()),
            SheetImportError::Refused {
                code: "WORLD_PAUSED".into(),
                message: "paused".into(),
            },
            SheetImportError::sheet("SHEET_ENCRYPTED", "x"),
            SheetImportError::sheet("SHEET_TOO_LARGE", "x"),
            SheetImportError::sheet("SHEET_TOO_MANY_PAGES", "x"),
            SheetImportError::sheet("SHEET_NOT_RECOGNISED", "x"),
            SheetImportError::sheet("SHEET_UNREADABLE", "x"),
            SheetImportError::NoMapping,
            SheetImportError::PlanChanged,
            SheetImportError::Invalid("x".into()),
            SheetImportError::Storage("x".into()),
            SheetImportError::Database("x".into()),
        ];
        let mut seen: Vec<&str> = errors.iter().map(|e| outcome(&Err(e))).collect();
        seen.push(outcome(&Ok(())));
        for value in &seen {
            assert!(
                OUTCOMES.contains(value),
                "{value} is not a declared outcome"
            );
        }
        for declared in OUTCOMES {
            assert!(seen.contains(declared), "{declared} is never produced");
        }
    }

    #[test]
    fn certainties_are_the_closed_set() {
        for c in [
            PlanCertainty::Read,
            PlanCertainty::Uncertain,
            PlanCertainty::Unread,
            PlanCertainty::Corrected,
        ] {
            assert!(CERTAINTIES.contains(&certainty(&c)));
        }
    }

    #[test]
    fn the_report_results_are_the_closed_set() {
        use crate::staged_content::report::Outcome;
        let outcomes = [
            Outcome::SuppressedStale,
            Outcome::Counted,
            Outcome::Reported {
                chat_message_id: uuid::Uuid::nil(),
            },
        ];
        for o in &outcomes {
            assert!(ATTEMPT_RESULTS.contains(&o.result()));
        }
    }

    #[test]
    fn each_point_records_its_instrument_with_its_labels() {
        use crate::telemetry::instruments::testing::TestMeter;
        let m = TestMeter::new();
        let r = Recorders::new(&m.meter);
        count_import(&r, "dnd5e", "ddb-pdf", "applied");
        time_read(&r, "ddb-pdf", Duration::from_millis(40));
        count_decision(&r, "adopt_all", 3);
        count_attempt(&r, "rate_limited");
        let seen = m.collect();
        assert_eq!(
            seen["thunderforge.sheet_imports{outcome=applied,reader=ddb-pdf,system=dnd5e}"],
            1.0
        );
        assert_eq!(
            seen["thunderforge.sheet_import.read_duration{reader=ddb-pdf}"],
            1.0
        );
        assert_eq!(
            seen["thunderforge.staged_content.decisions{decision=adopt_all}"],
            3.0
        );
        assert_eq!(
            seen["thunderforge.unadopted_use_attempts{result=rate_limited}"],
            1.0
        );
    }
}
