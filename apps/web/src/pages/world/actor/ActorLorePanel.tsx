import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  createLoreEntry,
  getWorldLoreEntries,
  updateLoreEntry,
} from "@/api/lore";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import {
  attachLoreEntry,
  createLoreEntryAbout,
} from "@/pages/world/actor/actorContent";
import type { LoreEntryRecord, LoreLinkSourceRecord } from "@/types/lore";

/**
 * The lore attached to a character, and the two ways to attach more (FR-039).
 *
 * # Why this replaced a read-only card
 *
 * "Linked from (lore)" already showed which entries mention this character. It
 * was a list with nothing to do: the only way onto it was to go and write an
 * entry somewhere else, which is the trip spec 031 is trying to stop a Game
 * Master making in the middle of building an NPC.
 *
 * # Why an entry can be written here but not edited here
 *
 * A title, a paragraph, and it exists and is filed against this character.
 * That is the note taken while thinking about the character. Everything the
 * lore editor is for — Markdown, images, revisions, `[[` autocomplete,
 * permissions — is still one click away on the entry itself, and duplicating
 * any of it here would be a second lore editor to keep right.
 *
 * # Why success is confirmed by looking rather than announced
 *
 * Attaching means writing a `[[Name]]` link, and the server resolves those by
 * name with lore entries taking precedence over actors. So an entry titled
 * after this character captures the link and the character gets no backlink at
 * all. The panel therefore re-reads the actor and checks the result, and says
 * plainly when the link did not land where it was aimed — the alternative is a
 * cheerful "attached" over a list that did not change.
 */

export interface ActorLorePanelProps {
  worldId: string;
  /**
   * The name a `[[link]]` has to match for the backlink to resolve here.
   *
   * The character's *name*, not its id: that is what the server resolves a
   * link against, which is also why this panel has no use for the id.
   */
  actorLabel: string;
  /** What the server currently says links here. */
  linkedFrom: LoreLinkSourceRecord[];
  canManage: boolean;
  /** Re-read the actor, so `linkedFrom` reflects what just happened. */
  onChanged: () => Promise<void>;
}

type Pending =
  | { kind: "idle" }
  | { kind: "working" }
  | { kind: "problem"; message: string }
  | { kind: "waiting"; entryId: string; title: string };

export function ActorLorePanel({
  worldId,
  actorLabel,
  linkedFrom,
  canManage,
  onChanged,
}: ActorLorePanelProps) {
  const [entries, setEntries] = useState<LoreEntryRecord[] | null>(null);
  const [writing, setWriting] = useState(false);
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [attachId, setAttachId] = useState("");
  const [pending, setPending] = useState<Pending>({ kind: "idle" });

  useEffect(() => {
    if (!canManage) {
      return;
    }
    let active = true;
    getWorldLoreEntries(worldId)
      .then((rows) => {
        if (active) setEntries(rows);
      })
      .catch(() => {
        // A world with no lore is normal, and a failed read means the attach
        // picker is empty rather than that this panel is broken.
        if (active) setEntries([]);
      });
    return () => {
      active = false;
    };
  }, [worldId, canManage]);

  const refreshAfterWrite = async (entryId: string, entryTitle: string) => {
    // The actor is re-read *before* this panel starts wondering whether the
    // link landed. Doing it the other way round shows the "did not link here"
    // note for as long as the refresh takes, on the ordinary path where it
    // did.
    await onChanged();
    setPending({ kind: "waiting", entryId, title: entryTitle });
    if (canManage) {
      getWorldLoreEntries(worldId)
        .then(setEntries)
        .catch(() => {});
    }
  };

  const write = async () => {
    if (title.trim() === "") {
      return;
    }
    setPending({ kind: "working" });
    const outcome = await createLoreEntryAbout(
      { createLoreEntry, updateLoreEntry },
      { worldId, actorLabel, title: title.trim(), content: body },
    );
    if (outcome.kind === "refused") {
      setPending({ kind: "problem", message: outcome.message });
      return;
    }
    setTitle("");
    setBody("");
    setWriting(false);
    await refreshAfterWrite(outcome.entryId, outcome.title);
  };

  const attach = async () => {
    const entry = (entries ?? []).find(
      (candidate) => candidate.id === attachId,
    );
    if (!entry) {
      return;
    }
    setPending({ kind: "working" });
    const outcome = await attachLoreEntry(
      { createLoreEntry, updateLoreEntry },
      entry,
      actorLabel,
    );
    if (outcome.kind === "refused") {
      setPending({ kind: "problem", message: outcome.message });
      return;
    }
    setAttachId("");
    await refreshAfterWrite(outcome.entryId, outcome.title);
  };

  /**
   * Whether the entry just written did not reach this actor after all.
   *
   * Derived from the refreshed `linkedFrom` at render rather than recorded
   * when the mutation returned: the mutation succeeding and the link resolving
   * *here* are two different things, and only the list can say which happened.
   */
  const unresolved =
    pending.kind === "waiting" &&
    !linkedFrom.some((source) => source.id === pending.entryId)
      ? pending.title
      : null;

  const alreadyLinked = new Set(linkedFrom.map((source) => source.id));
  const attachable = (entries ?? []).filter(
    (entry) => !alreadyLinked.has(entry.id),
  );

  return (
    <Card className="grid gap-3 p-4" data-testid="actor-lore-linked-from">
      <h2 className="text-sm font-semibold tracking-wide text-muted-foreground uppercase">
        Linked from (lore)
      </h2>

      {linkedFrom.length === 0 ? (
        <p className="text-sm text-muted-foreground italic">
          No lore entries link here yet.
        </p>
      ) : (
        <ul className="grid gap-1">
          {linkedFrom.map((source) => (
            <li key={source.id}>
              <Link
                to={`/world/${worldId}/lore/${source.slug}/view`}
                className="text-sm text-primary hover:underline"
              >
                {source.title}
              </Link>
            </li>
          ))}
        </ul>
      )}

      {pending.kind === "problem" ? (
        <StatusBadge variant="danger">{pending.message}</StatusBadge>
      ) : null}

      {/*
        Said only once the refreshed list has been looked at and this entry is
        not on it. A link that resolved to a same-named lore entry instead of
        this character is a real outcome with nothing to see, and this is the
        one thing the Game Master needs told to be able to fix it.
      */}
      {unresolved ? (
        <p
          className="text-xs text-muted-foreground"
          data-testid="actor-lore-unresolved"
        >
          &ldquo;{unresolved}&rdquo; was saved, but does not link here yet —
          another entry may share this character&rsquo;s name.
        </p>
      ) : null}

      {canManage ? (
        <div className="grid gap-2" data-testid="actor-lore-controls">
          {writing ? (
            <div className="grid gap-2" data-testid="actor-lore-new-form">
              <Input
                value={title}
                onChange={(event) => setTitle(event.target.value)}
                placeholder="What is this entry called?"
                aria-label="New lore entry title"
                data-testid="actor-lore-new-title"
              />
              <Textarea
                value={body}
                onChange={(event) => setBody(event.target.value)}
                placeholder="What do you want to remember? (optional)"
                aria-label="New lore entry text"
                data-testid="actor-lore-new-body"
              />
              <div className="flex gap-2">
                <Button
                  type="button"
                  size="sm"
                  onClick={() => void write()}
                  disabled={pending.kind === "working" || title.trim() === ""}
                  data-testid="actor-lore-new-save"
                >
                  Write it
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  onClick={() => {
                    setWriting(false);
                    setTitle("");
                    setBody("");
                  }}
                  data-testid="actor-lore-new-cancel"
                >
                  Cancel
                </Button>
              </div>
            </div>
          ) : (
            <div className="grid gap-2 sm:grid-cols-[2fr_auto_auto]">
              <select
                value={attachId}
                onChange={(event) => setAttachId(event.target.value)}
                disabled={entries === null || pending.kind === "working"}
                className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
                data-testid="actor-lore-attach-select"
                aria-label="Lore entry to attach"
              >
                <option value="">
                  {entries === null
                    ? "Loading lore…"
                    : attachable.length === 0
                      ? "Nothing else to attach"
                      : "Attach an existing entry…"}
                </option>
                {attachable.map((entry) => (
                  <option key={entry.id} value={entry.id}>
                    {entry.title}
                  </option>
                ))}
              </select>
              <Button
                type="button"
                size="sm"
                onClick={() => void attach()}
                disabled={attachId === "" || pending.kind === "working"}
                data-testid="actor-lore-attach-button"
              >
                Attach
              </Button>
              <Button
                type="button"
                size="sm"
                variant="secondary"
                onClick={() => setWriting(true)}
                data-testid="actor-lore-new"
              >
                New entry
              </Button>
            </div>
          )}
        </div>
      ) : null}
    </Card>
  );
}
