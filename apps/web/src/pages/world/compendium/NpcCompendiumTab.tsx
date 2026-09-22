import { Suspense, useEffect, useMemo, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { Link } from "react-router-dom";
import {
  ACTOR_IMAGE_PORTRAIT,
  ACTOR_IMAGE_TOKEN,
  getWorldActorImages,
  getWorldActors,
  uploadActorImage,
  type ActorImageRecord,
} from "@/api/actors";
import { indexActors, searchActorIds } from "@/search/actorSearch";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { imageForRole, portraitOf } from "@/pages/world/actor/actorImagery";
import {
  LazyHeroBuilderDialog,
  LazyQuickNpcDialog,
} from "@/pages/world/actor/heroBuilderLazy";
import type { WorldActorRecord } from "@/types/actor";

export interface NpcCompendiumTabProps {
  worldId: string;
  onSelect: (actorId: string) => void;
  selectedActorId: string | null;
  /** DM/GM-only — gates the "New NPC" link (FR-006, spec 010 precedent). */
  isGm: boolean;
  /** Bump to force a re-fetch (e.g. after creating a new NPC). */
  refreshKey?: number;
  /**
   * Called whenever the roster is (re)fetched, so the parent
   * `WorldCompendiumPage` can resolve `selectedActorId` into a full
   * `WorldActorRecord` for the preview panel without a second network
   * round trip (research.md §3). Not part of the external component
   * contract's minimal signature (contracts/compendium-npcs.md) — an
   * additive convenience prop.
   */
  onRosterLoaded?: (actors: WorldActorRecord[]) => void;
}

/**
 * Spec 011: the Compendium's NPCs tab. Adapted from spec 010's
 * `NpcCatalog` (`@/components/world/NpcCatalog/NpcCatalog`) — same
 * search/FlexSearch/table data flow, but rows are selectable (open the
 * right-side `ActorPreviewPanel`) instead of navigating away directly.
 * The inline View/Edit links remain as direct-navigation shortcuts,
 * independent of selection (contracts/compendium-npcs.md).
 *
 * Spec 031 (T068, FR-035): the inline "Add NPC" form is gone. Creating an NPC
 * meant two boxes wedged under the table, with no room for a description worth
 * reading and nowhere at all for a portrait — so it moved to `NpcEditorPage`
 * behind an explicit save, and this tab became a list and a link. `refreshKey`
 * survives because the parent may still have reason to refetch; the tab no
 * longer has one of its own.
 */
export function NpcCompendiumTab({
  worldId,
  onSelect,
  selectedActorId,
  isGm,
  refreshKey,
  onRosterLoaded,
}: NpcCompendiumTabProps) {
  const [actors, setActors] = useState<WorldActorRecord[] | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [query, setQuery] = useState("");
  const [matchedIds, setMatchedIds] = useState<string[] | null>(null);
  // Spec 031 (FR-036): the roster's imagery, keyed by actor. Fetched
  // separately from the roster itself because it costs the server a query per
  // actor, and only the screens that show a face should pay for it.
  const [imagesByActor, setImagesByActor] = useState<
    Record<string, ActorImageRecord[]>
  >({});
  /**
   * Owner, 2026-09-15: "This page would be really handy if we could set the
   * image of the individual right from this page. Maybe not everything, but
   * just the image."
   *
   * Which row is mid-upload, and which rows were refused. Keyed by actor
   * rather than held as one value apiece, because the list is the surface:
   * two rows can be in flight at once, and a single flag would put one row's
   * refusal on another row's face.
   */
  const [uploadingId, setUploadingId] = useState<string | null>(null);
  const [uploadError, setUploadError] = useState<Record<string, string>>({});
  // Spec 044 FR-028a, FR-027: the row whose look is being built, and whether
  // Quick NPC is open. The builder's code loads only when one of them is set.
  const [buildingId, setBuildingId] = useState<string | null>(null);
  const [quickNpcOpen, setQuickNpcOpen] = useState(false);
  // Whether the imagery has answered, so a row is not called "lacking art"
  // in the moment before its art arrives.
  const [imagesLoaded, setImagesLoaded] = useState(false);

  // Reset during render rather than at the top of the effect below: this
  // is state derived from the arguments, and doing it in the effect commits
  // one render pairing the new key with the previous key's data.
  useResetOnChange(`${worldId}|${refreshKey ?? ""}`, () => {
    setActors(null);
    setError(null);
  });

  useEffect(() => {
    let active = true;

    getWorldActors(worldId)
      .then((result) => {
        if (!active) {
          return;
        }
        setActors(result);
        onRosterLoaded?.(result);
        const npcs = result.filter((actor) => actor.isNpc);
        void indexActors(
          worldId,
          npcs.map((npc) => ({
            id: npc.id,
            label: npc.label,
            description: npc.description,
          })),
        );
      })
      .catch((err) => {
        if (active) {
          setError(err instanceof Error ? err : new Error(String(err)));
        }
      });

    getWorldActorImages(worldId)
      .then((byActor) => {
        if (active) {
          setImagesByActor(byActor);
          setImagesLoaded(true);
        }
      })
      .catch(() => {
        // Imagery that fails to load leaves the roster readable — a missing
        // portrait is not a reason to hide the NPC it belongs to.
        if (active) {
          setImagesByActor({});
        }
      });

    return () => {
      active = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [worldId, refreshKey]);

  useEffect(() => {
    let active = true;
    searchActorIds(worldId, query).then((ids) => {
      if (active) {
        setMatchedIds(ids);
      }
    });
    return () => {
      active = false;
    };
  }, [worldId, query]);

  const npcs = useMemo(
    () => (actors ?? []).filter((actor) => actor.isNpc),
    [actors],
  );

  const visibleNpcs = useMemo(() => {
    if (matchedIds === null) {
      return npcs;
    }
    const rank = new Map(matchedIds.map((id, index) => [id, index]));
    return npcs
      .filter((npc) => rank.has(npc.id))
      .sort((a, b) => (rank.get(a.id) ?? 0) - (rank.get(b.id) ?? 0));
  }, [npcs, matchedIds]);

  /**
   * Set or replace one NPC's portrait without leaving the list.
   *
   * The same call the editor's imagery panel makes (`uploadActorImage`), not a
   * second uploader: the server transcodes to WebP and refuses an oversized or
   * undecodable file before writing anything (ADR-057), so a refusal here means
   * this NPC's imagery is exactly as it was — which is why the old thumbnail
   * stays on screen and only the message changes.
   *
   * Portrait only. A token is drawn for map scale and cannot be judged at the
   * size a table shows a face; the row's Edit link goes to the editor, which
   * offers both, for everything else.
   */
  const handlePortrait = async (npc: WorldActorRecord, file: File) => {
    setUploadingId(npc.id);
    setUploadError((current) => {
      const next = { ...current };
      delete next[npc.id];
      return next;
    });
    try {
      const saved = await uploadActorImage(npc.id, ACTOR_IMAGE_PORTRAIT, file);
      // The reply is the whole of the change, so the row updates from it
      // rather than refetching every actor's imagery to learn one row.
      setImagesByActor((current) => ({
        ...current,
        [npc.id]: [
          ...(current[npc.id] ?? []).filter(
            (image) => image.role !== ACTOR_IMAGE_PORTRAIT,
          ),
          saved,
        ],
      }));
    } catch (err) {
      setUploadError((current) => ({
        ...current,
        [npc.id]:
          err instanceof Error ? err.message : "That portrait was refused.",
      }));
    } finally {
      setUploadingId((current) => (current === npc.id ? null : current));
    }
  };

  /** Rows a save stored, merged into one NPC's imagery by role. */
  const handleBuilt = (actorId: string, saved: ActorImageRecord[]) => {
    setImagesByActor((current) => ({
      ...current,
      [actorId]: [
        ...(current[actorId] ?? []).filter(
          (image) => !saved.some((row) => row.role === image.role),
        ),
        ...saved,
      ],
    }));
  };

  /**
   * Quick NPC made an NPC. It joins the list and is selected (US4 scenario 2)
   * whether or not its art was stored — a failed upload leaves it listed as
   * lacking art, never deleted (FR-028).
   */
  const handleQuickNpc = (
    actor: WorldActorRecord,
    saved: ActorImageRecord[],
  ) => {
    const next = [...(actors ?? []), actor];
    setActors(next);
    onRosterLoaded?.(next);
    void indexActors(
      worldId,
      next
        .filter((entry) => entry.isNpc)
        .map((npc) => ({
          id: npc.id,
          label: npc.label,
          description: npc.description,
        })),
    );
    handleBuilt(actor.id, saved);
    onSelect(actor.id);
  };

  const buildingNpc = npcs.find((npc) => npc.id === buildingId) ?? null;

  if (error) {
    return (
      <p className="text-sm text-destructive">
        Failed to load NPCs: {error.message}
      </p>
    );
  }

  if (actors === null) {
    return <p className="text-sm text-muted-foreground">Loading NPCs…</p>;
  }

  return (
    <div className="grid gap-3">
      <Input
        type="search"
        placeholder="Search NPCs by name or description…"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        data-testid="npc-catalog-search-input"
        aria-label="Search NPCs"
      />

      {npcs.length === 0 ? (
        <p className="text-sm text-muted-foreground">No NPCs yet.</p>
      ) : visibleNpcs.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          No NPCs match "{query}".
        </p>
      ) : (
        <div className="overflow-x-auto rounded-lg border border-border">
          <table className="w-full text-sm" data-testid="npc-catalog-table">
            <thead>
              <tr className="border-b border-border bg-muted/50 text-left text-xs tracking-wide text-muted-foreground uppercase">
                <th className="p-2 font-semibold">Portrait</th>
                <th className="p-2 font-semibold">Name</th>
                <th className="p-2 font-semibold">Description</th>
                <th className="p-2 font-semibold">Actions</th>
              </tr>
            </thead>
            <tbody>
              {visibleNpcs.map((npc) => (
                <tr
                  key={npc.id}
                  className={cn(
                    "cursor-pointer border-b border-border last:border-0 hover:bg-muted/40",
                    selectedActorId === npc.id && "bg-muted",
                  )}
                  data-testid={`npc-catalog-row-${npc.id}`}
                  onClick={() => onSelect(npc.id)}
                  aria-selected={selectedActorId === npc.id}
                >
                  <td className="p-2">
                    {/* The list shows the portrait, never the token: a token
                        is drawn for map scale and reads as a smudge here.

                        The face is also the control (owner, 2026-09-15). The
                        thing a person points at when they want to change a
                        picture is the picture, so the avatar itself opens the
                        file chooser rather than growing a button beside it —
                        and the box keeps the same 8×10 whatever state it is
                        in, so nothing below it moves when an image lands.

                        A real file input, visually hidden but focusable, with
                        the avatar as its label: that is a keyboard-operable
                        upload without re-implementing one, and it carries the
                        NPC's actual name for anyone who cannot see the face.

                        Offered only where the server would allow it — the same
                        permission the row's Edit link is gated on. The server
                        refuses either way (Constitution Principle III). */}
                    <div
                      className="relative h-10 w-8"
                      onClick={(event) => event.stopPropagation()}
                    >
                      {portraitOf(imagesByActor[npc.id]) ? (
                        <img
                          src={portraitOf(imagesByActor[npc.id])!.thumbnailUrl}
                          alt=""
                          className="h-10 w-8 rounded border border-border object-cover"
                          data-testid={`npc-catalog-portrait-${npc.id}`}
                        />
                      ) : (
                        <div
                          className="h-10 w-8 rounded border border-dashed border-border"
                          data-testid={`npc-catalog-portrait-empty-${npc.id}`}
                        />
                      )}
                      {npc.myPermissionLevel !== "VIEWER" ? (
                        <>
                          <input
                            type="file"
                            accept="image/*"
                            id={`npc-portrait-input-${npc.id}`}
                            disabled={uploadingId === npc.id}
                            className="peer sr-only"
                            // The name belongs on the input, which is what
                            // takes focus and what a screen reader lands on.
                            // On the label it named a thing nobody can reach.
                            aria-label={`${
                              portraitOf(imagesByActor[npc.id])
                                ? "Replace portrait for"
                                : "Set portrait for"
                            } ${npc.label}`}
                            data-testid={`npc-catalog-portrait-input-${npc.id}`}
                            onChange={(event) => {
                              const file = event.target.files?.[0];
                              // Cleared so choosing the same file again after
                              // a refusal still fires a change event.
                              event.target.value = "";
                              if (file) {
                                void handlePortrait(npc, file);
                              }
                            }}
                          />
                          <label
                            htmlFor={`npc-portrait-input-${npc.id}`}
                            // The pointer's target, not a second name: the
                            // input above already carries it.
                            aria-hidden="true"
                            title={`${
                              portraitOf(imagesByActor[npc.id])
                                ? "Replace portrait"
                                : "Set portrait"
                            }`}
                            data-testid={`npc-catalog-portrait-set-${npc.id}`}
                            className="absolute inset-0 grid cursor-pointer place-items-center rounded bg-background/95 text-[0.6rem] font-semibold text-foreground opacity-0 transition-opacity hover:opacity-100 peer-focus-visible:opacity-100 peer-focus-visible:ring-2 peer-focus-visible:ring-ring"
                          >
                            {uploadingId === npc.id ? "…" : "Edit"}
                          </label>
                        </>
                      ) : null}
                    </div>
                  </td>
                  <td className="p-2 font-medium">
                    {npc.label}
                    {uploadError[npc.id] ? (
                      // On the row that failed, and nowhere else. The previous
                      // portrait is untouched above it.
                      <span
                        role="alert"
                        className="block text-xs font-normal text-destructive"
                        data-testid={`npc-catalog-portrait-error-${npc.id}`}
                      >
                        {uploadError[npc.id]}
                      </span>
                    ) : null}
                  </td>
                  <td className="max-w-xs truncate p-2 text-muted-foreground">
                    {npc.description || (
                      <span className="italic">No description</span>
                    )}
                  </td>
                  <td className="p-2">
                    <div
                      className="flex gap-2"
                      onClick={(event) => event.stopPropagation()}
                    >
                      <Button
                        asChild
                        variant="ghost"
                        size="sm"
                        data-testid={`npc-catalog-view-${npc.id}`}
                      >
                        <Link to={`/world/${worldId}/actor/${npc.id}/view`}>
                          View
                        </Link>
                      </Button>
                      {imagesLoaded &&
                      (!portraitOf(imagesByActor[npc.id]) ||
                        !imageForRole(
                          imagesByActor[npc.id],
                          ACTOR_IMAGE_TOKEN,
                        )) ? (
                        // Spec 044 FR-028: an NPC without both pictures says
                        // so, beside the control that gives it them.
                        <span
                          role="img"
                          aria-label={`${npc.label} lacks ${
                            portraitOf(imagesByActor[npc.id])
                              ? "a token"
                              : imageForRole(
                                    imagesByActor[npc.id],
                                    ACTOR_IMAGE_TOKEN,
                                  )
                                ? "a portrait"
                                : "a portrait and a token"
                          }`}
                          className="self-center rounded border border-dashed border-border px-1.5 text-[0.65rem] text-muted-foreground"
                          data-testid={`npc-catalog-lacks-art-${npc.id}`}
                        >
                          No art
                        </span>
                      ) : null}
                      {npc.myPermissionLevel !== "VIEWER" ? (
                        <Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          onClick={() => setBuildingId(npc.id)}
                          aria-label={`Build look for ${npc.label}`}
                          data-testid={`npc-catalog-build-${npc.id}`}
                        >
                          Build look
                        </Button>
                      ) : null}
                      {npc.myPermissionLevel !== "VIEWER" ? (
                        <Button
                          asChild
                          variant="ghost"
                          size="sm"
                          data-testid={`npc-catalog-edit-${npc.id}`}
                        >
                          <Link to={`/world/${worldId}/actor/${npc.id}/edit`}>
                            Edit
                          </Link>
                        </Button>
                      ) : null}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {isGm ? (
        <div className="flex flex-wrap gap-2">
          <Button asChild size="sm" icon="skull" data-testid="new-npc-link">
            <Link to={`/world/${worldId}/compendium/npc/new`}>New NPC</Link>
          </Button>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            icon="spark"
            onClick={() => setQuickNpcOpen(true)}
            data-testid="quick-npc"
          >
            Quick NPC
          </Button>
        </div>
      ) : null}

      {buildingNpc ? (
        <Suspense fallback={null}>
          <LazyHeroBuilderDialog
            key={buildingNpc.id}
            open
            worldId={worldId}
            onOpenChange={(open) => {
              if (!open) setBuildingId(null);
            }}
            actorId={buildingNpc.id}
            actorLabel={buildingNpc.label}
            existingRoles={(imagesByActor[buildingNpc.id] ?? []).map(
              (image) => image.role,
            )}
            onSaved={(saved) => handleBuilt(buildingNpc.id, saved)}
          />
        </Suspense>
      ) : null}

      {isGm && quickNpcOpen ? (
        <Suspense fallback={null}>
          <LazyQuickNpcDialog
            open
            worldId={worldId}
            onOpenChange={setQuickNpcOpen}
            onCreated={handleQuickNpc}
          />
        </Suspense>
      ) : null}
    </div>
  );
}
