//! Interactive elements: what a Game Master may author, and what happens when
//! somebody activates it.
//!
//! # Why this is a contribution seam and not a list of six features
//!
//! Spec 030 asks for six behaviours — a prop that opens a lore entry, a lever
//! that toggles lights, a door that opens and locks, a region that fires when
//! crossed, a request the GM approves. Built as six behaviours it becomes one
//! module that knows about lighting, walls, lore and navigation, and about
//! audio on the day audio exists. Constitution Principle II forbids exactly
//! that.
//!
//! So nothing here names an effect. A subsystem *contributes* its effects as
//! [`EffectDeclaration`]s, they are assembled into an [`EffectRegistry`], and
//! the authorable vocabulary is the union of what is compiled in. An unbuilt
//! subsystem contributes nothing, so nothing dead is ever offered — which
//! dissolves the "grey out the options we cannot perform" problem rather than
//! answering it.
//!
//! # Why the rules live here rather than in the engine
//!
//! Three surfaces must agree: the engine dispatches, the server validates and
//! persists, the web app builds an authoring form. Each holding its own list
//! guarantees drift, and drift here is silent — a GM authors an effect nothing
//! handles and at the table nothing happens. This crate is the one the server
//! compiles and the one the web app's types are generated from.
//!
//! It is also the only one whose tests *execute*. The engine crate targets
//! `wasm32-unknown-unknown` with no test runner, so its `#[cfg(test)]` modules
//! compile and never run. Every rule below is a rule somebody could get wrong,
//! which is why they are here and not there.
//!
//! See `docs/adrs/20260830-054-interaction_effect_contribution_seam.md`.

use std::collections::BTreeMap;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ---------------------------------------------------------------------------
// The authored vocabulary
// ---------------------------------------------------------------------------

/// What an interactive is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub enum SubjectKind {
    /// A token with no actor — a book, a chest, a lever.
    Prop,
    /// A wall designated as a door.
    Door,
    /// A bounded area of the scene, invisible to players.
    Region,
}

impl SubjectKind {
    pub fn from_str_loose(value: &str) -> Option<Self> {
        match value {
            "prop" => Some(SubjectKind::Prop),
            "door" => Some(SubjectKind::Door),
            "region" => Some(SubjectKind::Region),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            SubjectKind::Prop => "prop",
            SubjectKind::Door => "door",
            SubjectKind::Region => "region",
        }
    }
}

/// What makes an interactive fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    /// Somebody clicked the subject.
    Click,
    /// A token crossed into the region. Valid only for a region — a book
    /// cannot be crossed.
    Enter,
}

impl Trigger {
    pub fn from_str_loose(value: &str) -> Option<Self> {
        match value {
            "click" => Some(Trigger::Click),
            "enter" => Some(Trigger::Enter),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Trigger::Click => "click",
            Trigger::Enter => "enter",
        }
    }
}

/// Who may set it off, and whether the Game Master gets a say first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub enum Activation {
    /// Any world member.
    Anyone,
    /// Only whoever runs the world.
    GmOnly,
    /// A player raises a request; nothing happens until a GM decides.
    RequiresApproval,
}

impl Activation {
    pub fn from_str_loose(value: &str) -> Option<Self> {
        match value {
            "anyone" => Some(Activation::Anyone),
            "gm_only" => Some(Activation::GmOnly),
            "requires_approval" => Some(Activation::RequiresApproval),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Activation::Anyone => "anyone",
            Activation::GmOnly => "gm_only",
            Activation::RequiresApproval => "requires_approval",
        }
    }
}

/// How many times it may fire before a Game Master resets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub enum FireMode {
    #[default]
    Always,
    Once,
}

impl FireMode {
    pub fn from_str_loose(value: &str) -> Option<Self> {
        match value {
            "always" => Some(FireMode::Always),
            "once" => Some(FireMode::Once),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            FireMode::Always => "always",
            FireMode::Once => "once",
        }
    }
}

// ---------------------------------------------------------------------------
// What a contributor declares
// ---------------------------------------------------------------------------

/// One option in a [`ConfigFieldKind::Choice`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub struct ChoiceOption {
    pub value: String,
    pub label: String,
}

/// The type of one configuration field.
///
/// # There is deliberately no free-text kind
///
/// A link effect must reference in-world content by identifier, never by
/// address. Without a free-text field in the vocabulary, a Game Master
/// *cannot* point an interactive at an arbitrary destination — not because a
/// rule forbids it but because there is nothing to type it into.
///
/// That is what retires the hostile-destination edge case without an
/// allowlist (a moderation surface nobody agreed to own) or a confirmation
/// prompt (which puts the judgement on the player, who has the least context
/// about where the link came from). Adding a text kind here would quietly
/// bring all of that back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ConfigFieldKind {
    /// A yes/no.
    Boolean,
    /// One of a fixed set.
    Choice { options: Vec<ChoiceOption> },
    /// A single reference to something in this world.
    ///
    /// `of` names the sort of thing — `"wall"`, `"light"`, `"loreEntry"`,
    /// `"scene"`. It is a string rather than an enum on purpose: an enum would
    /// mean every subsystem gaining a referenceable thing edits this file,
    /// which is the coupling the whole seam exists to avoid.
    Reference { of: String },
    /// One or more references of the same sort.
    ReferenceList { of: String },
}

/// One field a Game Master fills in when authoring an effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub struct ConfigField {
    pub key: String,
    pub label: String,
    #[serde(flatten)]
    pub kind: ConfigFieldKind,
    /// Whether authoring is rejected without it.
    #[serde(default)]
    pub required: bool,
}

/// What a subsystem contributes to become triggerable.
///
/// Data, not behaviour — its three consumers need it in three places. The
/// handler lives in the plugin that owns the subsystem and never here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub struct EffectDeclaration {
    /// Stable, namespaced by contributor — `door.set_state`, `light.toggle`.
    ///
    /// The namespace is what makes collision detection a prefix concern
    /// rather than a coordination problem between teams.
    pub id: String,
    /// What a Game Master is shown when choosing.
    pub label: String,
    /// One line, in a GM's language rather than an engineer's.
    pub description: String,
    /// Which subjects it may attach to. A door effect belongs on a door.
    pub subject_kinds: Vec<SubjectKind>,
    /// What the authoring form renders and the server validates.
    pub config: Vec<ConfigField>,
}

impl EffectDeclaration {
    /// The contributor's namespace — everything before the first `.`.
    pub fn namespace(&self) -> &str {
        self.id.split('.').next().unwrap_or(&self.id)
    }

    pub fn field(&self, key: &str) -> Option<&ConfigField> {
        self.config.iter().find(|f| f.key == key)
    }
}

// ---------------------------------------------------------------------------
// The registry
// ---------------------------------------------------------------------------

/// Why a set of contributions could not be assembled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// Two contributors declared the same id.
    DuplicateId { id: String },
    /// An id with no namespace, which defeats collision detection.
    UnnamespacedId { id: String },
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::DuplicateId { id } => {
                write!(f, "two contributors declare the effect `{id}`")
            }
            RegistryError::UnnamespacedId { id } => {
                write!(f, "effect `{id}` is not namespaced by its contributor")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// Everything this build can perform.
///
/// Assembled once, at startup, from what is compiled in. Nothing marks an
/// effect unavailable — an absent contributor simply contributes nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectRegistry {
    declarations: BTreeMap<String, EffectDeclaration>,
}

impl EffectRegistry {
    /// Assemble contributed sets into a registry, or fail on collision.
    ///
    /// Failing *here* is the point. A duplicate id found when a Game Master
    /// happens to author one of the two is a collision found at the table,
    /// mid-session, by the people least able to do anything about it.
    pub fn assemble<I>(contributions: I) -> Result<Self, RegistryError>
    where
        I: IntoIterator<Item = Vec<EffectDeclaration>>,
    {
        let mut declarations: BTreeMap<String, EffectDeclaration> = BTreeMap::new();
        for set in contributions {
            for declaration in set {
                if !declaration.id.contains('.') {
                    return Err(RegistryError::UnnamespacedId { id: declaration.id });
                }
                if declarations.contains_key(&declaration.id) {
                    return Err(RegistryError::DuplicateId { id: declaration.id });
                }
                declarations.insert(declaration.id.clone(), declaration);
            }
        }
        Ok(Self { declarations })
    }

    pub fn get(&self, id: &str) -> Option<&EffectDeclaration> {
        self.declarations.get(id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.declarations.contains_key(id)
    }

    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }

    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    /// Every declaration, in id order — stable, so the authoring form does not
    /// reshuffle itself between builds.
    pub fn all(&self) -> impl Iterator<Item = &EffectDeclaration> {
        self.declarations.values()
    }

    /// What may be attached to a given subject.
    pub fn for_subject(&self, subject: SubjectKind) -> impl Iterator<Item = &EffectDeclaration> {
        self.declarations
            .values()
            .filter(move |d| d.subject_kinds.contains(&subject))
    }
}

// ---------------------------------------------------------------------------
// Regions
// ---------------------------------------------------------------------------

/// The bounded area of a region interactive.
///
/// Carried on the interactive rather than in `shapes`: a shape is authored
/// annotation the table sees, and a region is invisible and exists only to be
/// crossed. Storing them together would make every shape query filter out
/// regions and `visible_to_players` do two unrelated jobs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase", tag = "shape")]
pub enum RegionGeometry {
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    Polygon {
        /// `[x, y]` pairs, in order. Implicitly closed.
        points: Vec<[f32; 2]>,
    },
}

impl RegionGeometry {
    /// Whether a point is inside. Even-odd ray casting for a polygon; the
    /// boundary is deliberately not fussed over, because a token standing
    /// exactly on a region's edge is a rounding accident rather than a
    /// meaningful state.
    pub fn contains(&self, point: Vec2) -> bool {
        match self {
            RegionGeometry::Rect {
                x,
                y,
                width,
                height,
            } => point.x >= *x && point.x <= x + width && point.y >= *y && point.y <= y + height,
            RegionGeometry::Polygon { points } => {
                if points.len() < 3 {
                    return false;
                }
                let mut inside = false;
                let mut j = points.len() - 1;
                for i in 0..points.len() {
                    let (xi, yi) = (points[i][0], points[i][1]);
                    let (xj, yj) = (points[j][0], points[j][1]);
                    let straddles = (yi > point.y) != (yj > point.y);
                    if straddles {
                        let t = (point.y - yi) / (yj - yi);
                        if point.x < xi + t * (xj - xi) {
                            inside = !inside;
                        }
                    }
                    j = i;
                }
                inside
            }
        }
    }

    pub fn is_valid(&self) -> bool {
        match self {
            RegionGeometry::Rect { width, height, .. } => *width > 0.0 && *height > 0.0,
            RegionGeometry::Polygon { points } => points.len() >= 3,
        }
    }
}

/// Whether a move crossed *into* a region.
///
/// Entry is a transition, not a state: a token that was already inside and
/// moved a step has not entered anything. Getting this wrong means a region
/// fires on every frame of movement within it, which at the table reads as the
/// scene having a stutter rather than as a trigger misbehaving (FR-030).
pub fn entered(previous: Vec2, current: Vec2, geometry: &RegionGeometry) -> bool {
    !geometry.contains(previous) && geometry.contains(current)
}

/// Which of several regions a move entered, in a stable order.
///
/// `regions` is `(interactive_id, geometry)`. The result is sorted by id —
/// arbitrary, but *reproducible*, which is what a token crossing into two
/// overlapping regions at once needs. Undefined order would make that
/// crossing behave differently on different runs, and a GM debugging their own
/// scene would have nothing to hold onto.
pub fn entries_for<'a>(
    previous: Vec2,
    current: Vec2,
    regions: impl IntoIterator<Item = (&'a str, &'a RegionGeometry)>,
) -> Vec<&'a str> {
    let mut hit: Vec<&str> = regions
        .into_iter()
        .filter(|(_, geometry)| entered(previous, current, geometry))
        .map(|(id, _)| id)
        .collect();
    hit.sort_unstable();
    hit
}

// ---------------------------------------------------------------------------
// Authoring validation
// ---------------------------------------------------------------------------

/// An interactive as a Game Master is proposing it, before it is stored.
#[derive(Debug, Clone, PartialEq)]
pub struct InteractiveDraft {
    pub subject_kind: SubjectKind,
    /// The token for a prop, the wall for a door. None for a region.
    pub subject_ref: Option<String>,
    /// The area, for a region. None otherwise.
    pub geometry: Option<RegionGeometry>,
    /// None is legitimate: an interactive with no effect is scenery.
    pub effect_id: Option<String>,
    pub effect_config: serde_json::Value,
    pub trigger: Trigger,
    pub activation: Activation,
    pub fire_mode: FireMode,
}

/// Why a draft cannot be stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthoringError {
    /// A prop or door with no subject, or a region carrying one.
    SubjectShape { expected: SubjectKind },
    /// A region with no area, or a non-region carrying one.
    GeometryShape { expected: SubjectKind },
    /// An area that encloses nothing.
    DegenerateGeometry,
    /// `Trigger::Enter` on something that cannot be crossed.
    EnterNeedsRegion { subject_kind: SubjectKind },
    /// No contributor declares this effect.
    UnknownEffect { id: String },
    /// This effect does not attach to this sort of subject.
    WrongSubjectForEffect {
        id: String,
        subject_kind: SubjectKind,
    },
    /// A required field was not filled in.
    MissingConfigField { key: String },
    /// A field held something its declaration does not accept.
    InvalidConfigField { key: String },
    /// A field nothing declared.
    UnknownConfigField { key: String },
}

impl std::fmt::Display for AuthoringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthoringError::SubjectShape { expected } => write!(
                f,
                "a {} needs exactly the subject reference its kind implies",
                expected.as_str()
            ),
            AuthoringError::GeometryShape { expected } => write!(
                f,
                "a {} needs exactly the geometry its kind implies",
                expected.as_str()
            ),
            AuthoringError::DegenerateGeometry => {
                write!(f, "the region encloses no area")
            }
            AuthoringError::EnterNeedsRegion { subject_kind } => write!(
                f,
                "a {} cannot be crossed, so it cannot trigger on entry",
                subject_kind.as_str()
            ),
            AuthoringError::UnknownEffect { id } => {
                write!(f, "no contributor declares the effect `{id}`")
            }
            AuthoringError::WrongSubjectForEffect { id, subject_kind } => write!(
                f,
                "the effect `{id}` does not attach to a {}",
                subject_kind.as_str()
            ),
            AuthoringError::MissingConfigField { key } => {
                write!(f, "`{key}` is required")
            }
            AuthoringError::InvalidConfigField { key } => {
                write!(f, "`{key}` does not hold what it was declared to hold")
            }
            AuthoringError::UnknownConfigField { key } => {
                write!(f, "`{key}` is not a field of this effect")
            }
        }
    }
}

/// Check a draft against the shape rules and the effect's own declaration.
///
/// Returns *every* problem rather than the first, because a Game Master
/// filling in a form deserves to be told all of what is wrong at once rather
/// than one thing per attempt.
pub fn validate_draft(
    draft: &InteractiveDraft,
    registry: &EffectRegistry,
) -> Result<(), Vec<AuthoringError>> {
    let mut errors = Vec::new();

    let wants_subject = draft.subject_kind != SubjectKind::Region;
    if draft.subject_ref.is_some() != wants_subject {
        errors.push(AuthoringError::SubjectShape {
            expected: draft.subject_kind,
        });
    }
    if draft.geometry.is_some() == wants_subject {
        errors.push(AuthoringError::GeometryShape {
            expected: draft.subject_kind,
        });
    }
    if let Some(geometry) = &draft.geometry
        && !geometry.is_valid()
    {
        errors.push(AuthoringError::DegenerateGeometry);
    }

    if draft.trigger == Trigger::Enter && draft.subject_kind != SubjectKind::Region {
        errors.push(AuthoringError::EnterNeedsRegion {
            subject_kind: draft.subject_kind,
        });
    }

    if let Some(effect_id) = &draft.effect_id {
        match registry.get(effect_id) {
            None => errors.push(AuthoringError::UnknownEffect {
                id: effect_id.clone(),
            }),
            Some(declaration) => {
                if !declaration.subject_kinds.contains(&draft.subject_kind) {
                    errors.push(AuthoringError::WrongSubjectForEffect {
                        id: effect_id.clone(),
                        subject_kind: draft.subject_kind,
                    });
                }
                errors.extend(validate_config(declaration, &draft.effect_config));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Check submitted configuration against what an effect declared.
pub fn validate_config(
    declaration: &EffectDeclaration,
    config: &serde_json::Value,
) -> Vec<AuthoringError> {
    let mut errors = Vec::new();

    let Some(object) = config.as_object() else {
        // A null config is only acceptable when nothing is required.
        if config.is_null() {
            for field in &declaration.config {
                if field.required {
                    errors.push(AuthoringError::MissingConfigField {
                        key: field.key.clone(),
                    });
                }
            }
        } else {
            errors.push(AuthoringError::InvalidConfigField {
                key: String::from("<root>"),
            });
        }
        return errors;
    };

    for key in object.keys() {
        if declaration.field(key).is_none() {
            errors.push(AuthoringError::UnknownConfigField { key: key.clone() });
        }
    }

    for field in &declaration.config {
        let value = object.get(&field.key);
        let present = value.is_some_and(|v| !v.is_null());
        if !present {
            if field.required {
                errors.push(AuthoringError::MissingConfigField {
                    key: field.key.clone(),
                });
            }
            continue;
        }
        let value = value.expect("present");
        let ok = match &field.kind {
            ConfigFieldKind::Boolean => value.is_boolean(),
            ConfigFieldKind::Choice { options } => value
                .as_str()
                .is_some_and(|v| options.iter().any(|o| o.value == v)),
            ConfigFieldKind::Reference { .. } => value.as_str().is_some_and(|v| !v.is_empty()),
            ConfigFieldKind::ReferenceList { .. } => value.as_array().is_some_and(|items| {
                items
                    .iter()
                    .all(|i| i.as_str().is_some_and(|s| !s.is_empty()))
            }),
        };
        if !ok {
            errors.push(AuthoringError::InvalidConfigField {
                key: field.key.clone(),
            });
        }
    }

    errors
}

// ---------------------------------------------------------------------------
// Activation
// ---------------------------------------------------------------------------

/// Everything the decision depends on, gathered by the caller.
///
/// The server assembles this from storage and calls [`resolve_activation`].
/// Keeping the decision here rather than inline in a resolver is what lets the
/// truth table below actually be tested — the server's job is to gather facts
/// and obey the answer, not to re-derive it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivationContext {
    /// Whether the person activating runs the world.
    pub actor_is_gm: bool,
    /// Whether the interactive carries an effect at all.
    pub has_effect: bool,
    /// Whether that effect is in the current registry (FR-041).
    pub effect_available: bool,
    /// Whether the subject refuses state changes from players — a locked door.
    pub subject_locked: bool,
    pub activation: Activation,
    pub fire_mode: FireMode,
    /// Whether a `once` interactive has already fired.
    pub has_fired: bool,
}

/// Why an activation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub enum RefusalReason {
    /// Reserved to whoever runs the world.
    GmOnly,
    /// The door is locked.
    Locked,
    /// It fires once, and it has.
    AlreadyFired,
}

impl RefusalReason {
    pub fn as_str(self) -> &'static str {
        match self {
            RefusalReason::GmOnly => "gmOnly",
            RefusalReason::Locked => "locked",
            RefusalReason::AlreadyFired => "alreadyFired",
        }
    }
}

/// What happens when somebody activates an interactive.
///
/// A tagged outcome rather than a boolean, because "it did not run" covers
/// four genuinely different situations and a player told only "no" cannot tell
/// a locked door from a broken product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase", tag = "outcome")]
pub enum ActivationOutcome {
    /// The effect runs.
    Performed,
    /// A request was raised. Nothing has happened yet.
    Requested,
    /// Not permitted.
    Refused { reason: RefusalReason },
    /// The effect's subsystem is not in this build (FR-041). Not an error, and
    /// not a reason to delete anything.
    Unavailable,
    /// There is no effect. Legitimate scenery.
    NoEffect,
}

/// The truth table the server enforces.
///
/// # Order, and why it is this one
///
/// Scenery first: an interactive with no effect is not unavailable, absent, or
/// refused — there is simply nothing to run, and saying anything else would
/// make a GM think they had misconfigured something they had not.
///
/// Then availability, because an effect whose subsystem is missing cannot be
/// permitted or refused in any meaningful sense.
///
/// Then permission before fire state. "You may not" and "it has already
/// happened" can both be true, and telling a player they are not allowed is
/// more useful than telling them they are too late for something they were
/// never going to be allowed to do.
///
/// Then approval last among the permitted paths, because a locked door must
/// not queue a request. Queueing one would put a decision in front of the GM
/// that their own lock has already made.
///
/// A Game Master's own activation never queues, whatever the mode says — they
/// are the person the queue exists to ask.
pub fn resolve_activation(context: ActivationContext) -> ActivationOutcome {
    if !context.has_effect {
        return ActivationOutcome::NoEffect;
    }
    if !context.effect_available {
        return ActivationOutcome::Unavailable;
    }

    if context.activation == Activation::GmOnly && !context.actor_is_gm {
        return ActivationOutcome::Refused {
            reason: RefusalReason::GmOnly,
        };
    }
    if context.subject_locked && !context.actor_is_gm {
        return ActivationOutcome::Refused {
            reason: RefusalReason::Locked,
        };
    }
    if context.fire_mode == FireMode::Once && context.has_fired {
        return ActivationOutcome::Refused {
            reason: RefusalReason::AlreadyFired,
        };
    }
    if context.activation == Activation::RequiresApproval && !context.actor_is_gm {
        return ActivationOutcome::Requested;
    }

    ActivationOutcome::Performed
}

#[cfg(test)]
#[path = "interaction_tests.rs"]
mod tests;
