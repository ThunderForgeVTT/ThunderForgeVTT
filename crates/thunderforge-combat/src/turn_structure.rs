//! Whether a ruleset counts rounds, and what it calls them (spec 031 FR-031).
//!
//! Absence means no rounds: a system that has not said it counts rounds has
//! not asked for a round counter. The server's `turn_structure.rs` reads the
//! manifest from disk and calls this; the reasoning is recorded there.

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
