//! Bringing a Roll for Shoes character in from the one-page ThunderForge
//! sheet (spec 048 US4).
//!
//! The declaration under `sheetImport` in `system.json` lands the name and
//! the XP. This module is the rest:
//!
//! * the slot the host runs: the `tf-rfs-pdf` reader over bytes, with each
//!   refusal named by its error code;
//! * the refine hook, which turns the sheet's skill rows into the lineage
//!   `trait_data.skills` holds. A skill is the character's, not a piece of
//!   the world's compendium, so nothing is staged for the GM.
//!
//! The lineage is checked here so an apply is never refused by the pack's
//! own validator: a row whose "grew from" names no row, or a row that is not
//! exactly one level above the one it grew from, becomes a root of its own,
//! and the change is uncertain with the reason.

use serde_json::{json, Value};
use std::collections::BTreeSet;
use thunderforge_canvas_core::sheet_import::{SheetImport, SheetReadFailure, SheetReaderHandle};
use thunderforge_pdf::{Document, Limits};
use thunderforge_sheet_import::{
    ActorSnapshot, Certainty, FieldChange, ImportPlan, ImportedCharacter, PlanCertainty, ReadError,
    SheetReader,
};
use thunderforge_system_roll_for_shoes_sheet::RfsPdf;

/// What this pack contributes to bringing a character in.
pub static SHEET_IMPORT: SheetImport = SheetImport {
    readers: &[&RFS_PDF],
    refine: Some(refine_json),
};

/// Where the skills land.
pub const SKILLS: &str = "trait_data.skills";

struct RfsPdfHandle;

static RFS_PDF: RfsPdfHandle = RfsPdfHandle;

impl SheetReaderHandle for RfsPdfHandle {
    fn id(&self) -> &'static str {
        RfsPdf.id()
    }

    fn version(&self) -> &'static str {
        RfsPdf.version()
    }

    fn read(&self, bytes: &[u8]) -> Result<String, SheetReadFailure> {
        let doc = Document::from_bytes_bounded(bytes, Limits::default())
            .map_err(|error| failure(ReadError::Pdf(error)))?;
        let reading = RfsPdf.read(&doc).map_err(failure)?;
        serde_json::to_string(&reading).map_err(|e| SheetReadFailure {
            code: "SHEET_UNREADABLE",
            message: format!("The reading could not be written: {e}."),
        })
    }
}

fn failure(error: ReadError) -> SheetReadFailure {
    SheetReadFailure {
        code: error.code(),
        message: error.sentence(),
    }
}

/// The hook as the host calls it, over JSON.
pub fn refine_json(reading: &Value, current: &Value, plan: &mut Value) {
    let parsed = (
        serde_json::from_value::<ImportedCharacter>(reading.clone()),
        serde_json::from_value::<ActorSnapshot>(current.clone()),
        serde_json::from_value::<ImportPlan>(plan.clone()),
    );
    if let (Ok(reading), Ok(current), Ok(mut refined)) = parsed {
        refine(&reading, &current, &mut refined);
        if let Ok(value) = serde_json::to_value(&refined) {
            *plan = value;
        }
    }
}

/// One skill row, as the sheet printed it.
struct Row {
    row: u64,
    name: String,
    level: u64,
    grew_from: Option<u64>,
    parent: Option<usize>,
}

/// Roll for Shoes' last word on a plan: the skill rows become the lineage.
pub fn refine(reading: &ImportedCharacter, current: &ActorSnapshot, plan: &mut ImportPlan) {
    plan.content.retain(|change| change.kind != "skill");
    let mut doubts: Vec<String> = Vec::new();
    let mut source = None;
    let mut rows: Vec<Row> = Vec::new();
    for content in reading.content.iter().filter(|c| c.kind == "skill") {
        let Some(name) = content.name.value.clone() else {
            continue;
        };
        if let Certainty::Uncertain { reason } = &content.name.certainty {
            doubts.push(format!("{name}: {}", reason.trim_end_matches('.')));
        }
        source = source.or_else(|| content.name.source.clone());
        let number = |key: &str| content.fields.get(key).and_then(Value::as_u64);
        rows.push(Row {
            row: number("row").unwrap_or(rows.len() as u64 + 1),
            name,
            level: number("level").filter(|l| *l >= 1).unwrap_or(1),
            grew_from: number("grew_from"),
            parent: None,
        });
    }
    if rows.is_empty() {
        return;
    }
    for index in 0..rows.len() {
        let Some(from) = rows[index].grew_from else {
            continue;
        };
        let parent = rows
            .iter()
            .position(|r| r.row == from && r.row != rows[index].row);
        match parent {
            Some(p) if rows[p].level + 1 == rows[index].level => rows[index].parent = Some(p),
            Some(p) => doubts.push(format!(
                "{} is level {}, but grows from {}, which is level {}; it is kept as a skill of its own",
                rows[index].name, rows[index].level, rows[p].name, rows[p].level
            )),
            None => doubts.push(format!(
                "{} grows from row {from}, which names no skill; it is kept as a skill of its own",
                rows[index].name
            )),
        }
    }
    let skills = lineage(&rows, current.values.get(SKILLS));
    let (certainty, reason) = if doubts.is_empty() {
        (PlanCertainty::Read, None)
    } else {
        (
            PlanCertainty::Uncertain,
            Some(format!("{}.", doubts.join("; "))),
        )
    };
    plan.set_field(
        current,
        FieldChange {
            path: "skills".to_string(),
            target: SKILLS.to_string(),
            old: None,
            new: Some(skills),
            certainty,
            reason,
            source,
            play_state: false,
        },
    );
}

/// The skills as `trait_data.skills` holds them, in the sheet's order.
///
/// A skill the actor already has, with the same name and level under the same
/// parent, keeps its id, so a sheet brought in again is identical rather than
/// a change. Any other skill's id is made from its row: the same sheet always
/// plans the same ids, which the plan's hash depends on.
fn lineage(rows: &[Row], current: Option<&Value>) -> Value {
    let existing: Vec<(&str, &str, u64, Option<&str>)> = current
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|skill| {
            Some((
                skill.get("id")?.as_str()?,
                skill.get("name")?.as_str()?,
                skill.get("level")?.as_u64()?,
                skill.get("parentId").and_then(Value::as_str),
            ))
        })
        .collect();
    let mut ids: Vec<Option<String>> = vec![None; rows.len()];
    let mut used: BTreeSet<String> = BTreeSet::new();
    // Parents before children: a row's parent may be printed below it.
    let mut progress = true;
    while progress {
        progress = false;
        for index in 0..rows.len() {
            if ids[index].is_some() {
                continue;
            }
            let parent_id = match rows[index].parent {
                Some(p) => match &ids[p] {
                    Some(id) => Some(id.clone()),
                    None => continue,
                },
                None => None,
            };
            let kept = existing.iter().find(|(id, name, level, parent)| {
                !used.contains(*id)
                    && *name == rows[index].name
                    && *level == rows[index].level
                    && *parent == parent_id.as_deref()
            });
            let id = match kept {
                Some((id, ..)) => id.to_string(),
                None => {
                    let mut id = format!("sheet-{}", rows[index].row);
                    while used.contains(&id) || existing.iter().any(|(e, ..)| *e == id) {
                        id.push('x');
                    }
                    id
                }
            };
            used.insert(id.clone());
            ids[index] = Some(id);
            progress = true;
        }
    }
    let skills: Vec<Value> = rows
        .iter()
        .zip(&ids)
        .map(|(row, id)| {
            json!({
                "id": id,
                "name": row.name,
                "level": row.level,
                "parentId": row.parent.and_then(|p| ids[p].clone()),
            })
        })
        .collect();
    Value::Array(skills)
}

#[cfg(test)]
#[path = "sheet_import_tests.rs"]
mod tests;
