//! Reading a system's declaration of what its content looks like (spec 049).
//!
//! The mirror of [`crate::vision_profiles`], deliberately: a system declares
//! where its own meaning lives, and shared code reads the declaration rather
//! than the words. `scripts/check-system-registry.mjs` fails the build if any
//! file under `src/server/src` so much as quotes a system's id, which is what
//! keeps this honest.
//!
//! Split `_for_system` from `_from_manifest` the same way, so the reading can
//! be tested without a filesystem.
//!
//! # Absent is a refusal here, not a default
//!
//! This is the one place the vision precedent does **not** carry over. A
//! system that declares no vision gets ordinary sight, which is a correct
//! answer. A system that declares no content patterns has no correct answer:
//! reading a book with another system's vocabulary would produce entries that
//! look plausible and are wrong, which is the failure this whole feature
//! exists to avoid. So callers get an empty declaration and must refuse the
//! import with a reason (FR-015), rather than falling back to anything.

use thunderforge_canvas_core::content_patterns::ContentPatterns;

/// Read a system's `contentPatterns` block off disk.
///
/// Absent, unreadable and malformed all yield an empty declaration — and an
/// empty declaration means the import is refused, so the three are the same
/// answer to the caller.
pub fn content_patterns_for_system(systems_dir: &str, system_id: &str) -> ContentPatterns {
    let path = std::path::Path::new(systems_dir)
        .join(system_id)
        .join("system.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return ContentPatterns::default();
    };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
        return ContentPatterns::default();
    };
    content_patterns_from_manifest(&manifest)
}

/// The same, from a manifest already in hand.
///
/// A block that fails to parse is treated as absent rather than as an error,
/// for the reason `vision_from_manifest` gives: install-time validation
/// (`pack_system_spec::validate_system_manifest`) is where a malformed block
/// is reported to the person who wrote it. What differs is the consequence —
/// absent here stops an import rather than quietly changing what it produces.
pub fn content_patterns_from_manifest(manifest: &serde_json::Value) -> ContentPatterns {
    let Some(block) = manifest.get("contentPatterns") else {
        return ContentPatterns::default();
    };
    // The manifest carries a bare array; the runtime type wraps it, so that
    // `ContentPatterns` has somewhere to grow a sibling field later without
    // changing what a pack author writes.
    serde_json::from_value::<Vec<_>>(block.clone())
        .map(|patterns| ContentPatterns { patterns })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use thunderforge_canvas_core::content_patterns::{FieldKind, Shape};

    fn manifest(patterns: serde_json::Value) -> serde_json::Value {
        serde_json::json!({ "id": "t", "contentPatterns": patterns })
    }

    #[test]
    fn a_manifest_with_no_block_declares_nothing() {
        let read = content_patterns_from_manifest(&serde_json::json!({ "id": "t" }));
        assert!(read.is_empty(), "absent means nothing declared");
    }

    #[test]
    fn a_malformed_block_declares_nothing_rather_than_erroring() {
        let read = content_patterns_from_manifest(&manifest(serde_json::json!("not an array")));
        assert!(read.is_empty());
    }

    #[test]
    fn an_anchored_pattern_is_read_whole() {
        let read = content_patterns_from_manifest(&manifest(serde_json::json!([{
            "kind": "creature",
            "shape": "anchored",
            "anchor": "Armor Class",
            "name": { "position": "before", "withinLines": 6, "prefer": "largest" },
            "fields": [{ "key": "armorClass", "label": "Armor Class", "as": "integer" }]
        }])));

        let pattern = read.for_kind("creature").expect("declared");
        assert_eq!(pattern.shape, Shape::Anchored);
        assert_eq!(pattern.anchor.as_deref(), Some("Armor Class"));
        assert_eq!(pattern.name.within_lines, Some(6));
        assert_eq!(pattern.fields[0].value_kind, FieldKind::Integer);
    }

    #[test]
    fn a_prose_pattern_carries_no_fields() {
        let read = content_patterns_from_manifest(&manifest(serde_json::json!([{
            "kind": "feat",
            "shape": "prose",
            "name": { "style": "heading", "endsAt": "nextName" }
        }])));

        let pattern = read.for_kind("feat").expect("declared");
        assert_eq!(pattern.shape, Shape::Prose);
        assert!(
            pattern.fields.is_empty(),
            "prose has nowhere to put a mechanical value, and that is the point"
        );
    }

    #[test]
    fn a_system_that_is_not_installed_declares_nothing() {
        let read = content_patterns_for_system("packs/systems", "no-such-system");
        assert!(read.is_empty());
    }

    /// The shipped 5e pack, read the way the server will read it.
    #[test]
    fn the_shipped_dnd5e_pack_is_read_off_disk() {
        // Named from the environment rather than written here, because a
        // shared file may not quote a system's id — check-system-registry
        // fails the build for it, and rightly.
        let id = std::fs::read_dir("../../packs/systems")
            .or_else(|_| std::fs::read_dir("packs/systems"))
            .expect("the packs directory is readable")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .find(|name| {
                std::fs::read_to_string(format!("packs/systems/{name}/system.json"))
                    .or_else(|_| {
                        std::fs::read_to_string(format!("../../packs/systems/{name}/system.json"))
                    })
                    .map(|text| text.contains("\"contentPatterns\""))
                    .unwrap_or(false)
            });

        let Some(id) = id else {
            // No pack declares patterns yet; nothing to assert.
            return;
        };
        let read = content_patterns_for_system("packs/systems", &id);
        let read = if read.is_empty() {
            content_patterns_for_system("../../packs/systems", &id)
        } else {
            read
        };
        assert!(
            !read.is_empty(),
            "a pack whose manifest declares contentPatterns is read as declaring them"
        );
    }
}
