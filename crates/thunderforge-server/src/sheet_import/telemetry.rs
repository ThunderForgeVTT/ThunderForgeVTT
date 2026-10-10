//! The five server points of spec 048 research R16, against spec 086's
//! contract (`specs/086-full-telemetry/contracts/server-instruments.md`).
//!
//! 086's policy crate is not on this branch yet, so each point is a
//! `tracing` event whose target is the OTel instrument's name and whose
//! fields are its attributes. When the branch meets 086, the names go into
//! the policy's `INSTRUMENTS` and these calls record through
//! `opentelemetry::global::meter("thunderforge")`; the call sites stay.
//!
//! Every attribute value comes from a closed set below. `system` and
//! `reader` are pack-declared ids, so they are bounded too. Nothing here
//! carries a name, an id or a piece of content.

use std::time::Duration;

use thunderforge_sheet_import::{ImportPlan, PlanCertainty};

use super::error::SheetImportError;

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
    tracing::info!(target: SHEET_IMPORTS, system, reader, outcome);
}

/// How long the server's own reading took.
pub fn record_read_duration(reader: &str, took: Duration) {
    let seconds = took.as_secs_f64();
    tracing::info!(target: READ_DURATION, reader, seconds);
}

/// How many fields of an applied plan were read, uncertain, unread or
/// corrected: one event per certainty that occurred, with its count.
pub fn record_fields(plan: &ImportPlan) {
    for (name, count) in field_counts(plan) {
        tracing::info!(target: FIELDS, certainty = name, count);
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

/// A GM's decision on staged content, once it has committed.
pub fn record_decision(decision: &'static str, pieces: usize) {
    debug_assert!(DECISION_KINDS.contains(&decision));
    tracing::info!(target: DECISIONS, decision, count = pieces as u64);
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
}
