import { useMemo, useState } from "react";
import { freshSeed, HeroPreview } from "@thunderforge/hero-builder";
import { BESTIARY, monsterSpec, type HeroSpec } from "@thunderforge/heroes";
import { updateActorSystemData } from "@/api/actorSystemData";
import { createActor, type ActorImageRecord } from "@/api/actors";
import { getGameSystemManifest } from "@/api/gameSystems";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";
import { applyStatBlock } from "@/pages/world/actor/applyStatBlock";
import { saveBuiltHero, savedImages } from "@/pages/world/actor/saveBuiltHero";
import {
  resolveStatBlocks,
  statBlockForCreature,
} from "@/pages/world/actor/systemStatBlocks";
import type { WorldActorRecord } from "@/types/actor";
import { declaredSizesOf, sizeCategoryOf, slotKey } from "@/utils/sizeCategory";

/**
 * Spec 047 FR-030, FR-032, FR-020: monsters from the bestiary, several at a
 * time, each with its own face and its size on its sheet.
 *
 * Loaded lazily by the compendium (`heroBuilderLazy.ts`), for the same reason
 * Quick NPC is: the drawings stay out of the list's chunk.
 *
 * # Why the size is written to the sheet
 *
 * The board takes a token's footprint from the sheet, through the field the
 * world's system declares in `combat.sizes.source` — not from the drawing. A
 * creature drawn Large and left unsized would stand on one square. So the
 * size the bestiary read is written where the system keeps sizes, when the
 * system declares that size; a system with no sizes, or without this one, is
 * left alone rather than given a value it never declared.
 *
 * # Where the numbers come from
 *
 * Not from here. The bestiary is drawings and a size; it holds no statistic
 * and must not (`packages/heroes/src/bestiary.ts`). But the world's game
 * system may publish a stat block for the creature (`systemStatBlocks.ts`),
 * and when it does each monster made here gets it: hit points, defence,
 * scores and its attacks, so six goblins can fight the moment they exist.
 * Every one gets the same block, with the average hit points the block
 * prints; a Game Master who wants them different edits the sheet. A system
 * with no block for the creature leaves the numbers to the Game Master, as
 * this dialog always did.
 *
 * # Why a failure keeps what was made
 *
 * As with Quick NPC: a monster that exists without its art, or without its
 * size, is still what the Game Master asked for. It stays, the dialog names
 * what is missing, and nothing is reported as a clean success.
 */
export interface BestiaryMonster {
  actor: WorldActorRecord;
  /** Whatever art was stored with it. */
  images: ActorImageRecord[];
}

export interface BestiaryDialogProps {
  worldId: string;
  /**
   * The world's game system, when the caller knows it. Only used to say,
   * before anything is made, whether the creature will come with statistics;
   * what is written goes by each actor's own system.
   */
  gameSystemId?: string | null;
  open: boolean;
  onOpenChange(open: boolean): void;
  /** Every monster that now exists, once, after the last one is made. */
  onCreated(monsters: BestiaryMonster[]): void;
}

const MAX_NAME = 80;
/** A pack is an encounter's worth; a horde is made by pressing twice. */
const MAX_COUNT = 12;

/**
 * Puts the system's stat block for this creature on the actor, if it has
 * one. `applied` is whether the block's numbers are on the sheet; the size is
 * among them, so the caller need not write it again.
 */
async function writeStatBlock(
  actor: WorldActorRecord,
  slug: string,
): Promise<{ applied: boolean; complaints: string[] }> {
  const block = statBlockForCreature(actor.gameSystemId, slug);
  const plan = block
    ? resolveStatBlocks(actor.gameSystemId)?.plan(block.id, null)
    : null;
  if (!plan || !actor.gameSystemId) {
    return { applied: false, complaints: [] };
  }
  return applyStatBlock(
    { id: actor.id, worldId: actor.worldId, gameSystemId: actor.gameSystemId },
    plan,
  );
}

/** Writes the creature's size where the actor's system keeps sizes. Returns
 * a complaint, or null when it was written or the system declares none. */
async function writeSize(
  actor: WorldActorRecord,
  size: string,
): Promise<string | null> {
  if (!actor.gameSystemId) return null;
  try {
    const sizes = declaredSizesOf(
      await getGameSystemManifest(actor.gameSystemId),
    );
    if (!sizeCategoryOf(sizes, size) || !sizes) return null;
    await updateActorSystemData(
      actor.id,
      actor.gameSystemId,
      slotKey(sizes.source.slot) as Parameters<typeof updateActorSystemData>[2],
      { [sizes.source.field]: size },
    );
    return null;
  } catch (err) {
    return err instanceof Error ? err.message : "The size was refused.";
  }
}

export default function BestiaryDialog({
  worldId,
  gameSystemId,
  open,
  onOpenChange,
  onCreated,
}: BestiaryDialogProps) {
  const [slug, setSlug] = useState(BESTIARY[0]!.slug);
  const [name, setName] = useState(BESTIARY[0]!.source.name);
  const [count, setCount] = useState(1);
  const [seed, setSeed] = useState(freshSeed);
  const [creating, setCreating] = useState(false);
  const [problems, setProblems] = useState<string[] | null>(null);

  const entry = BESTIARY.find((candidate) => candidate.slug === slug)!;
  const trimmed = name.trim();
  const statBlock = statBlockForCreature(gameSystemId, slug);

  // One spec per monster, each from its own seed derived from the shared one
  // (the derivation `monsterPack` uses), so six goblins are six goblins and
  // the preview is exactly what Create will store.
  const specs: HeroSpec[] = useMemo(
    () =>
      Array.from({ length: count }, (_, i) => ({
        ...monsterSpec({ ...entry.source, seed: `${seed}#${i}` }),
        name:
          count > 1
            ? `${trimmed || entry.source.name} ${i + 1}`
            : trimmed || entry.source.name,
      })),
    [entry, seed, count, trimmed],
  );

  const create = async () => {
    if (!trimmed) return;
    setCreating(true);
    setProblems(null);
    const made: BestiaryMonster[] = [];
    const complaints: string[] = [];
    try {
      for (const spec of specs) {
        let actor: WorldActorRecord;
        try {
          actor = await createActor({ worldId, label: spec.name, isNpc: true });
        } catch (err) {
          complaints.push(
            `${spec.name} was not created: ${
              err instanceof Error ? err.message : "it was refused."
            }`,
          );
          // The rest would be refused for the same reason.
          break;
        }
        const saved = await saveBuiltHero(actor.id, spec);
        const images = savedImages(saved.portrait, saved.token);
        if (images.length < 2) {
          const failed = [saved.portrait, saved.token].find(
            (outcome) => outcome.status === "failed",
          );
          complaints.push(
            `${spec.name} was created without its art: ${
              failed?.status === "failed"
                ? failed.message
                : "the upload failed."
            }`,
          );
        }
        const statBlock = await writeStatBlock(actor, slug);
        for (const complaint of statBlock.complaints) {
          complaints.push(`${spec.name}: ${complaint}`);
        }
        const sizeProblem =
          spec.size && !statBlock.applied
            ? await writeSize(actor, spec.size)
            : null;
        if (sizeProblem) {
          complaints.push(
            `${spec.name} was created without its size: ${sizeProblem}`,
          );
        }
        made.push({ actor, images });
      }
    } finally {
      setCreating(false);
    }
    if (made.length > 0) onCreated(made);
    if (complaints.length > 0) {
      setProblems(complaints);
      return;
    }
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="max-h-dvh overflow-y-auto sm:max-w-lg"
        data-testid="bestiary-dialog"
      >
        <div className="grid gap-1 pr-10">
          <DialogTitle>Bestiary</DialogTitle>
          <DialogDescription>
            A creature, how many, and each one its own. Statistics come from
            your game system, not from here.
          </DialogDescription>
        </div>

        {problems ? (
          <div
            role="alert"
            className="grid gap-2 text-sm"
            data-testid="bestiary-problems"
          >
            <ul className="grid gap-1">
              {problems.map((problem) => (
                <li key={problem}>{problem}</li>
              ))}
            </ul>
            <Button
              type="button"
              variant="secondary"
              className="justify-self-start"
              onClick={() => onOpenChange(false)}
              data-testid="bestiary-close"
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
              Creature
              <select
                className="h-9 rounded-md border border-input bg-background px-2 text-sm"
                value={slug}
                onChange={(event) => {
                  const next = BESTIARY.find(
                    (candidate) => candidate.slug === event.target.value,
                  )!;
                  // The name follows the creature unless it was changed by
                  // hand, which is the Game Master naming their monster.
                  if (!trimmed || trimmed === entry.source.name) {
                    setName(next.source.name);
                  }
                  setSlug(next.slug);
                }}
                data-testid="bestiary-creature"
              >
                {BESTIARY.map((candidate) => (
                  <option key={candidate.slug} value={candidate.slug}>
                    {candidate.source.name}
                  </option>
                ))}
              </select>
            </label>

            <div className="grid grid-cols-[1fr_6rem] gap-3">
              <label className="grid gap-1 text-sm font-medium">
                Name
                <Input
                  value={name}
                  maxLength={MAX_NAME}
                  onChange={(event) => setName(event.target.value)}
                  data-testid="bestiary-name"
                />
              </label>
              <label className="grid gap-1 text-sm font-medium">
                How many
                <Input
                  type="number"
                  min={1}
                  max={MAX_COUNT}
                  value={count}
                  onChange={(event) => {
                    const next = Math.trunc(Number(event.target.value));
                    if (Number.isFinite(next)) {
                      setCount(Math.min(MAX_COUNT, Math.max(1, next)));
                    }
                  }}
                  data-testid="bestiary-count"
                />
              </label>
            </div>

            <p className="text-sm text-muted-foreground">
              Size:{" "}
              <span className="capitalize" data-testid="bestiary-size">
                {specs[0]!.size ?? "medium"}
              </span>
            </p>

            {statBlock ? (
              <p
                className="text-sm text-muted-foreground"
                data-testid="bestiary-stat-block"
              >
                Comes with the {statBlock.name} stat block: {statBlock.summary}.{" "}
                {count > 1
                  ? "Each one gets the same numbers, with average hit points."
                  : "Hit points are the average."}{" "}
                You can change any of it on the sheet.
              </p>
            ) : null}

            <div
              className="flex flex-wrap gap-2"
              data-testid="bestiary-preview"
            >
              {specs.map((spec, i) => (
                <HeroPreview
                  key={`${slug}-${seed}-${i}`}
                  spec={spec}
                  idPrefix={`bestiary-${i}`}
                  size="sm"
                />
              ))}
            </div>

            <Button
              type="button"
              variant="secondary"
              className="justify-self-start"
              onClick={() => setSeed(freshSeed())}
              data-testid="bestiary-reroll"
            >
              Reroll
            </Button>

            <Button
              type="submit"
              disabled={!trimmed || creating}
              className="justify-self-start"
              data-testid="bestiary-create"
            >
              {creating
                ? "Creating…"
                : count > 1
                  ? `Create ${count}`
                  : "Create"}
            </Button>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
}
