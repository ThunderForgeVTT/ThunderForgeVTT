//! The line of arithmetic under a landed throw (data-model.md, "Derived:
//! the readout").
//!
//! It is computed here from the server's resolution and bindings, never sent
//! (AGENTS.md section 5), and it is ASCII only, because the engine's font has
//! no other glyphs (research R6).

use thunderforge_dice::{AddendKind, Breakdown, ResolutionKind, breakdown};

use super::ThrowSpec;

/// `Ayla: Stealth   13 + 3 = 16`.
pub fn readout(throw: &ThrowSpec) -> String {
    let prefix = match throw.label.as_deref().filter(|l| !l.is_empty()) {
        Some(label) => format!("{}: {}   ", throw.roller, label),
        None => format!("{}   ", throw.roller),
    };
    let resolution = &throw.resolution;
    let body = match breakdown(&resolution.formula, resolution, &throw.bindings) {
        Some(Breakdown::Sum(addends)) => sum(&addends, &resolution.kind),
        Some(Breakdown::Successes(_)) => match resolution.kind {
            ResolutionKind::SuccessCount(1) => Some("1 success".to_string()),
            ResolutionKind::SuccessCount(n) => Some(format!("{n} successes")),
            ResolutionKind::Total(_) => None,
        },
        None => None,
    }
    .unwrap_or_else(|| {
        format!(
            "{} = {}",
            substitute(&resolution.formula, throw),
            total(&resolution.kind)
        )
    });
    ascii(&(prefix + &body))
}

/// `+20 more`, when more dice were rolled than drawn.
pub fn chip(drawn: usize, total_dice: usize) -> Option<String> {
    (total_dice > drawn).then(|| format!("+{} more", total_dice - drawn))
}

/// A number as the readout prints it: whole numbers bare, others with two
/// decimals.
pub fn number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value:.2}")
    }
}

fn total(kind: &ResolutionKind) -> String {
    match kind {
        ResolutionKind::Total(t) => number(*t),
        ResolutionKind::SuccessCount(n) => n.to_string(),
    }
}

/// `a + b - c = total`, or `None` when the addends do not reach the
/// server's total.
fn sum(addends: &[thunderforge_dice::Addend], kind: &ResolutionKind) -> Option<String> {
    let ResolutionKind::Total(server) = kind else {
        return None;
    };
    let mut text = String::new();
    let mut reached = 0.0;
    for (i, addend) in addends.iter().enumerate() {
        let magnitude = match &addend.kind {
            AddendKind::Die { value, .. } => *value as f64,
            AddendKind::Constant { value, .. } => *value,
        };
        let signed = if addend.negative {
            -magnitude
        } else {
            magnitude
        };
        reached += signed;
        let shown = number(signed.abs());
        match (i, signed < 0.0) {
            (0, false) => text.push_str(&shown),
            (0, true) => text.push_str(&format!("-{shown}")),
            (_, false) => text.push_str(&format!(" + {shown}")),
            (_, true) => text.push_str(&format!(" - {shown}")),
        }
    }
    if addends.is_empty() || (reached - server).abs() > 1e-6 {
        return None;
    }
    Some(format!("{text} = {}", number(*server)))
}

/// The formula with each bound placeholder replaced by its value, matched
/// as a whole identifier.
fn substitute(formula: &str, throw: &ThrowSpec) -> String {
    let mut out = String::new();
    let chars: Vec<char> = formula.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let starts = (c.is_ascii_alphabetic() || c == '_')
            && (i == 0 || !(chars[i - 1].is_ascii_alphanumeric() || chars[i - 1] == '_'));
        if !starts {
            out.push(c);
            i += 1;
            continue;
        }
        let end = (i..chars.len())
            .find(|&j| !(chars[j].is_ascii_alphanumeric() || chars[j] == '_'))
            .unwrap_or(chars.len());
        let word: String = chars[i..end].iter().collect();
        match throw.bindings.get(&word) {
            Some(value) => out.push_str(&number(*value)),
            None => out.push_str(&word),
        }
        i = end;
    }
    out
}

fn ascii(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii() && !c.is_ascii_control() {
                c
            } else {
                '?'
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "readout_tests.rs"]
mod tests;
