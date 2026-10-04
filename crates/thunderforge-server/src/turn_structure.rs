//! Whether a ruleset counts rounds, and what it calls them.
//!
//! Spec 031's FR-031: turn and round structure is **determined by the active
//! game system**, and must not be imposed on systems that do not use it.
//! SC-011 measures it — a game system that does not use rounds presents no
//! round counter.
//!
//! # Why this is data and not a pack hook
//!
//! T076 was recorded as blocked on spec 032, and then on ADR-029 behind it, on
//! the reading that "system-supplied" meant a pack contributing behaviour.
//! It does not. Whether a ruleset has rounds is a *fact about the ruleset*,
//! stated once, and this is the same shape as [`crate::attributes`],
//! [`crate::status_display`] and [`crate::sheet`]: a manifest block, read
//! server-side, published as a value nothing downstream interprets.
//!
//! # Why absence means no rounds
//!
//! FR-031 says structure must not be *imposed*. A system that has not said it
//! counts rounds has not asked for a round counter, so it does not get one —
//! the product declines to assume rather than defaulting to the shape of the
//! ruleset that happened to be built first, which is the `DerivedStats`
//! mistake this codebase has now corrected three times.
//!
//! Every bundled manifest therefore says so explicitly, sourced from its own
//! research digest's `action_economy`, so that absence never silently applies
//! to a system that does count rounds:
//!
//! - **dnd5e**, **pathfinder2e**, **cypher_system**, **year_zero_engine** —
//!   rounds, by those names.
//! - **fate_core** — has rounds and calls them **exchanges**, which is exactly
//!   why the label travels with the flag rather than being hardcoded.
//! - **blades_in_the_dark** — "no strict turn order or initiative; the fiction
//!   determines who acts". No rounds, and the case SC-011 is written for.

/// What a system says about counting rounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnStructure {
    /// What this ruleset calls a round — "Round", Fate's "Exchange".
    ///
    /// Only present when the system counts them at all. A caller with no label
    /// has nothing to show, which is the whole of SC-011.
    pub round_label: Option<String>,
}

/// Read a system's turn structure from its manifest.
pub fn for_system(systems_dir: &str, system_id: &str) -> TurnStructure {
    let path = std::path::Path::new(systems_dir)
        .join(system_id)
        .join("system.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return TurnStructure { round_label: None };
    };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
        return TurnStructure { round_label: None };
    };
    from_manifest(&manifest)
}

/// Split out so it can be tested without a filesystem.
pub fn from_manifest(manifest: &serde_json::Value) -> TurnStructure {
    let Some(block) = manifest.get("turnStructure") else {
        return TurnStructure { round_label: None };
    };

    // `rounds: false` and an absent block are the same answer, deliberately:
    // a system declining rounds and a system that never mentioned them both
    // want no counter, and giving them two representations would invite a
    // caller to treat them differently.
    if block.get("rounds").and_then(|r| r.as_bool()) != Some(true) {
        return TurnStructure { round_label: None };
    }

    TurnStructure {
        round_label: Some(
            block
                .get("roundLabel")
                .and_then(|l| l.as_str())
                .filter(|label| !label.trim().is_empty())
                .unwrap_or("Round")
                .to_string(),
        ),
    }
}

#[cfg(test)]
#[path = "turn_structure_tests.rs"]
mod tests;
