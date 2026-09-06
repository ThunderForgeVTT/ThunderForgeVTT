//! What a token's resources are, and how much of that a given viewer is told.
//!
//! Spec 029. The rules for bars and counters — health, stamina, mana, or
//! whatever the active game system declares — live here rather than in the
//! engine crate for the usual reason: the engine's tests compile and never
//! run, so a rule placed there is untested by construction. These execute.
//!
//! # A resource is a list of entries, not a current-and-maximum pair
//!
//! The obvious model is `{ current, max }`, and it immediately raises a
//! question it cannot answer: what does a value above the maximum mean?
//! Temporary hit points, a shield, the second stage of a boss — all real, all
//! expressible only as "more than full", which then needs a rule about
//! clamping that will be wrong for at least one of them.
//!
//! [`ResourceEntry`] removes the question. Overflow is not a value exceeding
//! a bound; it is a further entry. A boss with three stages is three entries.
//! A shield is an entry stacked above the base pool. Damage takes the topmost
//! first. There is no state in which a value exceeds its maximum, so nothing
//! has to decide what to do about one.
//!
//! # Disclosure is part of the model, not a filter over it
//!
//! A bar is a disclosure channel: a player watching a boss's health bar learns
//! something whether or not anybody meant them to. So what a viewer is told is
//! decided here and applied on the server — see [`Disclosed`] — and the client
//! receives only the shape its state permits. A client that never receives a
//! figure cannot leak one.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How a resource is presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub enum ResourceKind {
    /// Has a maximum, and is drawn as a proportion of it.
    Bar,
    /// Has no maximum. A count, drawn as a number.
    Counter,
}

/// What a game system declares it tracks.
///
/// The engine holds no built-in notion of "health": one system tracks hit
/// points, another health/stamina/mana, a third health/energy. Hard-coding the
/// first would make every system after it a special case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub struct ResourceDefinition {
    pub id: String,
    pub label: String,
    pub kind: ResourceKind,
    /// Display order. The engine imposes none.
    pub order: i32,
    /// Whether more than one entry is permitted.
    pub allow_stacking: bool,
}

/// One layer of a resource: a pool with its own maximum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub struct ResourceEntry {
    pub current: i32,
    /// Absent for a counter, which has no maximum to be a proportion of.
    pub max: Option<i32>,
    /// Optional name for this layer — "Shield", "Stage 2".
    pub label: Option<String>,
}

/// Where one entry's numbers come from in a system's stored actor data.
///
/// The server reads a system's JSONB slot and pulls the named fields. It never
/// learns what "health" means — only that this resource's first entry takes
/// its current from `current_hp` and its maximum from `max_hp`.
///
/// That indirection is the whole point of FR-001. Without it, every new game
/// system would need server changes to be displayed, and the engine would
/// accumulate one special case per ruleset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub struct EntrySource {
    /// Field holding this entry's current value.
    pub current: String,
    /// Field holding its maximum. Absent for a counter, or for a layer whose
    /// size is whatever was granted — temporary hit points have no maximum of
    /// their own.
    pub max: Option<String>,
    /// A maximum fixed by the rules rather than stored per character.
    ///
    /// Blades in the Dark caps stress at nine and trauma at four; neither is
    /// written into a character's data because neither varies. Without this,
    /// such a pool could only be shown as a bare count — losing the thing a
    /// player most needs to see, which is how close to the cap they are.
    ///
    /// `max` wins when both are given: a stored value is about *this*
    /// character, and a literal is about everyone.
    pub max_value: Option<i32>,
    /// Name for this layer, shown when there is more than one.
    pub label: Option<String>,
    /// Skip this entry when the field is missing or zero.
    ///
    /// Temporary hit points are usually absent, and an ever-present empty
    /// "Temporary" layer would be visual noise on every character in the game.
    #[serde(default)]
    pub optional: bool,
}

/// Where a whole resource's entries come from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub struct ResourceSource {
    /// Which stored slot to read: `resourceData`, `traitData`, and so on.
    pub slot: String,
    /// Ordered. Index 0 is the base pool; later entries stack above it.
    pub entries: Vec<EntrySource>,
}

/// Build a resource's entries from a system's stored actor data.
///
/// `slot` is the decoded JSON for the column named by [`ResourceSource::slot`].
/// A field that is absent, non-numeric, or zero on an optional entry yields no
/// entry rather than a zeroed one — see [`EntrySource::optional`].
pub fn entries_from(slot: &serde_json::Value, source: &ResourceSource) -> Vec<ResourceEntry> {
    let read = |name: &str| -> Option<i32> {
        slot.get(name)
            .and_then(|v| v.as_i64())
            .and_then(|n| i32::try_from(n).ok())
    };

    let mut built = Vec::new();
    for entry in &source.entries {
        let Some(current) = read(&entry.current) else {
            continue;
        };
        if entry.optional && current == 0 {
            continue;
        }
        built.push(ResourceEntry {
            current,
            max: entry.max.as_deref().and_then(read).or(entry.max_value),
            label: entry.label.clone(),
        });
    }
    built
}

/// Why a set of entries could not be accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryError {
    /// More than one entry where the definition forbids stacking.
    StackingNotAllowed { got: usize },
    /// A current value outside `0..=max`.
    ///
    /// Not a state to clamp. Because overflow is a further entry, a value
    /// above its own entry's maximum cannot arise from ordinary play, so it
    /// means something upstream is wrong and should say so.
    ValueOutOfRange {
        index: usize,
        current: i32,
        max: i32,
    },
}

/// Check a set of entries against its definition.
pub fn validate_entries(
    definition: &ResourceDefinition,
    entries: &[ResourceEntry],
) -> Result<(), EntryError> {
    if !definition.allow_stacking && entries.len() > 1 {
        return Err(EntryError::StackingNotAllowed { got: entries.len() });
    }

    for (index, entry) in entries.iter().enumerate() {
        if let Some(max) = entry.max
            && (entry.current < 0 || entry.current > max)
        {
            return Err(EntryError::ValueOutOfRange {
                index,
                current: entry.current,
                max,
            });
        }
    }

    Ok(())
}

/// Apply `amount` of depletion, consuming the topmost entry first.
///
/// Spent entries stay in the list at zero rather than being removed: a boss on
/// its last stage should still read as being on its *last* stage, and that
/// needs the exhausted ones to remain visible.
///
/// Returns any amount that could not be absorbed.
pub fn deplete(entries: &mut [ResourceEntry], mut amount: i32) -> i32 {
    for entry in entries.iter_mut().rev() {
        if amount <= 0 {
            break;
        }
        let taken = amount.min(entry.current.max(0));
        entry.current -= taken;
        amount -= taken;
    }
    amount
}

/// Total current across every entry.
pub fn total_current(entries: &[ResourceEntry]) -> i32 {
    entries.iter().map(|e| e.current).sum()
}

/// Total maximum across every entry that has one.
pub fn total_max(entries: &[ResourceEntry]) -> i32 {
    entries.iter().filter_map(|e| e.max).sum()
}

/// What a viewer other than the Game Master is permitted to learn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(rename_all = "camelCase")]
pub enum DisclosureState {
    /// The exact entries.
    Visible,
    /// That the resource exists, and nothing else.
    ///
    /// The honest form of "hidden": removing the bar entirely also discloses
    /// something, because a token conspicuously lacking a bar every other
    /// token has is itself a signal.
    Greyed,
    /// A proportion, with no maximum.
    ///
    /// Discloses more than it appears to. A viewer who knows the damage they
    /// dealt can divide it by the change, recover the maximum, and read exact
    /// values from then on. Offered because a readable boss fight is a
    /// legitimate thing to want — but not equivalent in safety to
    /// [`DisclosureState::Chunked`], which rarely moves on a single hit.
    Percentage,
    /// The proportion rounded down to quarters.
    Chunked,
}

/// A resource as one viewer receives it.
///
/// Tagged on `disclosure`, so the shape carries exactly the one field its
/// state permits and no other. An over-disclosing payload is unrepresentable
/// rather than forbidden by a rule somebody has to remember — and on the
/// TypeScript side this generates a discriminated union that narrows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
#[serde(tag = "disclosure", rename_all = "camelCase")]
pub enum Disclosed {
    Visible { entries: Vec<ResourceEntry> },
    Greyed,
    Percentage { proportion: f32 },
    Chunked { quarter: u8 },
}

/// Fraction of the pool remaining, 0.0–1.0.
///
/// A resource with no maximum anywhere — a pure counter — has no proportion to
/// report and answers `None`.
pub fn proportion(entries: &[ResourceEntry]) -> Option<f32> {
    let max = total_max(entries);
    if max <= 0 {
        return None;
    }
    let current = total_current(entries).clamp(0, max);
    Some(current as f32 / max as f32)
}

/// Which quarter the pool sits in: 0 (empty) through 4 (full).
///
/// Rounds **down**, so anything short of full reads as less than full and only
/// a genuinely empty pool reads as empty. Rounding to nearest would show a
/// creature at 88% as "full" and one at 12% as "empty", both of which are lies
/// a player would act on.
pub fn quarter(entries: &[ResourceEntry]) -> Option<u8> {
    let fraction = proportion(entries)?;
    Some(match fraction {
        f if f <= 0.0 => 0,
        f if f >= 1.0 => 4,
        f => (f * 4.0).floor() as u8,
    })
}

/// What a token is, as far as disclosure is concerned.
///
/// Derived from the actor behind the token rather than configured on it —
/// see [`default_disclosure`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSubject {
    /// A player character belonging to the person looking at it.
    OwnCharacter,
    /// A player character belonging to somebody else at the table.
    PartyCharacter,
    /// Anyone the Game Master runs.
    NonPlayerCharacter,
}

/// How much a token discloses when nobody has said otherwise.
///
/// **There is no world-level default setting**, and that is the design rather
/// than an omission. A token is bound to an actor, and the actor already
/// records what it is; deriving the answer from data that exists beats a
/// setting somebody has to discover, because a table that never finds the
/// setting plays under whatever we guessed, while a derived default is
/// correct for a table that configures nothing — which is most tables.
///
/// - Your own character is exact. You always know your own hit points.
/// - Another player's character is exact. A party shares this at a table.
/// - An NPC is chunked: readable enough to play — "that ogre is nearly dead" —
///   without handing out figures the Game Master is entitled to keep.
///
/// An explicit per-token override still wins. This is the floor, not a
/// ceiling, and a Game Master who wants a boss fully visible or fully greyed
/// says so and is obeyed.
pub fn default_disclosure(subject: TokenSubject) -> DisclosureState {
    match subject {
        TokenSubject::OwnCharacter | TokenSubject::PartyCharacter => DisclosureState::Visible,
        TokenSubject::NonPlayerCharacter => DisclosureState::Chunked,
    }
}

/// Reduce a set of entries to what `state` permits a viewer to see.
///
/// This is the function the server calls. Everything it returns is safe to put
/// on the wire; everything it drops never leaves the server.
pub fn disclose(entries: &[ResourceEntry], state: DisclosureState) -> Disclosed {
    match state {
        DisclosureState::Visible => Disclosed::Visible {
            entries: entries.to_vec(),
        },
        DisclosureState::Greyed => Disclosed::Greyed,
        DisclosureState::Percentage => Disclosed::Percentage {
            proportion: proportion(entries).unwrap_or(0.0),
        },
        DisclosureState::Chunked => Disclosed::Chunked {
            quarter: quarter(entries).unwrap_or(0),
        },
    }
}

/// A colour as linear-ish sRGB components in 0.0–1.0, matching `token_kind`.
pub type Rgb = (f32, f32, f32);

/// Everything about how a status display *looks*, in one place.
///
/// FR-022 says the engine must not compile these in, and FR-023 says the
/// documented default set exists exactly once. Both point at the same
/// structure: the engine reads values from here and the application may
/// replace any of them, rather than each drawing site carrying its own
/// constant that nobody can find later.
///
/// # Why the palette is indexed by order and not by name
///
/// Nothing here knows what "health" means, and it must not — FR-001 is that
/// the engine renders what a system declares and understands none of it. A
/// palette keyed by resource id would smuggle that knowledge back in and
/// would silently fall back to grey for every system that names things
/// differently, which is most of them.
///
/// So the Nth resource a system declares gets the Nth colour. That makes the
/// colour a property of the *declaration order* a system author controls,
/// which is the only thing available that is both stable and meaningful.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayAppearance {
    /// The unfilled part of a bar, drawn behind the fill.
    pub track: Rgb,
    /// Opacity of the track, so a bar sits over artwork without hiding it.
    pub track_alpha: f32,
    /// Fill for a resource whose real value the viewer is not being told.
    ///
    /// Deliberately desaturated and mid-grey: a coarsened bar should read as
    /// "not telling you" rather than as a fifth resource type.
    pub undisclosed: Rgb,
    /// Fill colours, taken in declaration order and wrapped if a system
    /// declares more resources than there are slots.
    pub palette: Vec<Rgb>,
    /// Height of one bar, in world units.
    pub bar_height: f32,
    /// Vertical gap between stacked bars.
    pub bar_gap: f32,
    /// Distance from the token's centre to the first bar.
    pub first_bar_offset: f32,
}

/// How many distinct fills the default palette offers before wrapping.
///
/// Four, matching the token-kind palette, and for the same reason: past
/// roughly this many, a set cannot keep every pair separated in lightness as
/// well as hue, and a palette that promises a distinction it cannot deliver
/// is worse than one that repeats honestly.
pub const DEFAULT_PALETTE_LEN: usize = 4;

impl Default for DisplayAppearance {
    fn default() -> Self {
        Self {
            // Near-black, so the track reads as absence rather than as a
            // colour competing with the fill.
            track: (0.06, 0.07, 0.09),
            track_alpha: 0.78,
            undisclosed: (0.42, 0.45, 0.50),
            palette: vec![
                // Deep red. The first resource a system declares is the one
                // it considers most urgent, and red is where the eye goes.
                (0.784, 0.208, 0.216),
                // Blue, and much lighter — so the first pair, which is the
                // pair most tokens actually show, separates by lightness
                // before hue is considered at all.
                (0.282, 0.565, 0.996),
                // Green, lighter still. Far from the red in hue and from the
                // blue in luma, which is the harder of the two constraints.
                (0.463, 0.827, 0.427),
                // Violet, and the darkest — by a clear margin, not a narrow
                // one. Deliberately not amber: amber sits close to the green
                // in perceived lightness. The first violet tried here was
                // lighter and failed the separation test against the red by
                // 0.007, which is exactly the kind of near-miss that gets
                // eyeballed as fine and is not.
                (0.35, 0.16, 0.60),
            ],
            bar_height: 10.0,
            bar_gap: 3.0,
            first_bar_offset: 8.0,
        }
    }
}

impl DisplayAppearance {
    /// The fill for the resource at `order`, wrapping rather than running out.
    ///
    /// Wrapping is the honest failure: a system declaring six resources gets
    /// two repeated colours, which a viewer can still read positionally.
    /// Returning grey past the fourth would make the fifth and sixth
    /// indistinguishable from a resource that is being withheld, which means
    /// something entirely different.
    pub fn fill_for(&self, order: usize) -> Rgb {
        if self.palette.is_empty() {
            return self.undisclosed;
        }
        self.palette[order % self.palette.len()]
    }
}

/// Perceived lightness of a colour, 0.0–1.0.
///
/// Rec. 709 luma, matching `token_kind::TokenKind::luma`. A plain channel
/// average calls amber and slate equally light and they are nothing alike.
pub fn luma((r, g, b): Rgb) -> f32 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// A partial appearance, as the application sends it.
///
/// Every field optional and every absent field meaning "leave this alone".
/// The alternative — a full appearance — makes the application responsible
/// for repeating values it does not care about, which is how a caller ends
/// up pinning a default it never chose and never updates.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppearanceOverride {
    pub track: Option<Rgb>,
    pub track_alpha: Option<f32>,
    pub undisclosed: Option<Rgb>,
    pub palette: Option<Vec<Rgb>>,
    pub bar_height: Option<f32>,
    pub bar_gap: Option<f32>,
    pub first_bar_offset: Option<f32>,
}

impl AppearanceOverride {
    /// Fold this override onto an existing appearance.
    pub fn apply_to(&self, base: &mut DisplayAppearance) {
        if let Some(track) = self.track {
            base.track = track;
        }
        if let Some(alpha) = self.track_alpha {
            base.track_alpha = alpha;
        }
        if let Some(undisclosed) = self.undisclosed {
            base.undisclosed = undisclosed;
        }
        if let Some(palette) = &self.palette {
            base.palette = palette.clone();
        }
        if let Some(height) = self.bar_height {
            base.bar_height = height;
        }
        if let Some(gap) = self.bar_gap {
            base.bar_gap = gap;
        }
        if let Some(offset) = self.first_bar_offset {
            base.first_bar_offset = offset;
        }
    }
}

/// How much a value is being trusted to the viewer, for drawing purposes.
///
/// Three states rather than a boolean, because there are three genuinely
/// different things to say and a boolean can only carry two. An exact figure
/// and an estimate are both *values*, so a boolean groups them — and then a
/// bar showing "somewhere in the second quarter" is drawn identically to one
/// showing 47 of 90, which is FR-014's failure exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    /// The real figure.
    Exact,
    /// A band or a proportion — real information, but not a reading.
    Coarse,
    /// No value at all.
    Withheld,
}

/// The fill a bar is drawn with, given its resource colour and how precise the
/// figure behind it is.
///
/// A coarse bar keeps enough of its resource colour to stay identifiable —
/// which bar this is remains legible — while sitting visibly closer to the
/// withheld grey, so it does not read as a measurement. Blending rather than a
/// separate palette because a second palette would need the same
/// pair-separation guarantees as the first, and would then have to stay in
/// step with it for ever.
pub fn fill_for_precision(base: Rgb, undisclosed: Rgb, precision: Precision) -> Rgb {
    /// How far a coarse fill sits toward the withheld colour.
    ///
    /// Far enough to be unmistakable side by side, not so far that two
    /// coarse bars of different resources become the same bar.
    const COARSE_BLEND: f32 = 0.45;

    match precision {
        Precision::Exact => base,
        Precision::Withheld => undisclosed,
        Precision::Coarse => (
            base.0 + (undisclosed.0 - base.0) * COARSE_BLEND,
            base.1 + (undisclosed.1 - base.1) * COARSE_BLEND,
            base.2 + (undisclosed.2 - base.2) * COARSE_BLEND,
        ),
    }
}

/// How full a bar is drawn, and how precise the figure behind it is.
///
/// # Why this cannot leak a withheld value (FR-016)
///
/// Not by discipline, but by construction. `Disclosed` is a tagged union whose
/// coarse variants *do not carry* the exact figure: `Greyed` holds nothing at
/// all, `Chunked` holds only a quarter index, `Percentage` only a proportion.
/// So there is no exact value in scope here to leak into a width, an order or
/// a size — the audit FR-016 asks for is answered by the type rather than by
/// reading the renderer and hoping.
///
/// That is the argument for coarsening on the server and shipping the reduced
/// form, rather than shipping the figure with a flag saying how much to show.
pub fn bar_fill(disclosed: &Disclosed) -> (f32, Precision) {
    match disclosed {
        Disclosed::Visible { entries } => (proportion(entries).unwrap_or(0.0), Precision::Exact),
        // Coarse, not exact. Both carry real information and neither is a
        // reading, and drawing them like one is FR-014's failure: a player
        // cannot tell an estimate from a measurement, so they act on the
        // estimate as though it were one.
        Disclosed::Percentage { proportion } => (proportion.clamp(0.0, 1.0), Precision::Coarse),
        // A quarter index is drawn at the *bottom* of its band, so a token in
        // the 1-4 band never looks half full. Reading a coarse bar as more
        // precise than it is would defeat the point of coarsening it.
        Disclosed::Chunked { quarter } => {
            ((*quarter as f32 / 4.0).clamp(0.0, 1.0), Precision::Coarse)
        }
        // Full regardless of the real figure, which is the point: a withheld
        // bar whose width varied would leak the value it is withholding.
        Disclosed::Greyed => (1.0, Precision::Withheld),
    }
}

#[cfg(test)]
#[path = "resource_display_tests.rs"]
mod tests;
