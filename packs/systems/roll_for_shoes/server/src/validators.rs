//! What a Roll for Shoes character may store.
//!
//! Two slots, and the rules below are the whole of them. The numbering (R1-R3,
//! T1-T11) is the one in `specs/061-roll-for-shoes/data-model.md`, and the
//! message each rule produces is quoted there, because the sheet shows these
//! strings to a player rather than swallowing them.
//!
//! # What this deliberately does not check
//!
//! Whether a new skill's name is *more specific* than the skill it grew out
//! of, and whether it is *relevant* to what the character tried to do. Those
//! are the two conditions the game puts on an advancement, and they are
//! settled at the table by the people playing. A validator that arbitrated
//! them would be playing the game for them, so any non-empty name passes here.
//!
//! There is also no cap on a skill's level. The dice engine refuses to resolve
//! more than a thousand dice at once, which is a fact about one roll and not
//! about a character, so it is not enforced here.

/// A refusal, with the field it is about.
///
/// Rendered as `field: message`, which is what the sheet puts in front of the
/// player and what `specs/061-roll-for-shoes/data-model.md` lists.
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl ValidationError {
    fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for ValidationError {}

// ============================================================================
// resource_data — the experience a character has earned by failing
// ============================================================================

/// Validates `{ "xp": <integer ≥ 0> }`.
///
/// `xp` may be absent: a character who has never failed has nothing stored,
/// and the sheet reads an absent balance as zero rather than writing one on
/// creation.
pub fn validate_resource_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    // R1
    let obj = data
        .as_object()
        .ok_or_else(|| ValidationError::new("resource_data", "must be a JSON object"))?;

    let Some(xp) = obj.get("xp") else {
        return Ok(());
    };

    // R2
    let xp = xp
        .as_i64()
        .ok_or_else(|| ValidationError::new("xp", "must be an integer"))?;

    // R3
    if xp < 0 {
        return Err(ValidationError::new("xp", "must not be negative"));
    }

    Ok(())
}

// ============================================================================
// trait_data — the description, and the skill lineage
// ============================================================================

/// Validates a character's description and skills.
///
/// The lineage is the only structure in this game: every skill but the first
/// records the skill it grew out of, and sits exactly one level above it.
/// That single relation (T9) is also what makes a cycle impossible rather than
/// forbidden — levels strictly increase along every parent link, and a cycle
/// would need a level above itself — so there is no traversal here.
pub fn validate_trait_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    // T1
    let obj = data
        .as_object()
        .ok_or_else(|| ValidationError::new("trait_data", "must be a JSON object"))?;

    // T2
    if let Some(description) = obj.get("description") {
        if !description.is_null() && !description.is_string() {
            return Err(ValidationError::new("description", "must be text"));
        }
    }

    let Some(skills) = obj.get("skills") else {
        return Ok(());
    };
    if skills.is_null() {
        return Ok(());
    }

    // T3
    let skills = skills
        .as_array()
        .ok_or_else(|| ValidationError::new("skills", "must be a list"))?;

    if skills.is_empty() {
        return Ok(());
    }

    // First pass: each entry's own shape, and the ids it may be pointed at by.
    let mut levels_by_id: std::collections::HashMap<&str, i64> = std::collections::HashMap::new();

    for (index, entry) in skills.iter().enumerate() {
        // T4
        let entry = entry.as_object().ok_or_else(|| {
            ValidationError::new(
                format!("skills[{index}]"),
                "must be an object with id, name and level",
            )
        })?;

        for key in ["id", "name", "level"] {
            if !entry.contains_key(key) {
                return Err(ValidationError::new(
                    format!("skills[{index}]"),
                    "must be an object with id, name and level",
                ));
            }
        }

        // T5
        let id = entry.get("id").and_then(|id| id.as_str()).unwrap_or("");
        if id.is_empty() || levels_by_id.contains_key(id) {
            return Err(ValidationError::new(
                format!("skills[{index}].id"),
                "must be unique and not empty",
            ));
        }

        // T6
        let name = entry.get("name").and_then(|name| name.as_str());
        if !name.is_some_and(|name| !name.trim().is_empty()) {
            return Err(ValidationError::new(
                format!("skills[{index}].name"),
                "must not be empty",
            ));
        }

        // T7
        let level = entry.get("level").and_then(serde_json::Value::as_i64);
        let level = level.filter(|level| *level >= 1).ok_or_else(|| {
            ValidationError::new(
                format!("skills[{index}].level"),
                "must be a whole number of at least 1",
            )
        })?;

        levels_by_id.insert(id, level);
    }

    // Second pass: how the entries relate. Parents may appear after their
    // children in the list — the sheet appends, but nothing promises an order
    // — so every id has to be known before any link is checked.
    let mut roots = 0usize;

    for (index, entry) in skills.iter().enumerate() {
        let entry = entry.as_object().expect("shape checked above");
        let level = entry
            .get("level")
            .and_then(serde_json::Value::as_i64)
            .expect("level checked above");

        let parent_id = entry.get("parentId").unwrap_or(&serde_json::Value::Null);

        if parent_id.is_null() {
            roots += 1;
            // T11
            if level != 1 {
                return Err(ValidationError::new(
                    "skills",
                    "the starting skill must be at level 1",
                ));
            }
            continue;
        }

        // T8
        let own_id = entry.get("id").and_then(|id| id.as_str()).unwrap_or("");
        let parent_level = parent_id
            .as_str()
            .filter(|parent| *parent != own_id)
            .and_then(|parent| levels_by_id.get(parent))
            .ok_or_else(|| {
                ValidationError::new(
                    format!("skills[{index}].parentId"),
                    "must name another skill",
                )
            })?;

        // T9
        if level != parent_level + 1 {
            return Err(ValidationError::new(
                format!("skills[{index}].level"),
                "must be one higher than its parent",
            ));
        }
    }

    // T10
    if roots != 1 {
        return Err(ValidationError::new(
            "skills",
            "must have exactly one starting skill",
        ));
    }

    Ok(())
}

// ============================================================================
// The registry's shape
// ============================================================================

pub fn validate_resource_data_for_registry(data: &serde_json::Value) -> Result<(), String> {
    validate_resource_data(data).map_err(|e| e.to_string())
}

pub fn validate_trait_data_for_registry(data: &serde_json::Value) -> Result<(), String> {
    validate_trait_data(data).map_err(|e| e.to_string())
}
