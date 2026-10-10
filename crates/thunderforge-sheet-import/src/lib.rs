//! Bringing a character in from a sheet (spec 048).
//!
//! A pack's reader turns a document into an [`ImportedCharacter`]: a
//! system-neutral character in which every leaf says how sure the reader is
//! and where on the page it came from. The mapping engine then plans that
//! reading onto an actor, from the pack's `sheetImport` declaration and its
//! optional refine hook. Nothing here names a game system.

pub mod character;
pub mod hash;
pub mod mapping;
pub mod plan;
pub mod reader;

pub use character::{
    Certainty, ClassLevel, ContentLink, Field, ImportedCharacter, Leaf, NamedContent, Note,
    ReaderStamp, SkillMark, Source,
};
pub use hash::{content_hash, normalise_name};
pub use mapping::{ContentTarget, MappingError, SheetMapping};
pub use plan::{
    ActorSnapshot, ContentChange, ContentIndex, Corrections, CurrentLink, FieldChange, ImportPlan,
    Indexed, PlanCertainty, RefineFn, RefineHook, Resolution, Unmapped, plan, plan_hash, plan_with,
};
pub use reader::{ReadError, Recognition, SheetReader};
