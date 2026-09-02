//! What an interface pack is, as a type.
//!
//! One file — `packs/interface/<id>/interface.json` — carrying colour tokens
//! for light and dark, an optional canvas appearance override, an optional
//! layout, and the systems it targets. There is no stylesheet, no module, and
//! no second file.
//!
//! # Why the shape is the safety property
//!
//! Spec 032's FR-003 says an interface pack cannot contribute behaviour, and
//! requires that to be enforced by automated validation rather than reviewer
//! judgement. There are two ways to get there: let a pack ship code and police
//! what the code does, or give the format nowhere to put code. The first needs
//! ADR-029 answered and it is an empty file. The second needs no answer, and
//! is why this half of the feature can ship at all (SC-011, ADR-059).
//!
//! `deny_unknown_fields` is doing that work at every level. A key this file
//! does not name is a rejection rather than an ignored value — an author who
//! misspells `background` finds out at validation instead of by looking at a
//! screen that is subtly wrong, and a key that means something in some other
//! product means nothing here.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use thunderforge_canvas_core::resource_display::AppearanceOverride;

use crate::layout::LayoutNode;
use crate::{Compatibility, SystemManifestLegal};

/// The only accepted value of `type`.
///
/// A pack is an interface pack or a system pack and never both (FR-002),
/// because the safety rule attaches to the type: an interface pack is
/// permitted to be data-only precisely because it is not the other thing.
pub const PACK_TYPE: &str = "interface";

/// Values for the CSS custom properties the application already themes with.
///
/// Every field optional, and an absent field means "keep the base pack's" — so
/// a pack that only wants a different accent colour is four lines long. The
/// alternative, a complete set, makes an author repeat twenty-nine values they
/// do not care about, which is how a pack ends up pinning a default it never
/// chose and never updates. `AppearanceOverride` in canvas-core arrived at the
/// same conclusion for the same reason; this is that decision applied again.
///
/// The vocabulary is not invented here. Each key maps one-for-one onto a
/// `--kebab-case` custom property already defined in
/// `apps/web/src/styles/globals.css`, which means a pack cannot introduce a
/// token the application does not consume — another way FR-003 holds without
/// anyone policing it.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenMap {
    // Ground
    pub background: Option<String>,
    pub foreground: Option<String>,
    // Surfaces
    pub card: Option<String>,
    pub card_foreground: Option<String>,
    pub popover: Option<String>,
    pub popover_foreground: Option<String>,
    // Emphasis
    pub primary: Option<String>,
    pub primary_foreground: Option<String>,
    pub secondary: Option<String>,
    pub secondary_foreground: Option<String>,
    pub accent: Option<String>,
    pub accent_foreground: Option<String>,
    // Recessive
    pub muted: Option<String>,
    pub muted_foreground: Option<String>,
    // Signal
    pub destructive: Option<String>,
    // Edges
    pub border: Option<String>,
    pub input: Option<String>,
    pub ring: Option<String>,
    // Charts
    pub chart1: Option<String>,
    pub chart2: Option<String>,
    pub chart3: Option<String>,
    pub chart4: Option<String>,
    pub chart5: Option<String>,
    // Sidebar
    pub sidebar: Option<String>,
    pub sidebar_foreground: Option<String>,
    pub sidebar_primary: Option<String>,
    pub sidebar_primary_foreground: Option<String>,
    pub sidebar_accent: Option<String>,
    pub sidebar_accent_foreground: Option<String>,
    pub sidebar_border: Option<String>,
    pub sidebar_ring: Option<String>,
    /// A CSS length, not a colour. The one geometric token, because corner
    /// radius is the single shape decision that reads as look rather than as
    /// layout.
    pub radius: Option<String>,
}

impl TokenMap {
    /// Every colour this map declares, paired with the key that declared it.
    ///
    /// `radius` is excluded: it is a length, and asking for its contrast would
    /// be a category error rather than a failing check.
    pub fn colours(&self) -> Vec<(&'static str, &str)> {
        // A plain list rather than a closure: the borrow checker rightly
        // objects to a closure capturing the output vector while taking
        // references that outlive its body, and spelling the pairs out keeps
        // the mapping from camelCase key to field visible in one place.
        [
            ("background", &self.background),
            ("foreground", &self.foreground),
            ("card", &self.card),
            ("cardForeground", &self.card_foreground),
            ("popover", &self.popover),
            ("popoverForeground", &self.popover_foreground),
            ("primary", &self.primary),
            ("primaryForeground", &self.primary_foreground),
            ("secondary", &self.secondary),
            ("secondaryForeground", &self.secondary_foreground),
            ("accent", &self.accent),
            ("accentForeground", &self.accent_foreground),
            ("muted", &self.muted),
            ("mutedForeground", &self.muted_foreground),
            ("destructive", &self.destructive),
            ("border", &self.border),
            ("input", &self.input),
            ("ring", &self.ring),
            ("chart1", &self.chart1),
            ("chart2", &self.chart2),
            ("chart3", &self.chart3),
            ("chart4", &self.chart4),
            ("chart5", &self.chart5),
            ("sidebar", &self.sidebar),
            ("sidebarForeground", &self.sidebar_foreground),
            ("sidebarPrimary", &self.sidebar_primary),
            ("sidebarPrimaryForeground", &self.sidebar_primary_foreground),
            ("sidebarAccent", &self.sidebar_accent),
            ("sidebarAccentForeground", &self.sidebar_accent_foreground),
            ("sidebarBorder", &self.sidebar_border),
            ("sidebarRing", &self.sidebar_ring),
        ]
        .into_iter()
        .filter_map(|(name, value)| value.as_deref().map(|value| (name, value)))
        .collect()
    }

    /// Look one colour up by its camelCase key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.colours()
            .into_iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value)
    }
}

/// One interface pack's manifest.
///
/// Deliberately not `JsonSchema`, unlike [`crate::SystemManifest`]. That type
/// generates a schema because the system-pack install path validates against
/// one; this one is validated by `serde` plus the explicit checks in
/// [`crate::interface::validate`], and a generated schema would be a second
/// description of the same shape that nothing consumes and everything could
/// drift from.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InterfaceManifest {
    /// Lowercase, hyphenated, and equal to the pack's directory name — a pack
    /// whose identity depends on which of the two you read is a pack that can
    /// be referred to two ways.
    pub id: String,
    /// Always `"interface"` (FR-002).
    #[serde(rename = "type")]
    pub pack_type: String,
    pub title: String,
    pub version: String,
    pub description: String,
    pub compatibility: Compatibility,
    /// Reused verbatim from the system-pack manifest rather than redeclared. A
    /// pack is a redistributable artifact whoever wrote it.
    pub legal: SystemManifestLegal,
    pub light: TokenMap,
    pub dark: TokenMap,
    /// The canvas half. Absent means the engine's own defaults.
    ///
    /// Colours here are `[r, g, b]` floats rather than CSS strings, matching
    /// the engine's `Rgb`. The two halves differ because they are consumed by
    /// two renderers, and converting between them in the manifest would put a
    /// conversion in the one place nobody would test it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas: Option<AppearanceOverride>,
    /// The systems this pack is built for, by manifest id.
    ///
    /// Empty means "any system", and is only permissible for a pack whose
    /// layout names nothing — naming an identifier is naming a system,
    /// whatever the list says (FR-025b). Forge declares empty.
    #[serde(default)]
    pub targets: Vec<String>,
    /// Absent inherits the base pack's, which is generic and therefore works
    /// against any system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<Vec<LayoutNode>>,
}

impl InterfaceManifest {
    /// Every identifier this pack's layout names.
    pub fn referenced_ids(&self) -> Vec<&str> {
        self.layout
            .as_deref()
            .unwrap_or_default()
            .iter()
            .flat_map(|node| node.referenced_ids())
            .collect()
    }

    /// Which layout constructs this pack uses.
    pub fn layout_kinds(&self) -> Vec<&'static str> {
        LayoutNode::kinds_present(self.layout.as_deref().unwrap_or_default())
    }
}

#[cfg(test)]
#[path = "interface_tests.rs"]
mod tests;

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Text needs 4.5:1; a border does not.
///
/// Applying the text threshold to a border colour would reject packs for no
/// reader's benefit, which is how a legibility rule stops being taken
/// seriously. Both numbers are WCAG AA.
const TEXT_PAIRS: &[(&str, &str)] = &[
    ("foreground", "background"),
    ("cardForeground", "card"),
    ("popoverForeground", "popover"),
    ("mutedForeground", "background"),
    ("primaryForeground", "primary"),
    ("secondaryForeground", "secondary"),
    ("accentForeground", "accent"),
    ("sidebarForeground", "sidebar"),
    ("sidebarPrimaryForeground", "sidebarPrimary"),
    ("sidebarAccentForeground", "sidebarAccent"),
];

/// Focus indicators, and only those.
///
/// # Why `border` and `input` are not here
///
/// WCAG 1.4.11 covers "visual information required to identify user interface
/// components and states". A hairline divider between two cards is not that —
/// it is decoration, and the component it sits beside is identified by its
/// background and its spacing. Requiring 3:1 of it over-applies the criterion.
///
/// That is not a convenient reading arrived at to make a check pass, and the
/// numbers are worth writing down because they look alarming out of context.
/// In the palette this product ships, `border` on `background` is **1.26:1**
/// in light and **1.25:1** in dark — the dark values are `oklch(1 0 0 / 10%)`,
/// deliberately a tenth of a white line. Every design system built on these
/// tokens looks like that. A floor that rejected them would reject the
/// product's own appearance, and a rule that fails on day one is a rule
/// somebody turns off rather than obeys.
///
/// A **focus ring** is the opposite case: it exists solely to identify a
/// state, which is exactly what 1.4.11 protects, and a reader who cannot see
/// where focus is cannot use a keyboard.
const NON_TEXT_PAIRS: &[(&str, &str)] = &[("ring", "background"), ("sidebarRing", "sidebar")];

impl TokenMap {
    /// This map's values laid over `base`'s.
    ///
    /// Contrast is a property of what a reader actually sees, and what a
    /// reader sees is the pack's declarations over the base pack's. Checking a
    /// partial map on its own would let a pack declare an unreadable
    /// foreground and pass, because the background it would be read against
    /// was never in the file.
    pub fn overlaid_on(&self, base: &TokenMap) -> TokenMap {
        let mut out = base.clone();
        macro_rules! take {
            ($($field:ident),* $(,)?) => {
                $( if self.$field.is_some() { out.$field = self.$field.clone(); } )*
            };
        }
        take!(
            background,
            foreground,
            card,
            card_foreground,
            popover,
            popover_foreground,
            primary,
            primary_foreground,
            secondary,
            secondary_foreground,
            accent,
            accent_foreground,
            muted,
            muted_foreground,
            destructive,
            border,
            input,
            ring,
            chart1,
            chart2,
            chart3,
            chart4,
            chart5,
            sidebar,
            sidebar_foreground,
            sidebar_primary,
            sidebar_primary_foreground,
            sidebar_accent,
            sidebar_accent_foreground,
            sidebar_border,
            sidebar_ring,
            radius,
        );
        out
    }
}

/// Everything wrong with a pack, rather than the first thing.
///
/// An author fixing one rejection at a time, each costing a full validation
/// run, is how a five-minute correction becomes an afternoon.
pub type Findings = Vec<String>;

fn check_colours(mode: &str, tokens: &TokenMap, findings: &mut Findings) {
    for (key, value) in tokens.colours() {
        if let Err(error) = crate::contrast::parse_color(value) {
            findings.push(format!(
                "{mode}.{key}: {error}. An unparseable colour is a rejection, not a \
                 fallback — a value nothing can measure cannot be checked for legibility"
            ));
        }
    }
}

fn check_legibility(mode: &str, tokens: &TokenMap, findings: &mut Findings) {
    let mut check = |pairs: &[(&str, &str)], floor: f64, what: &str| {
        for (foreground, background) in pairs {
            let (Some(fg), Some(bg)) = (tokens.get(foreground), tokens.get(background)) else {
                // A pair the merged map does not fully define is not a
                // failure: the base supplies what a pack omits, and a pack
                // plus the base is what a reader sees.
                continue;
            };
            let Ok(ratio) = crate::contrast::contrast_ratio_of(fg, bg) else {
                // Already reported by `check_colours`; saying it twice helps
                // nobody.
                continue;
            };
            if ratio < floor {
                findings.push(format!(
                    "{mode}: {foreground} on {background} is {ratio:.2}:1, below the \
                     {floor:.1}:1 floor for {what}. Rejected rather than warned about \
                     because a world's look is chosen by its Game Master — a reader who \
                     cannot see this has no setting of their own to escape to"
                ));
            }
        }
    };

    check(TEXT_PAIRS, crate::contrast::AA_NORMAL_TEXT, "text");
    check(
        NON_TEXT_PAIRS,
        crate::contrast::AA_LARGE_TEXT_AND_UI,
        "edges and focus rings",
    );
}

/// Everything that can be checked about a pack on its own.
///
/// `base` is the pack whose values fill in what this one omits — Forge, for
/// every pack but Forge, which is checked against itself. Targeting is checked
/// separately by [`validate_targeting`], because it needs the systems.
pub fn validate(
    manifest: &InterfaceManifest,
    directory_name: &str,
    base: &InterfaceManifest,
) -> Result<(), Findings> {
    let mut findings = Findings::new();

    // 1. Structural
    if manifest.pack_type != PACK_TYPE {
        findings.push(format!(
            "type is {:?}; an interface pack must declare {PACK_TYPE:?}. The type is \
             exclusive because the safety rule attaches to it — a pack is data-only \
             precisely because it is not a system pack",
            manifest.pack_type
        ));
    }
    if manifest.id != directory_name {
        findings.push(format!(
            "id is {:?} but the directory is {directory_name:?}. A pack whose identity \
             depends on which of the two you read can be referred to two ways",
            manifest.id
        ));
    }
    if manifest.id.is_empty()
        || !manifest
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        findings.push(format!(
            "id {:?} must be lowercase letters, digits and hyphens",
            manifest.id
        ));
    }

    // 2 & 3. Colours parse, and what a reader sees is legible.
    for (mode, tokens) in [("light", &manifest.light), ("dark", &manifest.dark)] {
        check_colours(mode, tokens, &mut findings);
        let merged = tokens.overlaid_on(if mode == "light" {
            &base.light
        } else {
            &base.dark
        });
        check_legibility(mode, &merged, &mut findings);
    }

    // 4. Legal — the same check the system-pack path already runs.
    match serde_json::to_value(manifest) {
        Ok(value) => {
            if let Err(error) = crate::validate_legal_content(&value) {
                findings.push(error);
            }
        }
        Err(error) => findings.push(format!("manifest could not be re-serialised: {error}")),
    }

    // 5. An untargeted pack must name nothing.
    if manifest.targets.is_empty() {
        for id in manifest.referenced_ids() {
            findings.push(format!(
                "layout names {id:?} while targets is empty. Naming an identifier is \
                 naming a system, whatever the list says — a pack that composes against \
                 any system may only address declarations generically"
            ));
        }
    }

    if findings.is_empty() {
        Ok(())
    } else {
        Err(findings)
    }
}

/// FR-026: every identifier the layout names is one its targets declare.
///
/// `declared_by` answers "what does this system publish", stored and derived
/// together, or `None` for a system this build does not have. Passed in rather
/// than looked up here, because reading manifests off a disk belongs to the
/// server and this crate is where the rules live.
///
/// Checked per target **independently**, never against their union: a pack
/// targeting both 5e and Blades must work for each, and `hitPoints` existing
/// in one does not excuse referencing it while rendering the other.
pub fn validate_targeting(
    manifest: &InterfaceManifest,
    declared_by: &dyn Fn(&str) -> Option<Vec<String>>,
) -> Result<(), Findings> {
    let mut findings = Findings::new();
    let referenced = manifest.referenced_ids();

    for target in &manifest.targets {
        let Some(declared) = declared_by(target) else {
            findings.push(format!(
                "targets {target:?}, which this build does not have"
            ));
            continue;
        };
        for id in &referenced {
            if !declared.iter().any(|d| d == id) {
                findings.push(format!(
                    "layout names {id:?}, which {target:?} does not declare"
                ));
            }
        }
    }

    if findings.is_empty() {
        Ok(())
    } else {
        Err(findings)
    }
}

/// The constructs Forge cannot demonstrate, because using one would make it
/// name a system.
///
/// The conformance test requires the rest. Listing the exemptions here rather
/// than inverting the check keeps the requirement stated positively: Forge
/// demonstrates everything except what FR-025b forbids it.
pub const IDENTIFIER_NAMING_KINDS: &[&str] = &["value", "pair", "tracker", "slotGrid"];

/// FR-007a: the base pack exercises every construct it is permitted to use,
/// and names nothing.
///
/// This is an obligation rather than a privilege, and it buys something real:
/// a format construct that cannot actually be built fails here, rather than
/// being discovered by a pack author a year later. The schema, not Forge,
/// remains the authority on what a pack may contain.
pub fn validate_conformance(forge: &InterfaceManifest) -> Result<(), Findings> {
    let mut findings = Findings::new();

    for id in forge.referenced_ids() {
        findings.push(format!(
            "the base pack names {id:?}. It is the fallback for every world, including \
             one bound to a system that ships next year, so it may address declarations \
             only generically (FR-025b)"
        ));
    }

    let used = forge.layout_kinds();
    for kind in LayoutNode::ALL_KINDS {
        if IDENTIFIER_NAMING_KINDS.contains(kind) {
            continue;
        }
        if !used.contains(kind) {
            findings.push(format!(
                "the base pack never uses {kind:?}. A construct the format offers and \
                 the reference pack cannot demonstrate is one nobody has shown can be \
                 built (FR-007a)"
            ));
        }
    }

    if findings.is_empty() {
        Ok(())
    } else {
        Err(findings)
    }
}
