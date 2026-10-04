//! Spec 034 (research.md R7, FR-008): a lore entry's tree position and title
//! become a repository path.
//!
//! **The path is a label; the header's `id` is the key.** Nothing in this
//! module may ever be read back as an identifier — `document::parse` recovers
//! the id from the front matter, and FR-027 says the same from the import
//! side. That single rule is what makes every hard case below *resolvable*
//! rather than merely difficult: a title that transliterates to nothing, two
//! siblings differing only by case or accent, a tree deeper than a filesystem
//! wants — each is a label collision, and a label collision is answered by
//! renaming, which is free when nothing matches on the name.
//!
//! Two properties this module owes its callers, both load-bearing:
//!
//! * **Every entry lands at exactly one path.** No entry is skipped for being
//!   awkward to name; a mirror missing a file because its title was three
//!   bullet characters is a silent data loss, and FR-013's posture is that
//!   losses are declared, never quiet.
//! * **Disambiguation is stable.** The suffix is derived from the entry's own
//!   identifier, never from its ordinal among its siblings. An ordinal (`-2`,
//!   `-3`, as `markdown::slug` uses for in-app URLs) is correct there and wrong
//!   here: deleting the second of three siblings would renumber the third, and
//!   the mirror would show a rename of a file whose entry nobody touched. With
//!   an id-derived suffix, adding or removing a sibling moves nothing else, and
//!   running the export twice over unchanged data produces a byte-identical
//!   tree — which is what lets FR-016 claim a commit means an edit happened.
//!
//! Pure: no database, no filesystem, no clock. The caller fetches the entries
//! and hands the whole set in at once, because collisions are a property of the
//! set and cannot be decided one entry at a time.

use std::collections::{BTreeMap, HashMap, HashSet};

use uuid::Uuid;

/// Directory levels permitted below the world's synchronisation directory.
///
/// Nothing in the lore tree is expected to approach this; the cap exists so a
/// pathological tree (or one with a parent cycle a future migration could
/// introduce) cannot generate a path a filesystem or a Windows clone refuses.
/// An entry deeper than this is placed in the deepest directory that fits
/// rather than dropped.
pub const MAX_DEPTH: usize = 8;

/// Bytes permitted in a single path component. Common filesystems allow 255;
/// this is well inside that and keeps a directory listing readable, which is
/// the whole point of deriving paths from titles (FR-008).
pub const MAX_COMPONENT_BYTES: usize = 64;

/// Bytes permitted in the whole path, relative to the synchronisation
/// directory. Windows' classic 260-character limit applies to the *absolute*
/// path, so the clone's own location has to fit too; 200 leaves room for it.
pub const MAX_PATH_BYTES: usize = 200;

/// The shortest file name the allocator will squeeze an entry into before it
/// gives up on descending: `entry-0189a06d.md`.
const MIN_FILE_BYTES: usize = 20;

/// Characters of the entry's identifier used as a disambiguating suffix. Eight
/// hex digits over the entries of one world is a collision risk far below the
/// point of worrying, and the tie-break below handles it regardless.
const SUFFIX_HEX_CHARS: usize = 8;

/// Names Windows refuses to create at any extension, inherited from DOS device
/// files. `slug` happily produces `con` from the title "Con", and a clone on
/// Windows would then fail to check out with an error naming nothing useful.
const RESERVED_STEMS: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// One lore entry, reduced to the two things a path is made of.
///
/// `parent_id` naming an entry absent from the set — excluded by FR-015's
/// moderation filter, say — makes this entry a root rather than an error. The
/// alternative is refusing to export a subtree because its parent was taken
/// down, and FR-015 explicitly requires the exclusion of one entry not to block
/// the others.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryNode {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub title: String,
}

/// Why an entry did not get the path its title implies.
///
/// Surfaced so the caller can write the `path_disambiguated` fidelity note the
/// contract requires: a Game Master looking at `entry-0189a06d.md` deserves to
/// be told the title had no transliterable characters, not left to guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisambiguationReason {
    /// The title transliterated to nothing usable — empty, whitespace only, or
    /// made entirely of punctuation `slug` drops. Emoji and CJK do *not* land
    /// here: `deunicode` has names for them, so "🐉🔥" becomes `dragon-fire`.
    TitleNormalisedToNothing,
    /// A sibling already claimed this name. Includes siblings differing only by
    /// case or accent, which fold together deliberately: a case-insensitive
    /// filesystem would otherwise clobber one with the other on checkout.
    SiblingCollision,
    /// The name would have exceeded a component or total path budget and was
    /// shortened.
    NameTooLong,
    /// The tree is deeper than [`MAX_DEPTH`] (or the path budget) allows, so
    /// the entry sits above where its ancestry says it belongs.
    DepthExceeded,
    /// The name a filesystem reserves for something other than a file.
    ReservedName,
}

/// Where one entry's file goes, and whether that answer needed forcing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedPath {
    /// Path to the entry's file, relative to the synchronisation directory,
    /// `/`-separated and always ending in `.md`.
    pub path: String,
    /// Directory this entry's children go in — `path` without its `.md`. Held
    /// rather than recomputed so a caller never has to re-derive the rule.
    pub directory: String,
    /// Every reason this path differs from the one the title implies, in the
    /// order the allocator hit them. Empty when the title mapped cleanly.
    pub reasons: Vec<DisambiguationReason>,
}

impl AssignedPath {
    /// Whether a `path_disambiguated` fidelity note is owed for this entry.
    pub fn was_disambiguated(&self) -> bool {
        !self.reasons.is_empty()
    }
}

/// Maps every entry in `entries` to exactly one path.
///
/// The whole set goes in at once because a collision is a fact about a set.
/// The result is keyed by id and ordered by it, so a caller iterating the map
/// walks the same order on every run.
///
/// Determinism, precisely: the output depends only on the *contents* of
/// `entries`, never on their order. Entries are processed shallowest first and
/// then by id — which for the UUIDv7 identifiers this codebase mints is
/// creation order — so the earliest-created entry of a colliding pair keeps the
/// clean name and later arrivals take the suffix. A sibling appearing or
/// disappearing therefore never renames anyone else's file.
///
/// A duplicated id in `entries` is resolved by keeping the first occurrence;
/// the caller is reading rows keyed by a primary key, so this is defensive
/// rather than expected.
pub fn map_entry_paths(entries: &[EntryNode]) -> BTreeMap<Uuid, AssignedPath> {
    let mut nodes: Vec<&EntryNode> = Vec::with_capacity(entries.len());
    let mut seen: HashSet<Uuid> = HashSet::with_capacity(entries.len());
    for entry in entries {
        if seen.insert(entry.id) {
            nodes.push(entry);
        }
    }

    let by_id: HashMap<Uuid, &EntryNode> = nodes.iter().map(|node| (node.id, *node)).collect();
    let effective_parents = resolve_parents(&nodes, &by_id);

    let mut depths: HashMap<Uuid, usize> = HashMap::with_capacity(nodes.len());
    for node in &nodes {
        depth_of(node.id, &effective_parents, &mut depths);
    }

    // Shallowest first so a parent's directory is always known before its
    // children need it; then by id, which is what makes the result independent
    // of the caller's ordering.
    let mut order: Vec<&EntryNode> = nodes.clone();
    order.sort_by_key(|node| (depths.get(&node.id).copied().unwrap_or(0), node.id));

    let mut assigned: BTreeMap<Uuid, AssignedPath> = BTreeMap::new();
    // Names already taken in each directory. A stem is claimed once and covers
    // both `<stem>.md` and the `<stem>/` its children live in, so a file and a
    // directory can never disagree about who owns a name.
    let mut claims: HashMap<String, HashSet<String>> = HashMap::new();

    for node in order {
        let mut reasons = Vec::new();

        let parent_directory = effective_parents
            .get(&node.id)
            .copied()
            .flatten()
            .and_then(|parent_id| assigned.get(&parent_id))
            .map(|parent| parent.directory.clone())
            .unwrap_or_default();

        let parent_depth = parent_directory
            .split('/')
            .filter(|segment| !segment.is_empty())
            .count();

        // Descend only if the level exists *and* something nameable still fits
        // inside the path budget once we are down there.
        let directory = if parent_directory.is_empty() {
            String::new()
        } else if parent_depth <= MAX_DEPTH
            && parent_directory.len() + 1 + MIN_FILE_BYTES <= MAX_PATH_BYTES
        {
            parent_directory.clone()
        } else {
            reasons.push(DisambiguationReason::DepthExceeded);
            climb_until_it_fits(&parent_directory)
        };

        let prefix_len = if directory.is_empty() {
            0
        } else {
            directory.len() + 1
        };
        // ".md" is three bytes, and every entry gets one.
        let stem_budget = MAX_PATH_BYTES.saturating_sub(prefix_len + 3);

        let mut base = slug::slugify(&node.title);
        if base.is_empty() {
            reasons.push(DisambiguationReason::TitleNormalisedToNothing);
            base = "entry".to_string();
        }
        if base.len() > MAX_COMPONENT_BYTES.min(stem_budget) {
            reasons.push(DisambiguationReason::NameTooLong);
            base = truncate_slug(&base, MAX_COMPONENT_BYTES.min(stem_budget));
        }
        if RESERVED_STEMS.contains(&base.as_str()) {
            reasons.push(DisambiguationReason::ReservedName);
        }

        let taken = claims.entry(directory.clone()).or_default();
        // A title that named nothing, or a name the filesystem reserves, is
        // suffixed unconditionally: `entry.md` for the first such entry and
        // `entry-0189a06d.md` for the second would be two rules where one will
        // do, and the id in the name is the only thing distinguishing these
        // files anyway.
        let forced = reasons.contains(&DisambiguationReason::TitleNormalisedToNothing)
            || reasons.contains(&DisambiguationReason::ReservedName);

        let stem = if !forced && !taken.contains(&base) {
            base.clone()
        } else {
            if !forced {
                reasons.push(DisambiguationReason::SiblingCollision);
            }
            let suffix = short_id(node.id);
            let room = stem_budget.saturating_sub(suffix.len() + 1);
            let trimmed = if base.len() > room {
                if !reasons.contains(&DisambiguationReason::NameTooLong) {
                    reasons.push(DisambiguationReason::NameTooLong);
                }
                truncate_slug(&base, room)
            } else {
                base.clone()
            };
            let mut candidate = join_stem(&trimmed, &suffix);
            // Reached only by a repeated id or an eight-hex-digit collision
            // inside one directory. Vanishingly unlikely, and still resolved
            // rather than left to overwrite a sibling's file — and still inside
            // the budget, which is why the base is re-trimmed for the counter.
            let mut tie = 2usize;
            while taken.contains(&candidate) {
                let counter = tie.to_string();
                let room = stem_budget.saturating_sub(suffix.len() + counter.len() + 2);
                candidate = format!(
                    "{}-{counter}",
                    join_stem(&truncate_slug(&trimmed, room), &suffix)
                );
                tie += 1;
            }
            candidate
        };

        taken.insert(stem.clone());

        let path = if directory.is_empty() {
            format!("{stem}.md")
        } else {
            format!("{directory}/{stem}.md")
        };
        let child_directory = path.trim_end_matches(".md").to_string();

        assigned.insert(
            node.id,
            AssignedPath {
                path,
                directory: child_directory,
                reasons,
            },
        );
    }

    assigned
}

/// A relative link from the file at `from` to the file at `to`, both
/// synchronisation-directory-relative.
///
/// This is FR-012 and SC-011's mechanism: a link that resolves in a plain
/// markdown viewer over a clone, with no network and no knowledge of the
/// platform. Components produced by [`map_entry_paths`] are `[a-z0-9-]` only,
/// so the result never needs percent-encoding or angle-bracket wrapping.
pub fn relative_link(from: &str, to: &str) -> String {
    let from_dirs: Vec<&str> = from.split('/').collect();
    let from_dirs = &from_dirs[..from_dirs.len().saturating_sub(1)];
    let to_parts: Vec<&str> = to.split('/').collect();

    let shared = from_dirs
        .iter()
        .zip(to_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();
    // Never let the shared prefix swallow the target's own file name.
    let shared = shared.min(to_parts.len().saturating_sub(1));

    let mut out = String::new();
    for _ in shared..from_dirs.len() {
        out.push_str("../");
    }
    out.push_str(&to_parts[shared..].join("/"));
    if out.is_empty() {
        // Only possible if `to` is empty; a self-link is still a valid target.
        out.push_str(to_parts.last().copied().unwrap_or_default());
    }
    out
}

/// Title → a single path component, with no disambiguation and no context.
///
/// Case and accent both fold away here, and that folding is the point: it is
/// what makes "Ashen Vale", "ashen vale" and "Ashen Válé" collide *before* a
/// case-insensitive filesystem gets the chance to collide them destructively
/// during a checkout. An empty result means the title carried nothing a path
/// can be made of, and the caller must supply a name.
pub fn slugify_component(title: &str) -> String {
    let slug = slug::slugify(title);
    truncate_slug(&slug, MAX_COMPONENT_BYTES)
}

/// Drops trailing directory levels until a minimal file name fits the budget.
fn climb_until_it_fits(directory: &str) -> String {
    let mut parts: Vec<&str> = directory.split('/').filter(|s| !s.is_empty()).collect();
    while !parts.is_empty() {
        let joined = parts.join("/");
        if parts.len() <= MAX_DEPTH && joined.len() + 1 + MIN_FILE_BYTES <= MAX_PATH_BYTES {
            return joined;
        }
        parts.pop();
    }
    String::new()
}

/// Each entry's parent, or `None` where following it would leave the set or go
/// round in a circle. A cycle cannot arise from the current schema; it is
/// handled because the alternative is a pure function that hangs.
fn resolve_parents(
    nodes: &[&EntryNode],
    by_id: &HashMap<Uuid, &EntryNode>,
) -> HashMap<Uuid, Option<Uuid>> {
    let mut parents = HashMap::with_capacity(nodes.len());
    for node in nodes {
        let parent = node.parent_id.filter(|parent_id| {
            *parent_id != node.id && by_id.contains_key(parent_id) && {
                // Walk up; a chain longer than the set cannot be acyclic.
                let mut cursor = *parent_id;
                let mut steps = 0usize;
                loop {
                    match by_id.get(&cursor).and_then(|entry| entry.parent_id) {
                        Some(next) if next == node.id => break false,
                        Some(next) if by_id.contains_key(&next) => {
                            cursor = next;
                            steps += 1;
                            if steps > nodes.len() {
                                break false;
                            }
                        }
                        _ => break true,
                    }
                }
            }
        });
        parents.insert(node.id, parent);
    }
    parents
}

fn depth_of(
    id: Uuid,
    parents: &HashMap<Uuid, Option<Uuid>>,
    memo: &mut HashMap<Uuid, usize>,
) -> usize {
    if let Some(depth) = memo.get(&id) {
        return *depth;
    }
    // Iterative so a long chain cannot overflow the stack.
    let mut chain = Vec::new();
    let mut cursor = Some(id);
    let mut base = 0usize;
    while let Some(current) = cursor {
        if let Some(depth) = memo.get(&current) {
            base = *depth;
            break;
        }
        chain.push(current);
        cursor = parents.get(&current).copied().flatten();
    }
    for (offset, node) in chain.iter().rev().enumerate() {
        memo.insert(*node, base + offset);
    }
    memo.get(&id).copied().unwrap_or(0)
}

fn short_id(id: Uuid) -> String {
    id.simple()
        .to_string()
        .chars()
        .take(SUFFIX_HEX_CHARS)
        .collect()
}

fn join_stem(base: &str, suffix: &str) -> String {
    if base.is_empty() {
        suffix.to_string()
    } else {
        format!("{base}-{suffix}")
    }
}

/// Truncates on a character boundary and never leaves a dangling separator, so
/// the result is still a well-formed slug rather than `ashen-`.
fn truncate_slug(slug: &str, max_bytes: usize) -> String {
    if slug.len() <= max_bytes {
        return slug.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !slug.is_char_boundary(end) {
        end -= 1;
    }
    slug[..end].trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test identifiers with distinct *leading* hex digits, because the
    /// disambiguating suffix is taken from the front of the identifier — a
    /// naive `Uuid::from_u128(1)` would give every entry the same suffix and
    /// quietly test the tie-break instead of the rule.
    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n << 100 | n)
    }

    fn node(n: u128, parent: Option<u128>, title: &str) -> EntryNode {
        EntryNode {
            id: id(n),
            parent_id: parent.map(id),
            title: title.to_string(),
        }
    }

    fn path_of(map: &BTreeMap<Uuid, AssignedPath>, n: u128) -> String {
        map.get(&id(n)).expect("entry assigned a path").path.clone()
    }

    #[test]
    fn mirrors_the_tree_in_the_directory_structure() {
        let entries = vec![
            node(1, None, "Westeros"),
            node(2, Some(1), "The Red Keep"),
            node(3, Some(2), "The Black Cells"),
        ];
        let map = map_entry_paths(&entries);

        assert_eq!(path_of(&map, 1), "westeros.md");
        assert_eq!(path_of(&map, 2), "westeros/the-red-keep.md");
        assert_eq!(path_of(&map, 3), "westeros/the-red-keep/the-black-cells.md");
        assert!(map.values().all(|assigned| !assigned.was_disambiguated()));
    }

    #[test]
    fn every_entry_lands_at_exactly_one_path() {
        let entries = vec![
            node(1, None, "•—•"),
            node(2, None, "..."),
            node(3, None, ""),
            node(4, None, "   "),
            node(5, None, "Ashen Vale"),
        ];
        let map = map_entry_paths(&entries);

        assert_eq!(map.len(), entries.len());
        let paths: HashSet<&String> = map.values().map(|assigned| &assigned.path).collect();
        assert_eq!(paths.len(), entries.len(), "paths must be distinct");
    }

    #[test]
    fn a_title_that_normalises_to_nothing_is_named_from_its_id() {
        for title in ["", "   ", "...", "!!!", "。、", "•—•"] {
            let entries = vec![node(7, None, title)];
            let map = map_entry_paths(&entries);
            let assigned = map.get(&id(7)).expect("assigned");

            assert!(
                assigned.path.starts_with("entry-"),
                "{title:?} produced {}",
                assigned.path
            );
            assert!(assigned.path.ends_with(".md"));
            assert!(
                assigned
                    .reasons
                    .contains(&DisambiguationReason::TitleNormalisedToNothing),
                "{title:?} did not report why it was renamed"
            );
        }
    }

    #[test]
    fn non_latin_titles_transliterate_rather_than_vanishing() {
        let entries = vec![
            node(1, None, "Древний Лес"),
            node(2, None, "Ελληνικά"),
            node(3, None, "Ræveskoven"),
        ];
        let map = map_entry_paths(&entries);

        assert_eq!(path_of(&map, 1), "drevnii-les.md");
        assert_eq!(path_of(&map, 2), "ellenika.md");
        assert_eq!(path_of(&map, 3), "raeveskoven.md");
        assert!(map.values().all(|assigned| !assigned.was_disambiguated()));
    }

    #[test]
    fn emoji_and_cjk_transliterate_rather_than_falling_back_to_an_id() {
        // `slug` (via `deunicode`) has names for these, and a readable name is
        // better than an identifier even when it is a surprising one. Recorded
        // as a test because it is the behaviour a Game Master will actually
        // see, and because a future dependency bump changing it should fail
        // here rather than silently rename half a mirror.
        let entries = vec![
            node(1, None, "🐉🔥"),
            node(2, None, "龍の巣"),
            node(3, None, "☃"),
        ];
        let map = map_entry_paths(&entries);

        assert_eq!(path_of(&map, 1), "dragon-fire.md");
        assert_eq!(path_of(&map, 2), "long-nochao.md");
        assert_eq!(path_of(&map, 3), "snowman.md");
        assert!(map.values().all(|assigned| !assigned.was_disambiguated()));
    }

    #[test]
    fn siblings_differing_only_by_case_do_not_collide() {
        let entries = vec![
            node(1, None, "Ashen Vale"),
            node(2, None, "ASHEN VALE"),
            node(3, None, "ashen vale"),
        ];
        let map = map_entry_paths(&entries);

        assert_eq!(path_of(&map, 1), "ashen-vale.md");
        assert_ne!(path_of(&map, 2), path_of(&map, 1));
        assert_ne!(path_of(&map, 3), path_of(&map, 1));
        assert_ne!(path_of(&map, 3), path_of(&map, 2));
        for n in [2, 3] {
            assert!(
                map[&id(n)]
                    .reasons
                    .contains(&DisambiguationReason::SiblingCollision)
            );
        }
    }

    #[test]
    fn siblings_differing_only_by_accent_do_not_collide() {
        let entries = vec![node(1, None, "Ashen Vale"), node(2, None, "Áshen Vàle")];
        let map = map_entry_paths(&entries);

        assert_eq!(path_of(&map, 1), "ashen-vale.md");
        assert_ne!(path_of(&map, 2), "ashen-vale.md");
        assert!(
            map[&id(2)]
                .reasons
                .contains(&DisambiguationReason::SiblingCollision)
        );
    }

    #[test]
    fn a_collision_only_renames_the_later_entry() {
        let entries = vec![node(1, None, "Ashen Vale"), node(2, None, "Ashen Vale")];
        let map = map_entry_paths(&entries);

        assert_eq!(path_of(&map, 1), "ashen-vale.md");
        assert_eq!(
            path_of(&map, 2),
            format!("ashen-vale-{}.md", short_id(id(2))),
            "the suffix comes from the entry's own id, not its ordinal"
        );
    }

    #[test]
    fn the_same_title_in_two_directories_is_not_a_collision() {
        let entries = vec![
            node(1, None, "North"),
            node(2, None, "South"),
            node(3, Some(1), "The Gate"),
            node(4, Some(2), "The Gate"),
        ];
        let map = map_entry_paths(&entries);

        assert_eq!(path_of(&map, 3), "north/the-gate.md");
        assert_eq!(path_of(&map, 4), "south/the-gate.md");
        assert!(map.values().all(|assigned| !assigned.was_disambiguated()));
    }

    #[test]
    fn running_twice_renames_nothing() {
        let entries = vec![
            node(1, None, "Ashen Vale"),
            node(2, None, "ashen vale"),
            node(3, Some(1), "🐉"),
            node(4, Some(1), "The Gate"),
        ];
        let first = map_entry_paths(&entries);
        let second = map_entry_paths(&entries);
        assert_eq!(first, second);
    }

    #[test]
    fn the_input_order_does_not_change_the_answer() {
        let forward = vec![
            node(1, None, "Ashen Vale"),
            node(2, None, "ashen vale"),
            node(3, Some(2), "Deep"),
        ];
        let reversed: Vec<EntryNode> = forward.iter().rev().cloned().collect();
        assert_eq!(map_entry_paths(&forward), map_entry_paths(&reversed));
    }

    #[test]
    fn adding_a_sibling_does_not_move_the_existing_one() {
        let before = vec![node(1, None, "Ashen Vale")];
        let after = vec![node(1, None, "Ashen Vale"), node(2, None, "Ashen Vale")];

        assert_eq!(
            map_entry_paths(&before)[&id(1)].path,
            map_entry_paths(&after)[&id(1)].path
        );
    }

    #[test]
    fn removing_a_sibling_does_not_renumber_the_survivors() {
        let all = vec![
            node(1, None, "Vale"),
            node(2, None, "Vale"),
            node(3, None, "Vale"),
        ];
        let without_middle = vec![node(1, None, "Vale"), node(3, None, "Vale")];

        assert_eq!(
            map_entry_paths(&all)[&id(3)].path,
            map_entry_paths(&without_middle)[&id(3)].path,
            "an ordinal suffix would have renamed the third entry here"
        );
    }

    #[test]
    fn excessive_depth_is_capped_and_reported() {
        let mut entries = vec![node(1, None, "Level")];
        for n in 2..=20u128 {
            entries.push(node(n, Some(n - 1), "Level"));
        }
        let map = map_entry_paths(&entries);

        let deepest = &map[&id(20)];
        let depth = deepest.path.matches('/').count();
        assert!(depth <= MAX_DEPTH, "{} was {depth} deep", deepest.path);
        assert!(
            deepest
                .reasons
                .contains(&DisambiguationReason::DepthExceeded)
        );
        // Everything past the cap still gets a distinct file.
        let paths: HashSet<&String> = map.values().map(|assigned| &assigned.path).collect();
        assert_eq!(paths.len(), entries.len());
    }

    #[test]
    fn an_over_long_title_is_truncated_within_the_component_budget() {
        let entries = vec![node(1, None, &"Chronicle of the Long March ".repeat(20))];
        let map = map_entry_paths(&entries);
        let assigned = &map[&id(1)];

        assert!(assigned.path.len() <= MAX_PATH_BYTES);
        for component in assigned.path.split('/') {
            assert!(component.len() <= MAX_COMPONENT_BYTES + 3);
        }
        assert!(!assigned.path.contains("-.md"));
        assert!(
            assigned
                .reasons
                .contains(&DisambiguationReason::NameTooLong)
        );
    }

    #[test]
    fn no_path_exceeds_the_total_budget_even_when_every_level_is_long() {
        let long = "Chronicle of the Interminable and Exhaustively Titled March";
        let mut entries = vec![node(1, None, long)];
        for n in 2..=12u128 {
            entries.push(node(n, Some(n - 1), long));
        }
        let map = map_entry_paths(&entries);

        for assigned in map.values() {
            assert!(
                assigned.path.len() <= MAX_PATH_BYTES,
                "{} is {} bytes",
                assigned.path,
                assigned.path.len()
            );
        }
        let paths: HashSet<&String> = map.values().map(|assigned| &assigned.path).collect();
        assert_eq!(paths.len(), entries.len(), "paths must stay distinct");
    }

    #[test]
    fn a_windows_reserved_name_is_suffixed() {
        let entries = vec![node(1, None, "Con"), node(2, None, "NUL")];
        let map = map_entry_paths(&entries);

        assert_ne!(path_of(&map, 1), "con.md");
        assert_ne!(path_of(&map, 2), "nul.md");
        for n in [1, 2] {
            assert!(
                map[&id(n)]
                    .reasons
                    .contains(&DisambiguationReason::ReservedName)
            );
        }
    }

    #[test]
    fn a_parent_outside_the_set_makes_a_root_rather_than_an_error() {
        // FR-015: the parent was disabled by moderation and never exported.
        let entries = vec![node(2, Some(99), "Orphan")];
        let map = map_entry_paths(&entries);

        assert_eq!(path_of(&map, 2), "orphan.md");
    }

    #[test]
    fn a_parent_cycle_terminates() {
        let entries = vec![
            node(1, Some(2), "One"),
            node(2, Some(1), "Two"),
            node(3, Some(3), "Self"),
        ];
        let map = map_entry_paths(&entries);

        assert_eq!(map.len(), 3);
        assert!(map.values().all(|assigned| assigned.path.ends_with(".md")));
    }

    #[test]
    fn a_duplicated_id_is_assigned_once() {
        let entries = vec![node(1, None, "Vale"), node(1, None, "Vale")];
        let map = map_entry_paths(&entries);

        assert_eq!(map.len(), 1);
        assert_eq!(path_of(&map, 1), "vale.md");
    }

    #[test]
    fn a_parent_file_and_its_children_directory_coexist() {
        let entries = vec![node(1, None, "Westeros"), node(2, Some(1), "The Reach")];
        let map = map_entry_paths(&entries);

        assert_eq!(map[&id(1)].path, "westeros.md");
        assert_eq!(map[&id(1)].directory, "westeros");
        assert_eq!(map[&id(2)].path, "westeros/the-reach.md");
    }

    #[test]
    fn a_disambiguated_parent_carries_its_children_with_it() {
        let entries = vec![
            node(1, None, "Vale"),
            node(2, None, "vale"),
            node(3, Some(2), "Deep"),
        ];
        let map = map_entry_paths(&entries);

        let parent_directory = map[&id(2)].directory.clone();
        assert_eq!(map[&id(3)].path, format!("{parent_directory}/deep.md"));
    }

    #[test]
    fn relative_links_resolve_between_directories() {
        assert_eq!(relative_link("a.md", "b.md"), "b.md");
        assert_eq!(relative_link("x/a.md", "x/b.md"), "b.md");
        assert_eq!(relative_link("x/a.md", "b.md"), "../b.md");
        assert_eq!(relative_link("a.md", "x/b.md"), "x/b.md");
        assert_eq!(relative_link("x/y/a.md", "x/z/b.md"), "../z/b.md");
        assert_eq!(relative_link("x/y/a.md", "p/q/b.md"), "../../p/q/b.md");
        assert_eq!(relative_link("x/a.md", "x/a/b.md"), "a/b.md");
    }

    #[test]
    fn slugify_component_folds_case_and_accent_together() {
        assert_eq!(slugify_component("Ashen Vale"), "ashen-vale");
        assert_eq!(slugify_component("ASHEN VALE"), "ashen-vale");
        assert_eq!(slugify_component("Áshen Vàle"), "ashen-vale");
        assert_eq!(slugify_component("•—•"), "");
    }

    #[test]
    fn truncation_never_leaves_a_dangling_separator() {
        assert_eq!(truncate_slug("ashen-vale", 6), "ashen");
        assert_eq!(truncate_slug("ashen-vale", 5), "ashen");
        assert_eq!(truncate_slug("ashen-vale", 40), "ashen-vale");
    }
}
