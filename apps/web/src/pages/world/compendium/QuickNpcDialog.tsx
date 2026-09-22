import { Suspense, useState } from "react";
import { freshSeed, HeroPreview } from "@thunderforge/hero-builder";
import { randomHero, type HeroSpec } from "@thunderforge/heroes";
import { createActor, type ActorImageRecord } from "@/api/actors";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";
import { LazyHeroBuilderDialog } from "@/pages/world/actor/heroBuilderLazy";
import { saveBuiltHero, savedImages } from "@/pages/world/actor/saveBuiltHero";
import type { WorldActorRecord } from "@/types/actor";

/**
 * Spec 044 FR-027, FR-028: a named NPC with a face, from a name and one press.
 *
 * The roll is "any" race: a new NPC has no sheet yet, so there is nothing to
 * read one from. Loaded lazily by the compendium (`heroBuilderLazy.ts`).
 *
 * # Why a failed upload keeps the NPC
 *
 * The NPC is what the Game Master asked for; the art is the easy half to add
 * again. Deleting the NPC to make the failure disappear would lose the one
 * thing that worked and hide why. So it stays, the list marks it as lacking
 * art, and this dialog says so and names the row's "Build look" as the way
 * to finish — and reports no success.
 */
export interface QuickNpcDialogProps {
  worldId: string;
  open: boolean;
  onOpenChange(open: boolean): void;
  /** The NPC exists; `images` is whatever art was stored with it. */
  onCreated(actor: WorldActorRecord, images: ActorImageRecord[]): void;
}

type Look = Omit<HeroSpec, "name">;

const MAX_NAME = 80;

export default function QuickNpcDialog({
  worldId,
  open,
  onOpenChange,
  onCreated,
}: QuickNpcDialogProps) {
  const [name, setName] = useState("");
  const [look, setLook] = useState<Look>(() => randomHero(freshSeed()));
  const [building, setBuilding] = useState(false);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lacksArt, setLacksArt] = useState<string | null>(null);

  const trimmed = name.trim();
  const spec: HeroSpec = { ...look, name: trimmed || "New NPC" };

  const create = async () => {
    if (!trimmed) return;
    setCreating(true);
    setError(null);
    try {
      let actor: WorldActorRecord;
      try {
        actor = await createActor({ worldId, label: trimmed, isNpc: true });
      } catch (err) {
        setError(err instanceof Error ? err.message : "The NPC was refused.");
        return;
      }
      const saved = await saveBuiltHero(actor.id, spec);
      const images = savedImages(saved.portrait, saved.token);
      onCreated(actor, images);
      if (images.length === 2) {
        onOpenChange(false);
        return;
      }
      const failed = [saved.portrait, saved.token].find(
        (outcome) => outcome.status === "failed",
      );
      setLacksArt(
        failed?.status === "failed" ? failed.message : "The upload failed.",
      );
    } finally {
      setCreating(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="max-h-dvh overflow-y-auto sm:max-w-md"
        data-testid="quick-npc-dialog"
      >
        <div className="grid gap-1 pr-10">
          <DialogTitle>Quick NPC</DialogTitle>
          <DialogDescription>
            A name and a face. Everything else can come later.
          </DialogDescription>
        </div>

        {lacksArt ? (
          <div
            role="alert"
            className="grid gap-2 text-sm"
            data-testid="quick-npc-lacks-art"
          >
            <p>
              {trimmed} was created, but without its art: {lacksArt}
            </p>
            <p>Use “Build look” on its row to add a portrait and a token.</p>
            <Button
              type="button"
              variant="secondary"
              className="justify-self-start"
              onClick={() => onOpenChange(false)}
              data-testid="quick-npc-close"
            >
              Back to the list
            </Button>
          </div>
        ) : (
          <form
            className="grid gap-3"
            onSubmit={(event) => {
              event.preventDefault();
              void create();
            }}
          >
            <label className="grid gap-1 text-sm font-medium">
              Name
              <Input
                value={name}
                maxLength={MAX_NAME}
                onChange={(event) => setName(event.target.value)}
                data-testid="quick-npc-name"
                autoFocus
              />
            </label>

            {/* Reserved (FR-027, Decisions 2): a later spec puts a system's
                NPC template choice here, below the name, without moving
                anything else. */}
            <div data-testid="quick-npc-template-space" aria-hidden="true" />

            <div data-testid="quick-npc-preview">
              <HeroPreview spec={spec} idPrefix="quick-npc" size="sm" />
            </div>

            <div className="flex flex-wrap gap-2">
              <Button
                type="button"
                variant="secondary"
                onClick={() => setLook(randomHero(freshSeed()))}
                data-testid="quick-npc-reroll"
              >
                Reroll
              </Button>
              <Button
                type="button"
                variant="secondary"
                onClick={() => setBuilding(true)}
                data-testid="quick-npc-open-builder"
              >
                Open in builder
              </Button>
            </div>

            {error ? (
              <p
                role="alert"
                className="text-sm text-destructive"
                data-testid="quick-npc-error"
              >
                {error}
              </p>
            ) : null}

            <Button
              type="submit"
              disabled={!trimmed || creating}
              className="justify-self-start"
              data-testid="quick-npc-create"
            >
              {creating ? "Creating…" : "Create"}
            </Button>
          </form>
        )}

        {building ? (
          <Suspense fallback={null}>
            <LazyHeroBuilderDialog
              open
              worldId={worldId}
              onOpenChange={setBuilding}
              actorId={null}
              actorLabel={trimmed || "New NPC"}
              existingRoles={[]}
              initialSpec={spec}
              onUse={(chosen) => {
                const { name: builtName, ...chosenLook } = chosen;
                setLook(chosenLook);
                if (builtName.trim() && builtName !== "New NPC") {
                  setName(builtName);
                }
              }}
            />
          </Suspense>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}
