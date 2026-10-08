//! Spec 084: how a game system shapes a roll and spends a reroll.
//!
//! The host knows that a roll can be rolled with advantage, that a sheet
//! can carry things that change how its rolls are made, and that some of
//! those things can be spent to roll again. It does not know which ones a
//! system has or what they are called. A pack that has any fills this slot
//! on its [`SystemContribution`](crate::system_contribution::SystemContribution).
//!
//! Plain data and function pointers: no dice, database or network types,
//! so this crate stays clean for wasm.

/// A facet or a spend, with the name the table sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FacetLabel {
    pub id: &'static str,
    pub label: &'static str,
}

/// How the roller asked for a d20 to be rolled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Advantage {
    #[default]
    Normal,
    Advantage,
    Disadvantage,
}

/// What a roll is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollKind {
    Check,
    ToHit,
    Damage,
}

/// What the pack is given to shape one roll.
pub struct ShapeInput<'a> {
    pub kind: RollKind,
    /// The formula as declared, placeholders unbound.
    pub formula: &'a str,
    /// The actor's `trait_data`, or `Null` for a roll with no sheet.
    pub trait_data: &'a serde_json::Value,
    /// `Normal` for damage.
    pub advantage: Advantage,
    /// Damage only: whether the attack was made in melee.
    pub melee: bool,
    /// Damage only: the properties of the item the attack came from.
    pub item_properties: &'a [String],
}

/// The formula to roll, and the facets that shaped it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shaped {
    pub formula: String,
    pub facets: Vec<String>,
}

/// What the pack is given to plan one reroll.
pub struct RerollInput<'a> {
    pub spend: &'a str,
    /// `Check` or `ToHit`.
    pub kind: RollKind,
    /// The formula as it was rolled.
    pub formula: &'a str,
    /// The facets the roll carries.
    pub facets: &'a [String],
    pub actor_name: &'a str,
    /// Every slot of the actor's system data, keyed by slot name.
    pub sheet: &'a serde_json::Value,
    /// The world's effective system settings, by id.
    pub settings: &'a serde_json::Value,
}

/// How the dice change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RerollEdit {
    /// Reroll the die with the lowest final value among dice of these sides.
    RerollLowest { sides: u32 },
    /// Roll this formula instead, replaying the recorded dice into it.
    Reshape { formula: String },
}

/// A reroll the pack allows: the dice edit and the sheet after the spend.
#[derive(Debug, Clone, PartialEq)]
pub struct RerollPlan {
    pub edit: RerollEdit,
    /// The actor's `trait_data` after the spend: the only slot a spend writes.
    pub trait_data: serde_json::Value,
}

/// Shapes a roll. `Ok(None)` rolls the formula untouched; `Err` is a
/// refusal shown to the roller.
pub type ShapeFn = fn(&ShapeInput<'_>) -> Result<Option<Shaped>, String>;
/// Plans a reroll. `Err` is a refusal shown to the roller.
pub type RerollFn = fn(&RerollInput<'_>) -> Result<RerollPlan, String>;

/// Everything a system says about shaping and rerolling its rolls.
pub struct RollFacets {
    pub shape: ShapeFn,
    pub reroll: RerollFn,
    /// What a roll can be rerolled by, in the order the table is offered it.
    pub spends: &'static [FacetLabel],
    /// The name of every facet id `shape` can return.
    pub labels: &'static [FacetLabel],
}
