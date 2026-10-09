//! Hashes and names that must agree between the browser, the server and a
//! later re-import.
//!
//! The JSON is canonicalised here, with object keys sorted at every depth,
//! so a hash never depends on how a map happened to be built.

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Compact JSON with every object's keys in byte order.
pub fn canonical_json(value: &Value) -> String {
    let mut out = String::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                write_canonical(&map[key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

/// Lower-case hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The identity of a piece of content's own description: the same spell
/// read twice hashes the same, and a homebrew variant does not.
pub fn content_hash(kind: &str, fields: &Value) -> String {
    let body = serde_json::json!({ "kind": kind, "fields": fields });
    sha256_hex(canonical_json(&body).as_bytes())
}

/// The name content is matched by.
///
/// Lower case; a trailing "(PHB)"-style source tag and anything else in
/// parentheses dropped; apostrophes removed; other punctuation a space;
/// runs of spaces one. "Hunter's Mark (PHB)" and "hunters  mark" agree.
pub fn normalise_name(name: &str) -> String {
    let mut kept = String::with_capacity(name.len());
    let mut depth = 0usize;
    for ch in name.chars() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth > 0 => {}
            '\'' | '\u{2019}' | '\u{2018}' => {}
            c if c.is_alphanumeric() => kept.extend(c.to_lowercase()),
            _ => kept.push(' '),
        }
    }
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}
