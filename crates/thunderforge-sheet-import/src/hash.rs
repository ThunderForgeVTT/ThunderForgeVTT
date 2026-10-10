//! Hashes and names that must agree between the browser, the server and a
//! later re-import.
//!
//! The JSON is canonicalised here, with object keys sorted at every depth,
//! so a hash never depends on how a map happened to be built. A whole
//! number is written as one (`55`, never `55.0`): the browser's reading
//! passes through JavaScript, which keeps no difference between the two, and
//! the server's own reading must hash the same.

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
        Value::Number(number) => out.push_str(&canonical_number(number)),
        other => out.push_str(&other.to_string()),
    }
}

/// The number as JavaScript would write it, as far as a sheet's numbers go:
/// a float with nothing after the point is the integer.
fn canonical_number(number: &serde_json::Number) -> String {
    if number.is_f64()
        && let Some(float) = number.as_f64()
        && float.is_finite()
        && float.fract() == 0.0
        && float.abs() < 9_007_199_254_740_992.0
    {
        return format!("{}", float as i64);
    }
    number.to_string()
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_whole_float_hashes_as_the_integer_javascript_would_send() {
        let native = json!({ "weight": 55.0, "cost": 0.5, "n": -3.0 });
        let browser = json!({ "weight": 55, "cost": 0.5, "n": -3 });
        assert_eq!(canonical_json(&native), canonical_json(&browser));
        assert_eq!(
            canonical_json(&native),
            r#"{"cost":0.5,"n":-3,"weight":55}"#
        );
        assert_eq!(
            content_hash("item", &native),
            content_hash("item", &browser)
        );
    }
}
