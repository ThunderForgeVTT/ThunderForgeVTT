//! Spec 044 contract B7: a hero spec is checked before it is stored.
//!
//! A JSONB column takes anything, and the spec is untrusted input — it comes
//! from whoever may change the actor's image, in a request they can write by
//! hand. So before `uploadActorImage` writes an image with a spec, the spec
//! must be:
//!
//! 1. a JSON **object** (not an array, a string or a number);
//! 2. at most [`MAX_HERO_SPEC_BYTES`] serialised — the presets are well under
//!    1 KB, and the cap stops the column becoming a document store rather
//!    than being tight;
//! 3. valid against `HERO_SPEC_SCHEMA`, which `additionalProperties: false`
//!    makes refuse anything that is not a hero field — including the race a
//!    roll used (B5a).
//!
//! A refusal names what was wrong, and the caller stores neither the spec nor
//! the image.
//!
//! # Where the schema comes from
//!
//! `hero_spec_schema.json` beside this file is written by
//! `pnpm -F @thunderforge/heroes run schema` from `packages/heroes/src/
//! schema.ts`, and that package's `schema.test.ts` fails (in `pnpm verify`)
//! whenever this copy differs from what the package would write. Vendored
//! rather than read at build time so the crate builds without node; the test
//! is what keeps the copy honest.
//!
//! # Two checks the schema cannot make
//!
//! JSON Schema's `maxLength` counts code points, while the client's
//! `validateHero` counts UTF-16 code units, and a schema pattern's idea of
//! whitespace is not quite JavaScript's `trim()`. So `name` and `title` are
//! measured again here the way `validateHero` measures them, with the bounds
//! read from the schema, and nothing the client would refuse is stored.

use std::sync::LazyLock;

use serde_json::Value;

/// The largest spec stored, serialised as compact JSON.
pub const MAX_HERO_SPEC_BYTES: usize = 4 * 1024;

/// `HERO_SPEC_SCHEMA` as `packages/heroes` emits it.
pub const HERO_SPEC_SCHEMA_JSON: &str = include_str!("hero_spec_schema.json");

static SCHEMA: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(HERO_SPEC_SCHEMA_JSON).expect("hero_spec_schema.json is JSON")
});

static VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    jsonschema::draft202012::new(&SCHEMA).expect("hero_spec_schema.json is a valid 2020-12 schema")
});

/// Why a hero spec was not stored.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HeroSpecRefusal {
    #[error("heroSpec must be a JSON object describing a hero, not {0}")]
    NotAnObject(&'static str),
    #[error("heroSpec is {actual} bytes; a stored hero spec is at most {max} bytes")]
    TooLarge { actual: usize, max: usize },
    #[error("heroSpec is not a hero: {}", .0.join("; "))]
    Invalid(Vec<String>),
}

impl HeroSpecRefusal {
    /// A stable name for the rule, for a client that should not compare
    /// sentences.
    pub fn reason(&self) -> &'static str {
        match self {
            Self::NotAnObject(_) => "HERO_SPEC_NOT_OBJECT",
            Self::TooLarge { .. } => "HERO_SPEC_TOO_LARGE",
            Self::Invalid(_) => "HERO_SPEC_INVALID",
        }
    }
}

fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// JavaScript's `String.prototype.trim` whitespace: Unicode `White_Space`
/// plus the byte-order mark, which Rust's `char::is_whitespace` leaves out.
fn js_whitespace(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

fn max_length(field: &str) -> Option<usize> {
    SCHEMA
        .pointer(&format!("/properties/{field}/maxLength"))
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
}

/// `validateHero`'s own measure of the text fields: `String.length` (UTF-16
/// code units) against the schema's bound, and not only whitespace (an empty
/// title is allowed; an empty name is already refused by the schema).
fn text_problems(object: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut problems = Vec::new();
    for field in ["name", "title"] {
        let Some(Value::String(text)) = object.get(field) else {
            continue;
        };
        let may_be_empty = field == "title";
        if !(may_be_empty && text.is_empty()) && text.chars().all(js_whitespace) {
            problems.push(format!("{field}: needs some text"));
        }
        if let Some(max) = max_length(field)
            && text.encode_utf16().count() > max
        {
            problems.push(format!("{field}: at most {max} characters"));
        }
    }
    problems
}

/// Where in the spec a schema error points, as a field name: `/headgear` →
/// `headgear`, the root → the object itself.
fn field_of(path: &str) -> String {
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        "heroSpec".to_string()
    } else {
        trimmed.replace('/', ".")
    }
}

/// B7: `Ok(())` when `spec` may be stored beside an image.
pub fn check_hero_spec(spec: &Value) -> Result<(), HeroSpecRefusal> {
    let Value::Object(object) = spec else {
        return Err(HeroSpecRefusal::NotAnObject(kind_of(spec)));
    };

    let actual = serde_json::to_vec(spec)
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX);
    if actual > MAX_HERO_SPEC_BYTES {
        return Err(HeroSpecRefusal::TooLarge {
            actual,
            max: MAX_HERO_SPEC_BYTES,
        });
    }

    let mut problems: Vec<String> = VALIDATOR
        .iter_errors(spec)
        .map(|error| {
            format!(
                "{}: {}",
                field_of(&error.instance_path().to_string()),
                error
            )
        })
        .collect();
    for problem in text_problems(object) {
        if !problems.contains(&problem) {
            problems.push(problem);
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(HeroSpecRefusal::Invalid(problems))
    }
}
