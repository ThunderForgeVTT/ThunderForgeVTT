# Contract: the PDF region API, the neutral character, and the pack slot

## `thunderforge-pdf`: additions (game-agnostic, FR-001a)

```rust
// lib.rs
pub struct Limits { pub max_bytes: usize, pub max_pages: u32 }
impl Document {
    pub fn from_bytes_bounded(bytes: &[u8], limits: Limits) -> Result<Document, PdfError>;
}
pub enum PdfError {
    Unreadable(String),
    Page { page: u32, reason: String },
    Encrypted,                                  // new: lopdf is_encrypted()
    TooLarge { bytes: usize, limit: usize },    // new: checked before parsing
    TooManyPages { pages: u32, limit: u32 },    // new: checked after the page tree, before content
}

// region.rs (new)
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect { pub x0: f32, pub y0: f32, pub x1: f32, pub y1: f32 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PositionedLine { pub text: String, pub rect: Rect, pub size: f32, pub bold: bool, pub italic: bool }

pub struct PageText { pub number: u32, pub geometry: PageGeometry, pub lines: Vec<PositionedLine> }

impl PageText {
    pub fn read(doc: &Document, page: u32) -> Result<PageText, PdfError>;
    /// Every place `label` appears, matched on `layout::normalise`d text.
    pub fn find(&self, label: &str) -> Vec<Rect>;
    pub fn within(&self, area: Rect) -> Vec<&PositionedLine>;
    /// Lines whose vertical centre falls in `of`'s band, left edge ≥ of.x1, up to `max_dx`.
    pub fn right_of(&self, of: Rect, max_dx: f32) -> Vec<&PositionedLine>;
    /// Lines whose horizontal span overlaps `of`'s, top below of.y0, up to `max_dy`.
    pub fn below(&self, of: Rect, max_dy: f32) -> Vec<&PositionedLine>;
}
```

The lines come from the same `lines()` grouping, with `SAME_LINE = 0.4`, so a
sheet and a book agree on what a line is. The wasm module gains
`read_page_text(bytes, page) -> JSON`. The existing `read_pdf` output is
unchanged.

**Tests**: `region_tests.rs` uses synthetic runs, the way `layout_tests.rs`
does. It covers a label found twice, two lines at one y, a right_of band
that excludes the next column, below within a box, and rotated pages
refused as `Page{reason}`.

## `thunderforge-sheet-import` (new crate, names no system)

```rust
pub enum Certainty { Read, Uncertain { reason: String }, Unread }
pub struct Source { pub page: u32, pub rect: Rect, pub text: String }
pub struct Field<T> { pub value: Option<T>, pub certainty: Certainty, pub source: Option<Source> }

/// FR-005: a system-neutral character. Every leaf is a Field.
pub struct ImportedCharacter {
    pub identity: Identity,                  // name, player_name (ignored for auth), species, background, alignment, size, xp
    pub classes: Vec<ClassLevel>,            // name, subclass, level, hit_die
    pub abilities: BTreeMap<String, Field<i32>>,   // neutral keys: "str" "dex" ...
    pub proficiencies: Proficiencies,        // skills{key -> none|proficient|expertise|half}, saves, armour, weapons, tools, languages
    pub defences: Defences,                  // armour_class, resistances, immunities, vulnerabilities, condition_immunities
    pub resources: Resources,                // hp_max, hp_current, hp_temp, hit_dice[{die,total,used}], death_saves, coins, inspiration
    pub movement: Movement,                  // speeds{walk,fly,swim,climb,burrow}, senses{darkvision,...}
    pub spellcasting: Vec<Spellcasting>,     // class, ability, save_dc, attack_bonus; slots; pact slots
    pub content: Vec<NamedContent>,          // kind, name, fields, link{prepared,granted_by,uses,recharge,equipped,attuned,quantity}
    pub derived: BTreeMap<String, Field<i32>>, // what the sheet printed and the system may derive: "skill.perception", "passive.perception", "initiative" ...
    pub persona: Persona,                    // appearance + personality + backstory
    pub notes: Vec<Field<String>>,           // anything read that has no neutral home
}

pub trait SheetReader: Sync {
    fn id(&self) -> &'static str;            // "ddb-pdf"
    fn version(&self) -> &'static str;
    fn recognise(&self, doc: &Document) -> Recognition;  // Yes | No{reason}
    fn read(&self, doc: &Document) -> Result<ImportedCharacter, ReadError>;
}

pub enum ReadError { Pdf(PdfError), NotRecognised(String), Page { page: u32, reason: String } }

/// The mapping engine: shared, data-driven, with the pack's refine hook last.
pub fn plan(
    mapping: &SheetMapping,          // parsed from system.json `sheetImport`
    refine: Option<RefineFn>,
    reading: &ImportedCharacter,
    corrections: &Corrections,
    current: &ActorSnapshot,         // sheet JSON per data type + links
    world: &dyn ContentIndex,        // world + staged lookup by (kind, normalised name)
) -> ImportPlan;

pub fn plan_hash(plan: &ImportPlan) -> String;  // sha256 of canonical JSON
pub fn normalise_name(name: &str) -> String;     // casefold, strip punctuation and "(…)" source tags
pub fn content_hash(kind: &str, fields: &serde_json::Value) -> String;
```

**Invariants** (each one is a test in the crate):

- `plan` is pure and deterministic. The same inputs always give the same
  plan, byte for byte.
- A field that is `Unread` and has no correction is never written. Its
  current value is kept.
- An `Uncertain` field is written only if it has a correction, or if the
  person leaves it as read. In the second case the plan marks it
  `certainty: UNCERTAIN` and the review shows it (SC-003).
- A `derived` value is never written. If the mapping's derivation
  disagrees with the sheet, the plan gains a `crossChecks` row and the
  matching base field becomes `Uncertain`.
- Every reading leaf ends up in exactly one of: a field change, a content
  change, `unmapped`, or ignored as identical. Nothing is dropped (FR-013).
- `playState` targets are never changed on a re-import unless
  `overwritePlayState` names them.

## Canvas core: the `SheetImport` slot

`crates/thunderforge-canvas-core/src/system_contribution.rs` gains:

```rust
pub struct SheetImport {
    pub readers: &'static [&'static (dyn SheetReaderHandle)],   // native readers the server runs
    pub refine: Option<fn(&ImportedCharacterJson, &mut ImportPlanJson)>,
}
pub struct SystemContribution { /* … */ pub sheet_import: Option<&'static SheetImport> }
```

`SheetReaderHandle` is an object-safe wrapper that canvas-core defines over
bytes in and JSON out. This keeps canvas-core free of a `thunderforge-pdf`
dependency, so packs that never import a sheet do not compile a PDF parser.

The server looks up the contribution by the actor's `game_system_id`. No
contribution, or no `sheetImport` block in `system.json`, means
`SYSTEM_HAS_NO_MAPPING`.

## Web: how the host finds a reader

- The pack's web package exports `sheetReader: () => import("@thunderforge/sheet-dnd5e")`
  beside its sheet. `apps/web/src/pages/world/actor/systemActorSheets.ts`
  already globs the packs' exports, and a sibling `systemSheetReaders.ts`
  does the same for `sheetReader`.
- The host page calls `readSheet(bytes)` and receives
  `{ recognised, reading | error }`.
- The host never names `dnd5e` (`scripts/check-system-registry.mjs`).
