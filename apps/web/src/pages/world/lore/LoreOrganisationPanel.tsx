import { useCallback, useEffect, useMemo, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { Link } from "react-router-dom";
import {
  addLoreTag,
  getWorldLoreEntries,
  isLoreCycleRefusal,
  moveLoreEntry,
  removeLoreTag,
} from "@/api/lore";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import {
  ancestorsOf,
  buildLoreTree,
  filterLoreEntries,
  flattenLoreTree,
  validMoveTargets,
  type LoreTreeEntry,
} from "@/pages/world/lore/loreTree";
import type { LoreEntryRecord } from "@/types/lore";

export interface LoreOrganisationPanelProps {
  worldId: string;
  entry: LoreEntryRecord;
  /** Editor-or-Owner on this entry: may move it and change its tags. */
  canEdit: boolean;
  /** Handed the entry as the server returned it after a move. */
  onEntryChanged: (entry: LoreEntryRecord) => void;
}

/**
 * Spec 031 (T072, FR-038): where this entry sits, what it is labelled, and
 * the way back to everything else.
 *
 * # Why one panel and not three
 *
 * The playtest complaint was that lore is a flat list. A breadcrumb, a
 * move control and a tag box are three answers to one question — "where is
 * this and how do I get back to the rest" — and splitting them across the
 * page would leave the reader assembling the tree in their head, which is
 * the thing they were already doing.
 *
 * # Why the move list is filtered and the server still refuses
 *
 * `validMoveTargets` hides the entry's own descendants, because offering a
 * choice that cannot work is a bad menu. It is not the enforcement: two
 * Game Masters can each pick a target that is legal against the tree they
 * are looking at and form a loop between them. The server settles that in
 * the write and answers `LORE_CYCLE`, which this surface reports as what it
 * is rather than as a malfunction (Constitution Principle III).
 */
export function LoreOrganisationPanel({
  worldId,
  entry,
  canEdit,
  onEntryChanged,
}: LoreOrganisationPanelProps) {
  const [siblings, setSiblings] = useState<LoreEntryRecord[] | null>(null);
  const [tags, setTags] = useState<string[]>(entry.tags);
  const [draftTag, setDraftTag] = useState("");
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(
    () => getWorldLoreEntries(worldId).then(setSiblings),
    [worldId],
  );

  useEffect(() => {
    let active = true;
    getWorldLoreEntries(worldId)
      .then((entries) => {
        if (active) {
          setSiblings(entries);
        }
      })
      .catch(() => {
        // A tree that failed to load is a missing convenience, not a broken
        // page: the entry itself is already on screen and readable.
        if (active) {
          setSiblings([]);
        }
      });
    return () => {
      active = false;
    };
  }, [worldId]);

  // Whatever the server last said about this entry's tags wins over the
  // optimistic copy above. Done during render rather than in an effect: it
  // is state derived from a prop, and an effect would commit one render
  // showing the previous entry's tags under the new entry's title.
  useResetOnChange(`${entry.id}|${entry.tags.join("\u0000")}`, () => {
    setTags(entry.tags);
  });

  // The list the server returned, but with this entry's live parent and tags:
  // after a move, `siblings` is one refetch behind, and a breadcrumb that
  // lags the move it just performed reads as the move having failed.
  const entries = useMemo<LoreEntryRecord[]>(() => {
    const rest = (siblings ?? []).filter((row) => row.id !== entry.id);
    return [...rest, { ...entry, tags }];
  }, [siblings, entry, tags]);

  const trail = ancestorsOf<LoreTreeEntry>(entries, entry.id);
  const targets = validMoveTargets(entries, entry.id);
  const visible = flattenLoreTree(
    buildLoreTree(filterLoreEntries(entries, query)),
  );

  const handleMove = async (parentId: string | null) => {
    setBusy(true);
    setError(null);
    try {
      const moved = await moveLoreEntry(entry.id, parentId);
      onEntryChanged(moved);
      await reload();
    } catch (err) {
      setError(
        isLoreCycleRefusal(err)
          ? "That would file this entry inside itself. It hasn't moved."
          : err instanceof Error
            ? err.message
            : "Failed to move this entry",
      );
    } finally {
      setBusy(false);
    }
  };

  const handleAddTag = async () => {
    if (!draftTag.trim()) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      // The server's answer replaces the local list rather than being
      // appended to it — it is the authority on what the tag was normalised
      // to, and on whether it was already there.
      setTags(await addLoreTag(entry.id, draftTag));
      setDraftTag("");
      await reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to add that tag");
    } finally {
      setBusy(false);
    }
  };

  const handleRemoveTag = async (tag: string) => {
    setBusy(true);
    setError(null);
    try {
      setTags(await removeLoreTag(entry.id, tag));
      await reload();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to remove that tag",
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card className="grid gap-4 p-4" data-testid="lore-organisation">
      <nav
        aria-label="Lore breadcrumb"
        className="flex flex-wrap items-center gap-1 text-sm"
        data-testid="lore-breadcrumb"
      >
        <span className="text-muted-foreground">Lore</span>
        {trail.map((ancestor) => (
          <span key={ancestor.id} className="flex items-center gap-1">
            <span className="text-muted-foreground">/</span>
            <Link
              to={`/world/${worldId}/lore/${ancestor.slug}/view`}
              className="text-primary hover:underline"
            >
              {ancestor.title}
            </Link>
          </span>
        ))}
        <span className="text-muted-foreground">/</span>
        <span className="font-medium">{entry.title}</span>
      </nav>

      {canEdit ? (
        <Field label="Files under" htmlFor="lore-parent-select">
          <select
            id="lore-parent-select"
            data-testid="lore-parent-select"
            value={entry.parentId ?? ""}
            disabled={busy}
            onChange={(event) => void handleMove(event.target.value || null)}
            className="h-8 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
          >
            <option value="">Top level</option>
            {targets.map((target) => (
              <option key={target.id} value={target.id}>
                {target.title}
              </option>
            ))}
          </select>
        </Field>
      ) : null}

      <div className="grid gap-2" data-testid="lore-tags">
        <h2 className="text-sm font-semibold tracking-wide text-muted-foreground uppercase">
          Tags
        </h2>
        {tags.length === 0 ? (
          <p className="text-sm text-muted-foreground italic">
            No tags on this entry yet.
          </p>
        ) : (
          <ul className="flex flex-wrap gap-2">
            {tags.map((tag) => (
              <li
                key={tag}
                data-testid={`lore-tag-${tag}`}
                className="flex items-center gap-1 rounded-full border border-border px-2.5 py-0.5 text-xs"
              >
                {tag}
                {canEdit ? (
                  <button
                    type="button"
                    aria-label={`Remove tag ${tag}`}
                    data-testid={`lore-tag-remove-${tag}`}
                    disabled={busy}
                    className="text-muted-foreground hover:text-danger"
                    onClick={() => void handleRemoveTag(tag)}
                  >
                    ×
                  </button>
                ) : null}
              </li>
            ))}
          </ul>
        )}
        {canEdit ? (
          <div className="flex items-end gap-2">
            <Field label="New tag" htmlFor="lore-tag-input">
              <Input
                id="lore-tag-input"
                data-testid="lore-tag-input"
                value={draftTag}
                disabled={busy}
                placeholder="ancient ruins"
                onChange={(e) => setDraftTag(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    // Enter in a lone text box submits the surrounding form
                    // elsewhere on this page — the entry's own save. Tagging
                    // must not be able to trigger that by accident.
                    e.preventDefault();
                    void handleAddTag();
                  }
                }}
              />
            </Field>
            <Button
              variant="secondary"
              size="sm"
              data-testid="lore-add-tag-button"
              disabled={busy || !draftTag.trim()}
              onClick={() => void handleAddTag()}
            >
              Add tag
            </Button>
          </div>
        ) : null}
      </div>

      {error ? <StatusBadge variant="danger">{error}</StatusBadge> : null}

      <div className="grid gap-2" data-testid="lore-tree">
        <h2 className="text-sm font-semibold tracking-wide text-muted-foreground uppercase">
          All lore
        </h2>
        <Field label="Find by name or tag" htmlFor="lore-tree-filter">
          <Input
            id="lore-tree-filter"
            data-testid="lore-tree-filter"
            value={query}
            placeholder="ruins, or #ruins for tags only"
            onChange={(e) => setQuery(e.target.value)}
          />
        </Field>
        {visible.length === 0 ? (
          <p className="text-sm text-muted-foreground italic">
            Nothing here matches that.
          </p>
        ) : (
          <ul className="grid gap-1">
            {visible.map((node) => (
              <li
                key={node.entry.id}
                data-testid={`lore-tree-row-${node.entry.id}`}
                // Indentation is the tree: depth is a number the layout
                // reads, so a node never has to know its own ancestry.
                style={{ paddingLeft: `${node.depth}rem` }}
              >
                <Link
                  to={`/world/${worldId}/lore/${node.entry.slug}/view`}
                  className={
                    node.entry.id === entry.id
                      ? "text-sm font-semibold"
                      : "text-sm text-primary hover:underline"
                  }
                >
                  {node.entry.title}
                </Link>
                {node.entry.tags.length > 0 ? (
                  <span className="ml-2 text-xs text-muted-foreground">
                    {node.entry.tags.map((tag) => `#${tag}`).join(" ")}
                  </span>
                ) : null}
              </li>
            ))}
          </ul>
        )}
      </div>
    </Card>
  );
}
