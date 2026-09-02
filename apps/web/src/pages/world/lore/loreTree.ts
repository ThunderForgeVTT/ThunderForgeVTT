/**
 * Spec 031 (FR-038): turning a flat list of lore entries into the tree the
 * Game Master filed them in, and finding one again by name or by tag.
 *
 * Kept out of the component for the reason `playerFilter.ts` is: the web
 * suite runs in `node`, and the shape of a tree — which node hangs off
 * which, what a filter leaves standing, which parents a move may legally
 * offer — is the part that can actually be wrong. The component owns the
 * clicking; this owns the answer.
 *
 * Everything here is keyed by entry **id**, never by slug. Slugs are what
 * the URL happens to use today and are scheduled to stop being identity;
 * a tree keyed on them would have to be rewritten when that lands.
 */

export interface LoreTreeEntry {
  id: string;
  title: string;
  slug: string;
  parentId: string | null;
  tags: string[];
}

export interface LoreTreeNode<T extends LoreTreeEntry = LoreTreeEntry> {
  entry: T;
  depth: number;
  children: LoreTreeNode<T>[];
}

/** Titles compare the way a person reads them, not the way bytes sort. */
function byTitle(a: LoreTreeEntry, b: LoreTreeEntry): number {
  return a.title.localeCompare(b.title, undefined, { sensitivity: "base" });
}

/**
 * Builds the forest of roots from a flat list.
 *
 * An entry whose `parentId` names something not in the list is treated as a
 * root rather than dropped. That is not defensive padding: a parent can be
 * missing for an ordinary reason — it is under a DMCA placeholder, or the
 * caller passed a filtered subset — and an entry that vanishes from the tree
 * because of its *parent's* state is an entry nobody can find or fix.
 *
 * A cycle cannot reach here from the server, which refuses to write one, but
 * a walk that trusts that and is wrong hangs the browser. So descent is
 * bounded by a visited set: any node reached twice is simply not descended
 * into again.
 */
export function buildLoreTree<T extends LoreTreeEntry>(
  entries: readonly T[],
): LoreTreeNode<T>[] {
  const byId = new Map(entries.map((entry) => [entry.id, entry]));
  const childrenOf = new Map<string | null, T[]>();

  for (const entry of entries) {
    const key =
      entry.parentId && byId.has(entry.parentId) ? entry.parentId : null;
    const siblings = childrenOf.get(key);
    if (siblings) {
      siblings.push(entry);
    } else {
      childrenOf.set(key, [entry]);
    }
  }

  const visited = new Set<string>();
  const descend = (key: string | null, depth: number): LoreTreeNode<T>[] =>
    [...(childrenOf.get(key) ?? [])].sort(byTitle).flatMap((entry) => {
      if (visited.has(entry.id)) {
        return [];
      }
      visited.add(entry.id);
      return [{ entry, depth, children: descend(entry.id, depth + 1) }];
    });

  return descend(null, 0);
}

/** The tree flattened back into rows, parents before their children. */
export function flattenLoreTree<T extends LoreTreeEntry>(
  nodes: readonly LoreTreeNode<T>[],
): LoreTreeNode<T>[] {
  return nodes.flatMap((node) => [node, ...flattenLoreTree(node.children)]);
}

/**
 * The entries between the root and `entryId`, root first, excluding the entry
 * itself — the breadcrumb.
 */
export function ancestorsOf<T extends LoreTreeEntry>(
  entries: readonly T[],
  entryId: string,
): T[] {
  const byId = new Map(entries.map((entry) => [entry.id, entry]));
  const path: T[] = [];
  const seen = new Set<string>([entryId]);

  let cursor = byId.get(entryId)?.parentId ?? null;
  while (cursor && !seen.has(cursor)) {
    seen.add(cursor);
    const parent = byId.get(cursor);
    if (!parent) {
      break;
    }
    path.unshift(parent);
    cursor = parent.parentId;
  }
  return path;
}

/** `entryId` and everything beneath it. */
export function descendantIdsOf(
  entries: readonly LoreTreeEntry[],
  entryId: string,
): Set<string> {
  const found = new Set<string>([entryId]);
  // Repeated sweeps rather than recursion: the list arrives in no particular
  // order, so a child can be seen before its parent has been marked.
  let grew = true;
  while (grew) {
    grew = false;
    for (const entry of entries) {
      if (entry.parentId && found.has(entry.parentId) && !found.has(entry.id)) {
        found.add(entry.id);
        grew = true;
      }
    }
  }
  return found;
}

/**
 * The entries an entry may legally be moved under.
 *
 * Itself and its own descendants are excluded — those are exactly the moves
 * the server refuses with `LORE_CYCLE`. Offering them and then reporting the
 * refusal would be a menu of choices that cannot work; the server still
 * refuses, because two people choosing at the same time is a case no menu
 * can see (Constitution Principle III).
 */
export function validMoveTargets<T extends LoreTreeEntry>(
  entries: readonly T[],
  entryId: string,
): T[] {
  const forbidden = descendantIdsOf(entries, entryId);
  return entries.filter((entry) => !forbidden.has(entry.id)).sort(byTitle);
}

/** The same normalisation the server applies before it stores a tag. */
export function normaliseTag(raw: string): string {
  return raw.trim().replace(/\s+/g, " ").toLowerCase();
}

/**
 * Whether one entry answers the search box.
 *
 * Title and tags together, because "find it by either" is the whole point of
 * having both: a Game Master looking for the ruined keep may remember its
 * name or may only remember that they tagged it "ruins". A bare `#` prefix
 * narrows the search to tags, for the case where a word is both.
 */
export function matchesLoreQuery(entry: LoreTreeEntry, query: string): boolean {
  const trimmed = query.trim().toLowerCase();
  if (!trimmed) {
    return true;
  }
  if (trimmed.startsWith("#")) {
    const needle = normaliseTag(trimmed.slice(1));
    return needle
      ? entry.tags.some((tag) => tag.includes(needle))
      : entry.tags.length > 0;
  }
  return (
    entry.title.toLowerCase().includes(trimmed) ||
    entry.tags.some((tag) => tag.includes(trimmed))
  );
}

/**
 * The tree a search leaves standing: every match, plus the ancestors that
 * lead to it.
 *
 * The ancestors are kept even when they do not match. A hit shown without
 * the branch it hangs from is a row with no context, and worse, removing an
 * unmatched parent would silently promote its matching children to the root
 * — the filter would appear to have *moved* things.
 */
export function filterLoreEntries<T extends LoreTreeEntry>(
  entries: readonly T[],
  query: string,
): T[] {
  if (!query.trim()) {
    return [...entries];
  }
  const keep = new Set<string>();
  for (const entry of entries) {
    if (matchesLoreQuery(entry, query)) {
      keep.add(entry.id);
      for (const ancestor of ancestorsOf(entries, entry.id)) {
        keep.add(ancestor.id);
      }
    }
  }
  return entries.filter((entry) => keep.has(entry.id));
}

/** Every tag in use, normalised, de-duplicated and alphabetical. */
export function allTagsOf(entries: readonly LoreTreeEntry[]): string[] {
  return [...new Set(entries.flatMap((entry) => entry.tags))].sort((a, b) =>
    a.localeCompare(b),
  );
}
