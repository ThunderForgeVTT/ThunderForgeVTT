import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import {
  createInteractive,
  deleteInteractive,
  getInteractives,
  placeProp,
  updateInteractive,
  type Interactive,
  type SubjectKind,
} from "@/api/interactives";
import {
  beginPropPlacement,
  cancelTokenPlacement,
  onPlacementCancelled,
  onPlacementConfirmed,
} from "@/engine/bevy";
import { getWorldLoreEntries } from "@/api/lore";
import {
  InteractionAuthor,
  DoorControls,
  type ReferenceChoice,
} from "@/components/InteractionAuthor";
import type { WorldStore } from "@/engine/world/store";
import { refreshInteractives } from "@/engine/world/sync/interactives";
import type { WorldLight, WorldWall } from "@/engine/world/types";
import { placeAuthoredProp, type PropDraft } from "./placeAuthoredProp";

/**
 * The GM's rail panel for interactive elements (spec 030).
 *
 * # Why this exists rather than `InteractionAuthor` going straight in the rail
 *
 * `InteractionAuthor` edits *one* interactive on *one* subject. This is the
 * part that knows which subject the GM currently has selected, what already
 * exists on it, and what is referenceable in this world — none of which is the
 * author panel's business, and all of which needs the page.
 *
 * The split matters because the author panel is driven entirely by the effect
 * registry: it must not learn what a wall or a light is, or adding a subsystem
 * would mean editing it too.
 *
 * # Placing, as well as authoring (spec 031 FR-011)
 *
 * Authoring an interactive needed something already on the map to author it
 * onto, so a lore marker could be configured and never placed. The missing
 * half is a *gesture*, and the engine already has it: the carry the actors
 * pane uses to put an actor's token down. This panel asks for the same carry
 * with a different kind, and creates what the drop turns out to mean.
 *
 * Chrome never positions anything — it hands the engine a request and is told
 * where the drop landed (Constitution Principle I). What it does own is the
 * pair of server calls that follow, because knowing that a prop is a token and
 * an interactive points at one is this application's knowledge, not the
 * canvas's.
 */

export interface InteractionToolProps {
  worldStore: WorldStore;
  worldId: string;
  sceneId: string;
  selectedTokenId: string | null;
  selectedWallId: string | null;
  walls: Record<string, WorldWall>;
  lights: Record<string, WorldLight>;
}

export function InteractionTool({
  worldStore,
  worldId,
  sceneId,
  selectedTokenId,
  selectedWallId,
  walls,
  lights,
}: InteractionToolProps) {
  const [interactives, setInteractives] = useState<Interactive[]>([]);
  const [lore, setLore] = useState<ReferenceChoice[]>([]);
  const [problem, setProblem] = useState<string | null>(null);
  /** Whether a carry this panel began is still in flight. */
  const [carrying, setCarrying] = useState(false);
  /** What the last drop turned into, in the Game Master's words. */
  const [placed, setPlaced] = useState<string | null>(null);
  /**
   * The draft the carry was begun with.
   *
   * A ref rather than state because the confirmation arrives from the engine
   * on its own frame, and re-subscribing the listener on every keystroke in
   * the form would mean a drop landing between renders on a listener that had
   * already been torn down.
   */
  const draft = useRef<PropDraft | null>(null);
  /**
   * The same answer as `carrying`, readable from a cleanup function.
   *
   * Needed because the cleanup must tell the engine to abandon the carry when
   * this panel goes away mid-gesture, and must *not* when the carry has just
   * ended normally — a cancel sent while nothing is carried would otherwise be
   * waiting for the next one.
   */
  const carryingRef = useRef(false);

  const reload = useCallback(async () => {
    try {
      setInteractives(await getInteractives(sceneId));
      // Push the same list into the engine, so a change made here is on the
      // canvas immediately rather than at the next world event.
      await refreshInteractives(worldStore, sceneId);
    } catch {
      setProblem("Could not read this scene's interactive elements.");
    }
  }, [sceneId, worldStore]);

  useEffect(() => {
    let cancelled = false;
    getInteractives(sceneId)
      .then((list) => {
        if (!cancelled) setInteractives(list);
      })
      .catch(() => {
        if (!cancelled)
          setProblem("Could not read this scene's interactive elements.");
      });
    return () => {
      cancelled = true;
    };
  }, [sceneId]);

  useEffect(() => {
    let cancelled = false;
    getWorldLoreEntries(worldId)
      .then((entries) => {
        if (cancelled) return;
        setLore(entries.map((e) => ({ id: e.id, label: e.title })));
      })
      .catch(() => {
        // A world with no lore is normal, and a failed read here only means
        // the lore picker is empty — not that authoring is broken.
        if (!cancelled) setLore([]);
      });
    return () => {
      cancelled = true;
    };
  }, [worldId]);

  /** What the GM currently has selected, as a subject. */
  const subject = useMemo((): {
    kind: SubjectKind;
    ref: string;
  } | null => {
    if (selectedWallId) return { kind: "door", ref: selectedWallId };
    if (selectedTokenId) return { kind: "prop", ref: selectedTokenId };
    return null;
  }, [selectedTokenId, selectedWallId]);

  const existing = useMemo(
    () =>
      subject
        ? (interactives.find((i) => i.subjectRef === subject.ref) ?? null)
        : null,
    [interactives, subject],
  );

  const references = useMemo(
    (): Record<string, ReferenceChoice[]> => ({
      wall: Object.values(walls).map((wall) => ({
        id: wall.id,
        label:
          wall.doorState === "none"
            ? `Wall ${wall.id.slice(0, 8)}`
            : `Door ${wall.id.slice(0, 8)} (${wall.doorState})`,
      })),
      light: Object.values(lights).map((light) => ({
        id: light.id,
        label: `Light ${light.id.slice(0, 8)}`,
      })),
      loreEntry: lore,
    }),
    [walls, lights, lore],
  );

  const save = useCallback(
    async (draft: {
      effectId: string | null;
      effectConfig: Record<string, unknown> | null;
      activation: string;
      fireMode: string;
    }) => {
      if (!subject) return;
      setProblem(null);
      try {
        if (existing) {
          await updateInteractive(existing.interactiveId, {
            effectId: draft.effectId,
            effectConfig: draft.effectConfig,
            activation: draft.activation,
            fireMode: draft.fireMode,
            clearEffect: draft.effectId === null,
          });
        } else {
          await createInteractive({
            sceneId,
            subjectKind: subject.kind,
            subjectRef: subject.ref,
            effectId: draft.effectId,
            effectConfig: draft.effectConfig,
            trigger: "click",
            activation: draft.activation,
            fireMode: draft.fireMode,
          });
        }
        await reload();
      } catch {
        // Said out loud: the server refuses configuration that does not match
        // an effect's declaration, and a GM who saw nothing happen would
        // reasonably conclude the panel was broken.
        setProblem("That could not be saved.");
      }
    },
    [existing, reload, sceneId, subject],
  );

  const remove = useCallback(async () => {
    if (!existing) return;
    try {
      await deleteInteractive(existing.interactiveId);
      await reload();
    } catch {
      setProblem("That could not be removed.");
    }
  }, [existing, reload]);

  /**
   * Hand the engine a carry, and remember what it is for.
   *
   * Nothing is created here and nothing is positioned here. If the engine
   * cannot take the request — a bundle without the placement machine — the
   * Game Master is told, rather than left holding a form whose button appears
   * to do nothing.
   */
  const beginPlacing = useCallback(async (drafted: PropDraft) => {
    setProblem(null);
    setPlaced(null);
    const began = await beginPropPlacement();
    if (!began) {
      setProblem("This build cannot place things on the map.");
      return;
    }
    draft.current = drafted;
    carryingRef.current = true;
    setCarrying(true);
  }, []);

  useEffect(() => {
    if (!carrying) {
      return;
    }

    const stopConfirmed = onPlacementConfirmed((event) => {
      // An actor's token is somebody else's carry. Only one can be in flight
      // at a time, but the listener is global and the kind is what says whose
      // drop this was.
      if (event.kind !== "prop") {
        return;
      }
      const drafted = draft.current;
      carryingRef.current = false;
      setCarrying(false);
      draft.current = null;
      if (!drafted) {
        return;
      }

      void (async () => {
        const outcome = await placeAuthoredProp(
          { placeProp, createInteractive },
          sceneId,
          event,
          drafted,
        );
        if (outcome.kind === "placed") {
          setPlaced("Placed. Click the button again to place another.");
        } else {
          setProblem(outcome.message);
        }
        // Whatever happened, the scene's list and the canvas are re-read from
        // the server rather than guessed at: a badge drawn from a list this
        // panel assembled would be a second opinion about what exists.
        await reload();
      })();
    });

    const stopCancelled = onPlacementCancelled(() => {
      carryingRef.current = false;
      setCarrying(false);
      draft.current = null;
    });

    return () => {
      stopConfirmed();
      stopCancelled();
      // Only if it is still in hand. The engine holds the cancel until the
      // next carry reads it, so sending one after a drop would abandon the
      // *following* placement.
      if (carryingRef.current) {
        carryingRef.current = false;
        void cancelTokenPlacement();
      }
    };
  }, [carrying, reload, sceneId]);

  const selectedWall = selectedWallId ? walls[selectedWallId] : null;

  return (
    <div className="grid gap-3" data-testid="interaction-tool">
      <p className="text-xs text-muted-foreground">
        {interactives.length === 0
          ? "Nothing on this scene responds yet."
          : `${interactives.length} thing${interactives.length === 1 ? "" : "s"} on this scene respond.`}
      </p>

      {selectedWall ? (
        <DoorControls wall={selectedWall} onChanged={() => void reload()} />
      ) : null}

      {subject ? (
        <InteractionAuthor
          key={`${subject.kind}:${subject.ref}`}
          subjectKind={subject.kind}
          subjectRef={subject.ref}
          existing={existing}
          references={references}
          onSave={(draft) => void save(draft)}
          onDelete={existing ? () => void remove() : undefined}
        />
      ) : (
        <>
          <p className="text-xs text-muted-foreground">
            Select a token or a wall to give it something to do.
          </p>

          {/*
            Or make something new to do it with. Offered here, where the GM
            has just been told they have nothing selected, because that is
            the moment they are looking for a way to put one down — and
            because two author panels at once would be two forms with one
            Save each and no way to tell which is which.
          */}
          <section className="grid gap-2" data-testid="prop-placer">
            <p className="text-xs text-muted-foreground">
              Or place something new: choose what it does, then click the map.
            </p>
            <InteractionAuthor
              subjectKind="prop"
              references={references}
              onSave={(drafted) => void beginPlacing(drafted)}
              saveLabel="Place on the map"
            />
            {carrying ? (
              <div className="flex items-center gap-2">
                <p
                  className="text-xs text-muted-foreground"
                  data-testid="prop-placer-carrying"
                >
                  Click the map to drop it, or press Escape.
                </p>
                <Button
                  variant="ghost"
                  onClick={() => void cancelTokenPlacement()}
                  data-testid="prop-placer-cancel"
                >
                  Cancel
                </Button>
              </div>
            ) : null}
            {placed ? (
              <p className="text-xs" data-testid="prop-placer-placed">
                {placed}
              </p>
            ) : null}
          </section>
        </>
      )}

      {existing?.firedAt ? (
        <Button variant="ghost" onClick={() => void reload()}>
          Fired once already — reset it from the list
        </Button>
      ) : null}

      {problem ? (
        <p role="alert" className="text-xs">
          {problem}
        </p>
      ) : null}
    </div>
  );
}
