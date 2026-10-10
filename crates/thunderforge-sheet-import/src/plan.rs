//! The mapping engine: a reading, planned onto an actor (spec 048 FR-010 to
//! FR-014).
//!
//! [`plan`] is pure. It reads the declaration, the reading, the person's
//! corrections, the actor as it is now and an index of the world's content,
//! and says what an import would do. It writes nothing; the server applies a
//! plan only when its [`plan_hash`] matches the one the person reviewed.
//!
//! Every leaf of a reading ends in exactly one place: a field change, a
//! content change, `unmapped` (kept in the notes), `identical` (nothing to
//! do), `checked` (a derived value) or `ignored` (declared as such). Nothing
//! is dropped (FR-013).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::character::{Certainty, ContentLink, ImportedCharacter, Leaf, Source};
use crate::hash::{canonical_json, content_hash, normalise_name, sha256_hex};
use crate::mapping::{ContentTarget, SheetMapping};

/// The person's corrections, by neutral path. A value replaces what was
/// read; `content.N.name` renames a piece of content.
pub type Corrections = BTreeMap<String, Value>;

/// A pack's last word on a plan: what data cannot say.
pub type RefineFn = fn(&ImportedCharacter, &ActorSnapshot, &mut ImportPlan);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlanCertainty {
    Read,
    Uncertain,
    Unread,
    Corrected,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldChange {
    pub path: String,
    pub target: String,
    pub old: Option<Value>,
    pub new: Option<Value>,
    pub certainty: PlanCertainty,
    pub reason: Option<String>,
    pub source: Option<Source>,
    /// On a re-import: not written unless the person names the target.
    pub play_state: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Resolution {
    /// The world already has it.
    World { id: String },
    /// Brought in before, the same, still awaiting the GM.
    StagedExisting { id: String },
    /// New to the world: staged for the GM.
    StagedNew,
    /// Staged before under this name, but this reading describes it
    /// differently: staged again, marked as differing.
    Differs { id: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentChange {
    pub kind: String,
    pub name: String,
    pub normalised: String,
    pub target: ContentTarget,
    pub resolution: Resolution,
    pub fields: Value,
    pub content_hash: String,
    pub link: ContentLink,
    pub certainty: PlanCertainty,
    pub reason: Option<String>,
    pub source: Option<Source>,
    /// On a re-import: linked now, not on the sheet.
    pub removed: bool,
    /// The reading's `content.N` paths this change came from.
    pub from: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Unmapped {
    pub path: String,
    /// What the notes call it.
    pub label: String,
    pub value: Value,
    pub goes_to: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossCheck {
    pub path: String,
    pub sheet: Value,
    pub derived: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptInPlay {
    pub target: String,
    pub current: Option<Value>,
    pub sheet: Option<Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlan {
    pub reader_id: String,
    pub reader_version: String,
    pub is_reimport: bool,
    pub fields: Vec<FieldChange>,
    pub content: Vec<ContentChange>,
    pub unmapped: Vec<Unmapped>,
    pub cross_checks: Vec<CrossCheck>,
    pub kept_in_play: Vec<KeptInPlay>,
    /// Declared paths whose value the actor already has.
    pub identical: Vec<String>,
    /// Derived paths, checked and never written.
    pub checked: Vec<String>,
    /// Paths the declaration reads and deliberately keeps nowhere.
    pub ignored: Vec<String>,
}

/// One piece of content the actor links now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentLink {
    pub kind: String,
    pub name: String,
    pub id: String,
    pub staged: bool,
}

/// The actor as it is before the import.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActorSnapshot {
    /// Whether a sheet was imported onto this actor before.
    pub is_reimport: bool,
    /// Target ("ability_data.strength", "actor.label") to its value now.
    pub values: BTreeMap<String, Value>,
    pub links: Vec<CurrentLink>,
}

/// Where a world already holds a piece of content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Indexed {
    World { id: String },
    Staged { id: String, content_hash: String },
}

/// The world's content and what is staged in it, by kind and normalised
/// name.
pub trait ContentIndex {
    fn lookup(&self, kind: &str, normalised: &str) -> Option<Indexed>;
}

impl ImportPlan {
    /// Take the change to `target` out of the plan, for a pack's refine hook
    /// to reshape and put back with [`ImportPlan::set_field`].
    pub fn take_field(&mut self, target: &str) -> Option<FieldChange> {
        let index = self.fields.iter().position(|f| f.target == target)?;
        Some(self.fields.remove(index))
    }

    /// Put a change to a target into the plan the way the planner would:
    /// `old` is what the actor holds now, a value the actor already has is
    /// identical rather than a change, and a change to the same target
    /// replaces the one before it. For a pack's refine hook, which writes
    /// targets the declaration cannot express.
    pub fn set_field(&mut self, current: &ActorSnapshot, mut change: FieldChange) {
        self.fields.retain(|f| f.target != change.target);
        change.old = current.values.get(&change.target).cloned();
        if change.certainty != PlanCertainty::Unread && change.new == change.old {
            self.identical.push(change.path);
            return;
        }
        if change.certainty == PlanCertainty::Unread {
            change.new = None;
        }
        self.fields.push(change);
    }

    /// The field changes an apply writes. A play-state change on a re-import
    /// is written only if `overwrite` names its target.
    pub fn writes<'a, 'b>(
        &'a self,
        overwrite: &'b BTreeSet<String>,
    ) -> impl Iterator<Item = &'a FieldChange> + use<'a, 'b> {
        self.fields.iter().filter(move |field| {
            field.certainty != PlanCertainty::Unread
                && (!field.play_state || overwrite.contains(&field.target))
        })
    }

    /// Record what the sheet printed against what the rules derive.
    ///
    /// The same value is nothing. A different one is a cross-check, and the
    /// base fields it is derived from become uncertain, both numbers shown,
    /// unless the person already corrected them.
    pub fn cross_check(&mut self, path: &str, sheet: Value, derived: Value, base: &[&str]) {
        if sheet == derived {
            return;
        }
        let reason = format!("the sheet prints {sheet} for {path}; the rules give {derived}");
        for field in &mut self.fields {
            if base.contains(&field.path.as_str()) && field.certainty == PlanCertainty::Read {
                field.certainty = PlanCertainty::Uncertain;
                field.reason = Some(reason.clone());
            }
        }
        self.cross_checks.push(CrossCheck {
            path: path.to_string(),
            sheet,
            derived,
        });
    }

    /// The part of a plan its hash covers: everything the person reviews.
    fn hashed(&self) -> Value {
        serde_json::json!({
            "readerId": self.reader_id,
            "readerVersion": self.reader_version,
            "isReimport": self.is_reimport,
            "fields": self.fields,
            "content": self.content,
            "unmapped": self.unmapped,
            "crossChecks": self.cross_checks,
            "keptInPlay": self.kept_in_play,
        })
    }

    fn sort(&mut self) {
        self.fields.sort_by(|a, b| a.path.cmp(&b.path));
        self.content.sort_by(|a, b| {
            (a.removed, &a.kind, &a.normalised).cmp(&(b.removed, &b.kind, &b.normalised))
        });
        self.unmapped
            .sort_by(|a, b| natural(&a.path).cmp(&natural(&b.path)));
        self.cross_checks.sort_by(|a, b| a.path.cmp(&b.path));
        self.kept_in_play.sort_by(|a, b| a.target.cmp(&b.target));
        for list in [&mut self.identical, &mut self.checked, &mut self.ignored] {
            list.sort();
            list.dedup();
        }
    }
}

/// SHA-256 over the canonical JSON of what the person reviews.
pub fn plan_hash(plan: &ImportPlan) -> String {
    sha256_hex(canonical_json(&plan.hashed()).as_bytes())
}

/// A path's segments, with numbers compared as numbers: `notes.2` before
/// `notes.10`.
fn natural(path: &str) -> Vec<(u8, u64, &str)> {
    path.split('.')
        .map(|segment| match segment.parse::<u64>() {
            Ok(number) => (0, number, ""),
            Err(_) => (1, 0, segment),
        })
        .collect()
}

/// A leaf, with the person's correction applied.
struct Planned {
    path: String,
    value: Option<Value>,
    certainty: PlanCertainty,
    reason: Option<String>,
    source: Option<Source>,
}

fn planned(leaf: Leaf, corrections: &Corrections) -> Planned {
    if let Some(corrected) = corrections.get(&leaf.path) {
        return Planned {
            path: leaf.path,
            value: Some(corrected.clone()).filter(|v| !v.is_null()),
            certainty: PlanCertainty::Corrected,
            reason: None,
            source: leaf.source,
        };
    }
    let (certainty, reason) = match leaf.certainty {
        Certainty::Read => (PlanCertainty::Read, None),
        Certainty::Uncertain { reason } => (PlanCertainty::Uncertain, Some(reason)),
        Certainty::Unread => (PlanCertainty::Unread, None),
    };
    Planned {
        path: leaf.path,
        value: leaf.value,
        certainty,
        reason,
        source: leaf.source,
    }
}

/// Plan `reading` onto `current`.
pub fn plan(
    mapping: &SheetMapping,
    refine: Option<RefineFn>,
    reading: &ImportedCharacter,
    corrections: &Corrections,
    current: &ActorSnapshot,
    world: &dyn ContentIndex,
) -> ImportPlan {
    let refine = refine.map(|hook| move |r: &_, c: &_, p: &mut _| hook(r, c, p));
    plan_with(
        mapping,
        refine.as_ref().map(|hook| hook as &RefineHook),
        reading,
        corrections,
        current,
        world,
    )
}

/// A refine hook that need not be a plain function: the host's adapter over
/// a pack's JSON hook is a closure.
pub type RefineHook = dyn Fn(&ImportedCharacter, &ActorSnapshot, &mut ImportPlan);

/// [`plan`], with the refine hook as any callable.
pub fn plan_with(
    mapping: &SheetMapping,
    refine: Option<&RefineHook>,
    reading: &ImportedCharacter,
    corrections: &Corrections,
    current: &ActorSnapshot,
    world: &dyn ContentIndex,
) -> ImportPlan {
    let mut out = ImportPlan {
        reader_id: reading.reader.id.clone(),
        reader_version: reading.reader.version.clone(),
        is_reimport: current.is_reimport,
        ..ImportPlan::default()
    };

    let leaves = reading.leaves();
    let known: BTreeSet<String> = leaves.iter().map(|leaf| leaf.path.clone()).collect();
    let mut planned_leaves: Vec<Planned> = leaves
        .into_iter()
        .map(|leaf| planned(leaf, corrections))
        .collect();
    // A correction for something the sheet did not have at all: the person
    // typed in a value the export left out.
    for (path, value) in corrections {
        if !known.contains(path) && mapping.field_for(path).is_some() {
            planned_leaves.push(Planned {
                path: path.clone(),
                value: Some(value.clone()).filter(|v| !v.is_null()),
                certainty: PlanCertainty::Corrected,
                reason: None,
                source: None,
            });
        }
    }

    let mut groups: BTreeMap<String, Vec<Planned>> = BTreeMap::new();
    let mut content_leaves: Vec<usize> = Vec::new();
    for leaf in planned_leaves {
        if let Some(index) = leaf.path.strip_prefix("content.") {
            if let Ok(index) = index.parse::<usize>() {
                content_leaves.push(index);
            }
            continue;
        }
        if let Some(key) = leaf.path.strip_prefix("derived.")
            && mapping.is_derived(key)
        {
            if leaf.value.is_some() {
                out.checked.push(leaf.path);
            }
            continue;
        }
        if mapping.is_ignored(&leaf.path) {
            if leaf.value.is_some() {
                out.ignored.push(leaf.path);
            }
            continue;
        }
        if leaf.path.starts_with("notes.") {
            unmapped_leaf(&mut out, mapping, reading, leaf);
            continue;
        }
        match mapping.field_for(&leaf.path) {
            Some((declared, _)) => groups.entry(declared.to_string()).or_default().push(leaf),
            None => unmapped_leaf(&mut out, mapping, reading, leaf),
        }
    }

    for (declared, group) in groups {
        let target = mapping.fields[&declared].clone();
        field_change(
            &mut out,
            mapping,
            current,
            corrections,
            &declared,
            &target,
            group,
        );
    }

    plan_content(
        &mut out,
        mapping,
        reading,
        corrections,
        current,
        world,
        &content_leaves,
    );

    if let Some(refine) = refine {
        refine(reading, current, &mut out);
        // A hook that reshaped a piece of content changed what will be
        // staged: hash it again and look it up again, so the same sheet
        // imported twice finds what it staged the first time.
        for change in out.content.iter_mut().filter(|c| !c.removed) {
            let hash = content_hash(&change.kind, &change.fields);
            if hash != change.content_hash {
                change.resolution = resolve(world, &change.kind, &change.normalised, &hash);
                change.content_hash = hash;
            }
        }
    }

    // A kind the declaration hands to the pack, which the pack left
    // undecided, is kept in the notes rather than dropped.
    let (undecided, decided): (Vec<_>, Vec<_>) = std::mem::take(&mut out.content)
        .into_iter()
        .partition(|change| change.target == ContentTarget::Refine && !change.removed);
    out.content = decided;
    for change in undecided {
        for path in &change.from {
            out.unmapped.push(Unmapped {
                path: path.clone(),
                label: change.name.clone(),
                value: serde_json::json!({ "kind": change.kind, "name": change.name, "fields": change.fields }),
                goes_to: mapping.notes.clone(),
            });
        }
    }

    out.sort();
    out
}

fn unmapped_leaf(
    out: &mut ImportPlan,
    mapping: &SheetMapping,
    reading: &ImportedCharacter,
    leaf: Planned,
) {
    let Some(value) = leaf.value else {
        return;
    };
    let label = leaf
        .path
        .strip_prefix("notes.")
        .and_then(|index| index.parse::<usize>().ok())
        .and_then(|index| reading.notes.get(index))
        .map(|note| note.label.clone())
        .unwrap_or_else(|| leaf.path.clone());
    out.unmapped.push(Unmapped {
        path: leaf.path,
        label,
        value,
        goes_to: mapping.notes.clone(),
    });
}

#[allow(clippy::too_many_arguments)]
fn field_change(
    out: &mut ImportPlan,
    mapping: &SheetMapping,
    current: &ActorSnapshot,
    corrections: &Corrections,
    declared: &str,
    target: &str,
    group: Vec<Planned>,
) {
    let old = current.values.get(target).cloned();
    let (new, certainty, reason, source) = if let [only] = group.as_slice()
        && only.path == declared
    {
        (
            only.value.clone(),
            only.certainty,
            only.reason.clone(),
            only.source.clone(),
        )
    } else if let Some(whole) = corrections.get(declared) {
        let source = group.iter().find_map(|leaf| leaf.source.clone());
        (
            Some(whole.clone()).filter(|v| !v.is_null()),
            PlanCertainty::Corrected,
            None,
            source,
        )
    } else {
        compose(declared, &group, old.as_ref())
    };

    if certainty != PlanCertainty::Unread && new == old {
        out.identical.push(declared.to_string());
        return;
    }
    let play_state = current.is_reimport && mapping.play_state.contains(target);
    if play_state && certainty != PlanCertainty::Unread {
        out.kept_in_play.push(KeptInPlay {
            target: target.to_string(),
            current: old.clone(),
            sheet: new.clone(),
        });
    }
    out.fields.push(FieldChange {
        path: declared.to_string(),
        target: target.to_string(),
        old,
        new: if certainty == PlanCertainty::Unread {
            None
        } else {
            new
        },
        certainty,
        reason,
        source,
        play_state,
    });
}

type Composed = (Option<Value>, PlanCertainty, Option<String>, Option<Source>);

/// Several leaves landing as one value: a list of classes, a purse of coins.
///
/// An object starts from what the actor holds, so a part the sheet left
/// unread keeps its value. A list starts empty: it is the sheet's list.
fn compose(declared: &str, group: &[Planned], old: Option<&Value>) -> Composed {
    let relative = |path: &str| -> Vec<String> {
        path[declared.len()..]
            .trim_start_matches('.')
            .split('.')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    };
    let is_list = group
        .first()
        .and_then(|leaf| relative(&leaf.path).first().cloned())
        .is_some_and(|first| first.parse::<usize>().is_ok());
    let mut value = match (is_list, old) {
        (false, Some(old @ Value::Object(_))) => old.clone(),
        (false, _) => Value::Object(Default::default()),
        (true, _) => Value::Array(Vec::new()),
    };

    let mut any_read = false;
    let mut corrected = false;
    let mut reasons = Vec::new();
    for leaf in group {
        if leaf.certainty == PlanCertainty::Unread {
            continue;
        }
        any_read = true;
        corrected |= leaf.certainty == PlanCertainty::Corrected;
        let segments = relative(&leaf.path);
        if let Some(reason) = &leaf.reason {
            reasons.push(format!("{}: {reason}", segments.join(".")));
        }
        set_path(
            &mut value,
            &segments,
            leaf.value.clone().unwrap_or(Value::Null),
        );
    }
    let source = group.iter().find_map(|leaf| leaf.source.clone());
    if !any_read {
        return (None, PlanCertainty::Unread, None, source);
    }
    let certainty = if !reasons.is_empty() {
        PlanCertainty::Uncertain
    } else if corrected {
        PlanCertainty::Corrected
    } else {
        PlanCertainty::Read
    };
    let reason = (!reasons.is_empty()).then(|| reasons.join("; "));
    (Some(value), certainty, reason, source)
}

fn set_path(node: &mut Value, segments: &[String], value: Value) {
    let Some((first, rest)) = segments.split_first() else {
        *node = value;
        return;
    };
    if let Ok(index) = first.parse::<usize>() {
        if !node.is_array() {
            *node = Value::Array(Vec::new());
        }
        let items = node.as_array_mut().expect("just made a list");
        while items.len() <= index {
            items.push(Value::Null);
        }
        set_path(&mut items[index], rest, value);
    } else {
        if !node.is_object() {
            *node = Value::Object(Default::default());
        }
        let map = node.as_object_mut().expect("just made an object");
        set_path(map.entry(first.clone()).or_insert(Value::Null), rest, value);
    }
}

#[allow(clippy::too_many_arguments)]
fn plan_content(
    out: &mut ImportPlan,
    mapping: &SheetMapping,
    reading: &ImportedCharacter,
    corrections: &Corrections,
    current: &ActorSnapshot,
    world: &dyn ContentIndex,
    indices: &[usize],
) {
    let mut by_key: BTreeMap<(String, String), ContentChange> = BTreeMap::new();
    for &index in indices {
        let Some(item) = reading.content.get(index) else {
            continue;
        };
        let path = format!("content.{index}");
        let renamed = corrections
            .get(&format!("{path}.name"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let name = renamed.clone().or_else(|| item.name.value.clone());
        let target = mapping.content.get(&item.kind);
        let (Some(name), Some(target)) = (name, target) else {
            out.unmapped.push(Unmapped {
                label: item.name.value.clone().unwrap_or_else(|| path.clone()),
                value: serde_json::to_value(item).expect("content serialises"),
                path,
                goes_to: mapping.notes.clone(),
            });
            continue;
        };
        let (certainty, reason) = match (&renamed, &item.name.certainty) {
            (Some(_), _) => (PlanCertainty::Corrected, None),
            (None, Certainty::Uncertain { reason }) => {
                (PlanCertainty::Uncertain, Some(reason.clone()))
            }
            (None, Certainty::Unread) => (PlanCertainty::Unread, None),
            (None, Certainty::Read) => (PlanCertainty::Read, None),
        };
        let normalised = normalise_name(&name);
        let key = (item.kind.clone(), normalised.clone());
        if let Some(existing) = by_key.get_mut(&key) {
            merge_link(&mut existing.link, &item.link);
            existing.from.push(path);
            continue;
        }
        let fields = Value::Object(item.fields.clone().into_iter().collect());
        let hash = content_hash(&item.kind, &fields);
        let resolution = resolve(world, &item.kind, &normalised, &hash);
        let mut link = item.link.clone();
        link.granted_by.sort();
        link.granted_by.dedup();
        by_key.insert(
            key,
            ContentChange {
                kind: item.kind.clone(),
                name,
                normalised,
                target: target.clone(),
                resolution,
                fields,
                content_hash: hash,
                link,
                certainty,
                reason,
                source: item.name.source.clone(),
                removed: false,
                from: vec![path],
            },
        );
    }

    if current.is_reimport {
        for link in &current.links {
            let key = (link.kind.clone(), normalise_name(&link.name));
            if by_key.contains_key(&key) {
                continue;
            }
            let Some(target) = mapping.content.get(&link.kind) else {
                continue;
            };
            let resolution = if link.staged {
                Resolution::StagedExisting {
                    id: link.id.clone(),
                }
            } else {
                Resolution::World {
                    id: link.id.clone(),
                }
            };
            out.content.push(ContentChange {
                kind: link.kind.clone(),
                name: link.name.clone(),
                normalised: key.1,
                target: target.clone(),
                resolution,
                fields: Value::Null,
                content_hash: String::new(),
                link: ContentLink::default(),
                certainty: PlanCertainty::Read,
                reason: None,
                source: None,
                removed: true,
                from: Vec::new(),
            });
        }
    }
    out.content.extend(by_key.into_values());
}

/// Where a piece of content stands in the world: there already, staged the
/// same, staged differently, or new.
fn resolve(world: &dyn ContentIndex, kind: &str, normalised: &str, hash: &str) -> Resolution {
    match world.lookup(kind, normalised) {
        Some(Indexed::World { id }) => Resolution::World { id },
        Some(Indexed::Staged { id, content_hash }) if content_hash == hash => {
            Resolution::StagedExisting { id }
        }
        Some(Indexed::Staged { id, .. }) => Resolution::Differs { id },
        None => Resolution::StagedNew,
    }
}

/// The same content twice on a sheet (a spell two classes grant) is one
/// change; what grants it is everything that does.
fn merge_link(into: &mut ContentLink, from: &ContentLink) {
    into.granted_by.extend(from.granted_by.iter().cloned());
    into.granted_by.sort();
    into.granted_by.dedup();
    into.prepared = into.prepared.or(from.prepared);
    into.uses = into.uses.or(from.uses);
    if into.recharge.is_none() {
        into.recharge = from.recharge.clone();
    }
    into.equipped = into.equipped.or(from.equipped);
    into.attuned = into.attuned.or(from.attuned);
    into.quantity = into.quantity.or(from.quantity);
}
