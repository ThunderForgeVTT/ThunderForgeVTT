//! Spec 039 T042 and ADR-079: the adoption record is read by a takedown and by
//! nothing else.
//!
//! ADR-069 decided a collection share is not a "centralized public repository"
//! partly because nothing on the instance could answer "what came from this".
//! ADR-079 added a table that could, and was accepted on one condition: no
//! user, no administrator and no API ever reads it. These two tests are that
//! condition.
//!
//! 1. **Only `moderation::reach` names the table.** The source of both crates
//!    is walked, and a file outside the allowlist that names it fails here — a
//!    resolver, a route, an admin listing, a debug query "just for now".
//! 2. **The schema exposes nothing shaped like it.** No type, field or argument
//!    that speaks of adoption, provenance, or where a copy came from.
//!
//! If one of these fails, the thing you just wrote is the enumerability
//! ADR-069's determination depends on not existing. That is not a test to
//! update; it is a decision for the accountable owner, and a new ADR.

use std::path::Path;

const TABLE: &str = "content_adoptions";

/// Relative to the server crate's `src/`: Diesel's declaration, the one reader
/// and writer, its tests, and this file.
const ALLOWED: &[&str] = &[
    "schema.rs",
    "moderation/reach.rs",
    "moderation/reach_tests.rs",
    "graphql/adoption_surface_tests.rs",
];

fn files_naming_the_table(root: &Path, dir: &Path, prefix: &str, found: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("read source directory") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            files_naming_the_table(root, &path, prefix, found);
            continue;
        }
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("read source file");
        if source.contains(TABLE) {
            let relative = path
                .strip_prefix(root)
                .expect("under the root")
                .to_string_lossy()
                .replace('\\', "/");
            found.push(format!("{prefix}{relative}"));
        }
    }
}

#[test]
fn only_reach_names_the_adoption_table() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let server = manifest.join("src");
    let mut found = Vec::new();
    files_naming_the_table(&server, &server, "", &mut found);

    // The binary crate mounts the REST routes. A route reading the table would
    // live there, so it is walked too.
    let app = manifest.join("../app/src");
    if app.is_dir() {
        files_naming_the_table(&app, &app, "app:", &mut found);
    }

    let strays: Vec<&String> = found
        .iter()
        .filter(|file| !ALLOWED.contains(&file.as_str()))
        .collect();
    assert!(
        strays.is_empty(),
        "`{TABLE}` is named outside `moderation/reach.rs`: {strays:?}. Nothing but a \
         takedown's walk may read it (ADR-079) — no query, field, route or admin \
         listing, in either direction. Call into `moderation::reach` instead.",
    );

    // And the allowlist is not stale: the reader really is where it says.
    assert!(
        found.iter().any(|file| file == "moderation/reach.rs"),
        "the walk found no reader at all — has the table been renamed?",
    );
}

#[test]
fn the_schema_exposes_nothing_shaped_like_an_adoption() {
    let sdl = async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .finish()
    .sdl()
    .to_lowercase();

    // Not "provenance": the schema already uses it, honestly, for who granted
    // an authoring tool and where a legal enquiry came from — and this caught
    // its own author's doc comment ("an adopted copy") on the first run, which
    // is the kind of strictness worth keeping for the words that remain.
    for word in ["adoption", "adopted", "adopter", "copiedfrom"] {
        assert!(
            !sdl.contains(word),
            "the schema mentions `{word}`. The adoption record is readable by no \
             user, no administrator and no API (ADR-079); a field that exposes it, \
             or anything derived from it, is the listing ADR-069 relied on not \
             existing.",
        );
    }
}
