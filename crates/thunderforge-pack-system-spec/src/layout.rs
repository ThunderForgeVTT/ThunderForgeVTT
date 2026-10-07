//! What an interface pack may say about arrangement.
//!
//! # The line this type enforces
//!
//! Declaring *where a value appears* is presentation. Declaring *what a value
//! is* is behaviour (spec 032 FR-003a). This enum is where that line stops
//! being a sentence in a specification and becomes something a pack physically
//! cannot cross: there is no variant carrying an expression, no variant taking
//! a condition, and no variant that reads a value in order to decide anything.
//!
//! `"value": "strengthMod"` is a reference to something the system already
//! publishes. `"value": "(strength - 10) / 2"` is a computation, and there is
//! nowhere here to put it.
//!
//! That matters more than it did when a pack was only a palette. A pack now
//! describes a character sheet, and the pressure to add "just one conditional"
//! — a bar that turns red below a quarter, a row hidden when a score is zero —
//! will arrive attached to a real rendering problem rather than as a proposal
//! anyone would refuse. Each of those is a claim about what a number *means*,
//! which belongs to the system that owns the rule.
//!
//! # Generic and specific addressing
//!
//! A construct addresses the system's declarations one of two ways (FR-025a):
//!
//! - **Generically**, by kind and declaration order — "every declared
//!   attribute". Names nothing, so it composes against a system that ships
//!   after the pack does. This is all Forge uses, which is what makes it
//!   simultaneously the universal fallback and the format's conformance
//!   reference (FR-025b, FR-007a).
//! - **Specifically**, by identifier — `deathSaves`, `spellSlots`. Validated
//!   against each system the pack targets, independently (FR-026).
//!
//! Both are needed because the shipping systems differ in kind, not degree: a
//! six-ability block with an eighteen-row skill list, a three-pool column with
//! no skills, a skills-only ladder with no abilities at all. A layout that
//! only names identifiers serves none of them generally; one that only
//! addresses sets cannot say that a modifier belongs beside its score.
//!
//! # Why there is no construct per kind of value
//!
//! There was: a `tracker` with a box count, and a `slotGrid` with a level
//! count. Both are gone, and the format got smaller while covering more.
//!
//! A `tracker` carrying `boxes` and `rows` was a layout stating what a value
//! *is*, and it was wrong for two of the three systems that have a track —
//! Fate's stress is one flat run of eight, Cypher's damage ladder has no marks
//! at all. But a layout never needed to know: `value` names an identifier, and
//! the value that arrives says whether it is a number, a pool, a run of marks
//! or a rung on a ladder. The layout says *where*, the value says *what*,
//! which is FR-003a stated as a shape rather than as a rule.
//!
//! `slotGrid` went the same way for the same reason. It existed because a
//! spell slot is two numbers per level and nothing said how level three's
//! total was named — but that is a fact about the declaration, and groups
//! (FR-033) are where it belongs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A set of declarations, addressed by kind rather than by name.
///
/// Ordering within a set is always the system's own declaration order, never
/// the pack's — a system lists its abilities the way its book does, and a pack
/// reordering them would be making a claim about the ruleset.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum DeclarationSet {
    /// What the system calls ability scores — three, four, six, or none.
    Attributes,
    /// Bars and pools: hit points, stress, Wish Points.
    Resources,
    Skills,
    Movement,
    /// Everything the system computes rather than stores.
    Derived,
    /// Everything the system declares that none of the named sets claim.
    ///
    /// The construct that makes "every shipping system has a usable sheet" a
    /// property rather than a hope (FR-034, SC-012). A system declaring
    /// something this build has never heard of still has it drawn, because a
    /// value missing from a sheet is indistinguishable from the character not
    /// having it (FR-035).
    ///
    /// Forge ends with one of these, so nothing a system publishes falls off
    /// the bottom of the base pack.
    Other,
}

/// One node of a pack's layout.
///
/// # Why `title` exists on a section, and where FR-003b bites
///
/// A section heading is the one place a pack supplies words of its own, and
/// that makes it the one place a pack could reproduce a publisher's sheet.
/// Copying "ATTACKS & SPELLCASTING" off a printed sheet is exactly what
/// FR-003b forbids, and no validator can tell an original heading from a
/// transcribed one.
///
/// It is allowed anyway, because the alternative — sections that cannot be
/// labelled at all — makes every sheet worse in order to enforce by
/// construction something that is genuinely a matter of authorship. For packs
/// bundled with the product this is a review obligation, and it is the only
/// obligation in this format that validation cannot carry.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum LayoutNode {
    // ---- containers ----------------------------------------------------
    Section {
        /// The pack's own words. See the type-level note on FR-003b.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// Whether it starts collapsed. A default, not a rule — a reader who
        /// opens it stays opened, and nothing here can force it shut again.
        #[serde(default)]
        collapsed: bool,
        children: Vec<LayoutNode>,
    },
    Column {
        children: Vec<LayoutNode>,
    },
    Row {
        children: Vec<LayoutNode>,
    },

    // ---- generic: addresses a set, names nothing ------------------------
    /// Each declaration in the set as a labelled badge, wrapped into columns.
    BadgeGrid {
        of: DeclarationSet,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        columns: Option<u8>,
    },
    /// Each declaration in the set as a bar. Resources, usually.
    BarStack {
        of: DeclarationSet,
    },
    /// Each declaration in the set as a row.
    RowList {
        of: DeclarationSet,
    },

    // ---- specific: names identifiers the target system declares ---------
    /// One value, by identifier.
    Value {
        id: String,
    },
    /// Two values side by side — a score and the modifier derived from it.
    ///
    /// A pair rather than a computation: both halves are things the system
    /// already publishes, and this only says they belong next to each other.
    Pair {
        value: String,
        beside: String,
    },
    /// A value given room to breathe — a paragraph, a list a player adds to.
    ///
    /// The distinction from [`Self::Value`] is how much space it gets, which
    /// is a layout decision. It says nothing about what the value *is*: that
    /// is the value's own kind, and a `block` naming a number is a number in a
    /// wide box rather than a claim that the number is prose.
    Block {
        id: String,
    },
}

impl LayoutNode {
    /// Every identifier this node names, and everything below it.
    ///
    /// This is what FR-026 validates against a target system's declarations,
    /// and what FR-025b checks is empty for Forge.
    pub fn referenced_ids(&self) -> Vec<&str> {
        let mut out = Vec::new();
        self.collect_ids(&mut out);
        out
    }

    fn collect_ids<'a>(&'a self, out: &mut Vec<&'a str>) {
        match self {
            Self::Section { children, .. } | Self::Column { children } | Self::Row { children } => {
                for child in children {
                    child.collect_ids(out);
                }
            }
            // Generic nodes name nothing. That is the whole property that
            // lets Forge compose against a system nobody has written yet.
            Self::BadgeGrid { .. } | Self::BarStack { .. } | Self::RowList { .. } => {}
            Self::Value { id } | Self::Block { id } => out.push(id),
            Self::Pair { value, beside } => {
                out.push(value);
                out.push(beside);
            }
        }
    }

    /// A short name for this node's kind, for messages and for the
    /// conformance test that checks Forge exercises every one of them.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Section { .. } => "section",
            Self::Column { .. } => "column",
            Self::Row { .. } => "row",
            Self::BadgeGrid { .. } => "badgeGrid",
            Self::BarStack { .. } => "barStack",
            Self::RowList { .. } => "rowList",
            Self::Value { .. } => "value",
            Self::Pair { .. } => "pair",
            Self::Block { .. } => "block",
        }
    }

    /// Every kind the format offers.
    ///
    /// Written out rather than derived, so adding a variant without adding it
    /// here fails the conformance test rather than silently shrinking what
    /// Forge is required to demonstrate.
    pub const ALL_KINDS: &'static [&'static str] = &[
        "section",
        "column",
        "row",
        "badgeGrid",
        "barStack",
        "rowList",
        "value",
        "pair",
        "block",
    ];

    /// Which kinds appear in this tree.
    pub fn kinds_present(nodes: &[LayoutNode]) -> Vec<&'static str> {
        let mut out = Vec::new();
        fn walk(node: &LayoutNode, out: &mut Vec<&'static str>) {
            if !out.contains(&node.kind()) {
                out.push(node.kind());
            }
            if let LayoutNode::Section { children, .. }
            | LayoutNode::Column { children }
            | LayoutNode::Row { children } = node
            {
                for child in children {
                    walk(child, out);
                }
            }
        }
        for node in nodes {
            walk(node, &mut out);
        }
        out
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
