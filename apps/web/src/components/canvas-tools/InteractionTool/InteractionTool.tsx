import { useCallback, useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import {
  createInteractive,
  deleteInteractive,
  getInteractives,
  updateInteractive,
  type Interactive,
  type SubjectKind,
} from "@/api/interactives";
import { getWorldLoreEntries } from "@/api/lore";
import {
  InteractionAuthor,
  DoorControls,
  type ReferenceChoice,
} from "@/components/InteractionAuthor";
import type { WorldStore } from "@/engine/world/store";
import { refreshInteractives } from "@/engine/world/sync/interactives";
import type { WorldLight, WorldWall } from "@/engine/world/types";

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
        <p className="text-xs text-muted-foreground">
          Select a token or a wall to give it something to do.
        </p>
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
