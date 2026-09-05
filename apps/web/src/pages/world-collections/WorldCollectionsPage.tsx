import { useCallback, useEffect, useMemo, useState } from "react";
import { useParams } from "react-router-dom";
import { getWorldAbilities } from "@/api/abilities";
import { getWorldActors } from "@/api/actors";
import {
  addCollectionMember,
  createCollection,
  createCollectionShareLink,
  deleteCollection,
  getCollectionMembers,
  getWorldCollections,
  removeCollectionMember,
  revokeCollectionShareLink,
} from "@/api/collections";
import { getWorldItems } from "@/api/items";
import { getWorldLoreEntries } from "@/api/lore";
import { getScenes } from "@/api/scenes";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { COLLECTION_MEMBER_TYPES, memberTypeLabel } from "@/types/collection";
import type {
  CollectionMemberRecord,
  CollectionMemberType,
  CollectionRecord,
  CollectionShareLinkRecord,
} from "@/types/collection";

/**
 * Spec 026 (T031): `/world/:id/collections` — gathering a world's content into
 * a collection, and sharing it.
 *
 * # Refusals are shown, not replaced
 *
 * The server refuses members for reasons the author can act on: a GM-only
 * ability may not be shared, an artifact from another world does not belong,
 * the hundred-and-first member exceeds the cap. Each refusal names its reason.
 * Every catch below therefore surfaces `err.message` verbatim (FR-001a). A
 * generic "could not add member" would leave an author with no idea which of
 * those three happened or what to do next.
 */

/** One thing that can go in a collection, from any of the five sources. */
type Candidate = {
  id: string;
  label: string;
};

/**
 * The five artifact types name their identity and their title differently —
 * scenes use `sceneId`/`name`, actors `id`/`label`, lore `id`/`title`, items
 * and abilities `id`/`name`. Normalised once here so nothing below has to
 * remember which is which.
 */
async function loadCandidates(
  memberType: CollectionMemberType,
  worldId: string,
): Promise<Candidate[]> {
  switch (memberType) {
    case "scene": {
      const rows = await getScenes(worldId);
      return rows.map((r) => ({ id: r.sceneId, label: r.name }));
    }
    case "actor": {
      const rows = await getWorldActors(worldId);
      return rows.map((r) => ({ id: r.id, label: r.label }));
    }
    case "item": {
      const rows = await getWorldItems(worldId);
      return rows.map((r) => ({ id: r.id, label: r.name }));
    }
    case "lore": {
      const rows = await getWorldLoreEntries(worldId);
      return rows.map((r) => ({ id: r.id, label: r.title }));
    }
    case "ability": {
      const rows = await getWorldAbilities(worldId);
      return rows.map((r) => ({ id: r.id, label: r.name }));
    }
  }
}

export default function WorldCollectionsPage() {
  const { id: worldId = "" } = useParams();

  const [collections, setCollections] = useState<CollectionRecord[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [pageError, setPageError] = useState<string | null>(null);

  const [newName, setNewName] = useState("");
  const [newDescription, setNewDescription] = useState("");
  const [createError, setCreateError] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  const [openId, setOpenId] = useState<string | null>(null);

  // Bumped to ask for a reload. The fetch lives in the effect and sets state
  // from `.then`, rather than the effect calling an async function that sets
  // state itself — `react-hooks/set-state-in-effect` rejects the latter, and
  // the shipped share pages are all written this way.
  const [reloadToken, setReloadToken] = useState(0);
  const refresh = useCallback(() => setReloadToken((n) => n + 1), []);

  useEffect(() => {
    if (!worldId) {
      return;
    }
    let active = true;
    getWorldCollections(worldId)
      .then((rows) => {
        if (active) {
          setCollections(rows);
          setPageError(null);
        }
      })
      .catch((err: unknown) => {
        if (active) {
          setPageError(
            err instanceof Error ? err.message : "Could not load collections.",
          );
        }
      })
      .finally(() => {
        if (active) {
          setIsLoading(false);
        }
      });
    return () => {
      active = false;
    };
  }, [worldId, reloadToken]);

  const handleCreate = async () => {
    if (newName.trim() === "") {
      return;
    }
    setIsCreating(true);
    setCreateError(null);
    try {
      await createCollection({
        worldId,
        name: newName.trim(),
        description:
          newDescription.trim() === "" ? null : newDescription.trim(),
      });
      setNewName("");
      setNewDescription("");
      refresh();
    } catch (err: unknown) {
      setCreateError(
        err instanceof Error ? err.message : "Could not create the collection.",
      );
    } finally {
      setIsCreating(false);
    }
  };

  if (isLoading) {
    return <Loader fullScreen label="Loading collections" />;
  }

  return (
    <>
      <SEO
        title="Collections"
        description="Gather this world's content into a collection and share it"
        noindex
      />
      <Container>
        <main className="grid gap-6 py-10">
          <div>
            <h1 className="text-2xl font-semibold">Collections</h1>
            <p className="text-muted-foreground">
              Gather scenes, actors, items, lore and abilities into one thing
              you can hand to another Game Master.
            </p>
          </div>

          {pageError ? (
            <StatusBadge variant="danger">{pageError}</StatusBadge>
          ) : null}

          <Card className="grid gap-3 p-5">
            <h2 className="text-lg font-medium">New collection</h2>
            <input
              aria-label="Collection name"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="The Haunted Manor"
              className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
            />
            <input
              aria-label="Collection description"
              value={newDescription}
              onChange={(e) => setNewDescription(e.target.value)}
              placeholder="Optional description"
              className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
            />
            <div>
              <Button
                onClick={() => void handleCreate()}
                disabled={isCreating || newName.trim() === ""}
              >
                {isCreating ? "Creating..." : "Create collection"}
              </Button>
            </div>
            {createError ? (
              <StatusBadge variant="danger">{createError}</StatusBadge>
            ) : null}
          </Card>

          {collections.length === 0 ? (
            <p className="text-muted-foreground">
              No collections in this world yet.
            </p>
          ) : (
            <ul className="grid gap-4" data-testid="collection-list">
              {collections.map((collection) => (
                <li key={collection.id}>
                  <CollectionCard
                    collection={collection}
                    worldId={worldId}
                    isOpen={openId === collection.id}
                    onToggle={() =>
                      setOpenId(openId === collection.id ? null : collection.id)
                    }
                    onChanged={refresh}
                  />
                </li>
              ))}
            </ul>
          )}
        </main>
      </Container>
    </>
  );
}

function CollectionCard({
  collection,
  worldId,
  isOpen,
  onToggle,
  onChanged,
}: {
  collection: CollectionRecord;
  worldId: string;
  isOpen: boolean;
  onToggle: () => void;
  onChanged: () => void;
}) {
  const [members, setMembers] = useState<CollectionMemberRecord[] | null>(null);
  const [memberError, setMemberError] = useState<string | null>(null);

  const [pickerType, setPickerType] = useState<CollectionMemberType>("scene");
  const [loadedCandidates, setLoadedCandidates] = useState<{
    memberType: CollectionMemberType;
    rows: Candidate[];
  } | null>(null);
  const [selectedCandidateId, setSelectedCandidateId] = useState("");

  const [share, setShare] = useState<CollectionShareLinkRecord | null>(null);
  const [shareError, setShareError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [confirmingRevoke, setConfirmingRevoke] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  const [membersToken, setMembersToken] = useState(0);
  const reloadMembers = useCallback(() => setMembersToken((n) => n + 1), []);

  useEffect(() => {
    if (!isOpen) {
      return;
    }
    let active = true;
    getCollectionMembers(collection.id)
      .then((rows) => {
        if (active) {
          setMembers(rows);
          setMemberError(null);
        }
      })
      .catch((err: unknown) => {
        if (active) {
          setMemberError(
            err instanceof Error ? err.message : "Could not load members.",
          );
        }
      });
    return () => {
      active = false;
    };
  }, [isOpen, collection.id, membersToken]);

  // Cached with the type it was loaded for, so switching type reads as
  // "loading" without an effect having to null it out first. Clearing state
  // synchronously in an effect body is what `react-hooks/set-state-in-effect`
  // is about; deriving it costs nothing and avoids the extra render.
  useEffect(() => {
    if (!isOpen) {
      return;
    }
    let active = true;
    loadCandidates(pickerType, worldId)
      .then((rows) => {
        if (active) {
          setLoadedCandidates({ memberType: pickerType, rows });
        }
      })
      .catch((err: unknown) => {
        if (active) {
          setMemberError(
            err instanceof Error
              ? err.message
              : `Could not load this world's ${memberTypeLabel(pickerType, 2).toLowerCase()}.`,
          );
        }
      });
    return () => {
      active = false;
    };
  }, [isOpen, pickerType, worldId]);

  const candidates =
    loadedCandidates !== null && loadedCandidates.memberType === pickerType
      ? loadedCandidates.rows
      : null;

  const shareUrl = useMemo(
    () =>
      share === null
        ? null
        : `${window.location.origin}/collection/${share.shareCode}`,
    [share],
  );

  const handleAdd = async () => {
    if (selectedCandidateId === "") {
      return;
    }
    setMemberError(null);
    try {
      await addCollectionMember({
        collectionId: collection.id,
        memberType: pickerType,
        memberId: selectedCandidateId,
      });
      setSelectedCandidateId("");
      reloadMembers();
      onChanged();
    } catch (err: unknown) {
      // Verbatim (FR-001a). "This ability is visible only to the Game Master,
      // so it cannot be shared in a collection" is the whole answer; replacing
      // it with "could not add member" throws the answer away.
      setMemberError(
        err instanceof Error ? err.message : "Could not add that member.",
      );
    }
  };

  const handleRemove = async (memberId: string) => {
    setMemberError(null);
    try {
      await removeCollectionMember(collection.id, memberId);
      reloadMembers();
      onChanged();
    } catch (err: unknown) {
      setMemberError(
        err instanceof Error ? err.message : "Could not remove that member.",
      );
    }
  };

  const handleShare = async () => {
    setShareError(null);
    try {
      setShare(await createCollectionShareLink(collection.id));
    } catch (err: unknown) {
      setShareError(
        err instanceof Error ? err.message : "Could not create a share link.",
      );
    }
  };

  const handleRevoke = async () => {
    if (share === null) {
      return;
    }
    setShareError(null);
    try {
      await revokeCollectionShareLink(share.id);
      setShare({ ...share, revoked: true });
      setConfirmingRevoke(false);
    } catch (err: unknown) {
      setShareError(
        err instanceof Error ? err.message : "Could not revoke the link.",
      );
    }
  };

  const handleDelete = async () => {
    try {
      await deleteCollection(collection.id);
      onChanged();
    } catch (err: unknown) {
      setMemberError(
        err instanceof Error ? err.message : "Could not delete the collection.",
      );
    }
  };

  return (
    <Card className="grid gap-4 p-5">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h2 className="text-lg font-medium">{collection.name}</h2>
          {collection.description ? (
            <p className="text-sm text-muted-foreground">
              {collection.description}
            </p>
          ) : null}
          <p className="text-sm text-muted-foreground">
            {collection.memberCount}{" "}
            {collection.memberCount === 1 ? "member" : "members"}
          </p>
        </div>
        <Button variant="ghost" onClick={onToggle}>
          {isOpen ? "Close" : "Open"}
        </Button>
      </div>

      {isOpen ? (
        <div className="grid gap-4">
          {memberError ? (
            <StatusBadge variant="danger">{memberError}</StatusBadge>
          ) : null}

          <div className="grid gap-2">
            <h3 className="text-sm font-medium">Contents</h3>
            {members === null ? (
              <p className="text-sm text-muted-foreground">Loading…</p>
            ) : members.length === 0 ? (
              <p className="text-sm text-muted-foreground">
                Nothing in this collection yet.
              </p>
            ) : (
              <ul className="grid gap-1" data-testid="collection-members">
                {members.map((member) => (
                  <li
                    key={member.id}
                    className="flex items-center justify-between gap-3 text-sm"
                  >
                    <span className="flex items-baseline gap-2">
                      <span className="text-xs text-muted-foreground uppercase">
                        {memberTypeLabel(member.memberType)}
                      </span>
                      <span className="font-mono text-xs">
                        {member.memberId}
                      </span>
                    </span>
                    <Button
                      variant="ghost"
                      onClick={() => void handleRemove(member.memberId)}
                    >
                      Remove
                    </Button>
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div className="grid gap-2">
            <h3 className="text-sm font-medium">Add to this collection</h3>
            <div className="flex flex-wrap gap-2">
              <select
                aria-label="Content type"
                value={pickerType}
                onChange={(e) => {
                  setPickerType(e.target.value as CollectionMemberType);
                  setSelectedCandidateId("");
                }}
                className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
              >
                {COLLECTION_MEMBER_TYPES.map((type) => (
                  <option key={type} value={type}>
                    {memberTypeLabel(type, 2)}
                  </option>
                ))}
              </select>
              <select
                aria-label="Content to add"
                value={selectedCandidateId}
                onChange={(e) => setSelectedCandidateId(e.target.value)}
                disabled={candidates === null}
                className="h-9 min-w-56 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
              >
                <option value="">
                  {candidates === null ? "Loading…" : "Choose..."}
                </option>
                {(candidates ?? []).map((candidate) => (
                  <option key={candidate.id} value={candidate.id}>
                    {candidate.label}
                  </option>
                ))}
              </select>
              <Button
                onClick={() => void handleAdd()}
                disabled={selectedCandidateId === ""}
              >
                Add
              </Button>
            </div>
          </div>

          <div className="grid gap-2">
            <h3 className="text-sm font-medium">Sharing</h3>
            {shareError ? (
              <StatusBadge variant="danger">{shareError}</StatusBadge>
            ) : null}
            {share === null ? (
              <div>
                <Button onClick={() => void handleShare()}>
                  Create a share link
                </Button>
              </div>
            ) : share.revoked ? (
              <StatusBadge variant="warning">
                This link has been revoked. Copies already made are unaffected.
              </StatusBadge>
            ) : (
              <div className="grid gap-2">
                {/*
                  The same caveat the shipped ability share page carries, for
                  the same reason. No-enumeration is one of the invariants
                  ADR-069's determination rests on, so there is deliberately no
                  "list this collection's share links" call — which means this
                  link cannot be shown again after you leave the page, and
                  revoking it is only possible while it is on screen.
                */}
                <p className="text-xs text-muted-foreground">
                  Anyone with this link can read the collection without an
                  account, and copy it into a world they run. It is not listed
                  or discoverable anywhere, and it will not be shown again —
                  keep it if you may want to revoke it later.
                </p>
                <code
                  className="rounded-lg border border-input px-2.5 py-2 text-xs break-all"
                  data-testid="share-url"
                >
                  {shareUrl}
                </code>
                <div className="flex flex-wrap gap-2">
                  <Button
                    onClick={() => {
                      if (shareUrl !== null) {
                        void navigator.clipboard.writeText(shareUrl);
                        setCopied(true);
                      }
                    }}
                  >
                    {copied ? "Copied" : "Copy link"}
                  </Button>
                  <Button
                    variant="ghost"
                    onClick={() => setConfirmingRevoke(true)}
                  >
                    Revoke link
                  </Button>
                </div>

                {/*
                  T037 / FR-011. The sentence about existing copies belongs
                  *here*, at the moment of revoking, not in a help page. A Game
                  Master pressing this button is usually trying to take
                  something back, and revoking cannot do that: the link stops
                  resolving, and every copy already taken is independent and
                  stays where it is. Telling them afterwards is telling them
                  too late.
                */}
                {confirmingRevoke ? (
                  <Card className="grid gap-2 p-4" data-testid="revoke-confirm">
                    <p className="text-sm font-medium">Revoke this link?</p>
                    <p className="text-sm text-muted-foreground">
                      The link will stop working immediately, and nobody new
                      will be able to open or copy this collection.
                    </p>
                    <p className="text-sm text-muted-foreground">
                      <strong>Copies already made are not affected.</strong>{" "}
                      Anyone who has already copied this collection keeps their
                      copy, and revoking cannot take it back.
                    </p>
                    <div className="flex gap-2">
                      <Button onClick={() => void handleRevoke()}>
                        Revoke the link
                      </Button>
                      <Button
                        variant="ghost"
                        onClick={() => setConfirmingRevoke(false)}
                      >
                        Keep it
                      </Button>
                    </div>
                  </Card>
                ) : null}
              </div>
            )}
          </div>

          <div className="grid gap-2 border-t border-input pt-4">
            {confirmingDelete ? (
              <Card className="grid gap-2 p-4" data-testid="delete-confirm">
                <p className="text-sm font-medium">
                  Delete “{collection.name}”?
                </p>
                {/*
                  FR-013. The reassurance matters as much as the warning: a
                  collection is a list of references, and deleting the list
                  deletes nothing it referred to.
                */}
                <p className="text-sm text-muted-foreground">
                  This deletes the collection only. Every scene, actor, item,
                  lore entry and ability in it stays exactly where it is.
                </p>
                <div className="flex gap-2">
                  <Button onClick={() => void handleDelete()}>
                    Delete the collection
                  </Button>
                  <Button
                    variant="ghost"
                    onClick={() => setConfirmingDelete(false)}
                  >
                    Cancel
                  </Button>
                </div>
              </Card>
            ) : (
              <div>
                <Button
                  variant="ghost"
                  onClick={() => setConfirmingDelete(true)}
                >
                  Delete collection
                </Button>
              </div>
            )}
          </div>
        </div>
      ) : null}
    </Card>
  );
}
