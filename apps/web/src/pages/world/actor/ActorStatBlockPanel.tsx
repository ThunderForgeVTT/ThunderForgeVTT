import { useState } from "react";
import type { StatBlockPlan, StatBlockSlots } from "@thunderforge/host";
import { fetchActorSystemData } from "@/api/actorSystemData";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { applyStatBlock } from "@/pages/world/actor/applyStatBlock";
import { resolveStatBlocks } from "@/pages/world/actor/systemStatBlocks";
import type { WorldActorRecord } from "@/types/actor";

/**
 * "Apply stat block…" on an existing NPC.
 *
 * The bestiary puts a block on a creature as it makes it. This is the other
 * half: the NPC a Game Master already has (made by hand, by Quick NPC, or
 * last session) picks up a block from its own page. Renders nothing for a
 * system that ships no stat blocks, which is most of them.
 *
 * # Why it asks before replacing
 *
 * A block writes whole slots: scores, hit points, proficiencies. On an empty
 * sheet that is the point. On one a Game Master has already filled in, it is
 * the loss of what they typed, so a sheet holding any numbers gets a second
 * step that says what goes, and the first click writes nothing.
 *
 * What a block has no opinion on is kept. Which fields those are is the
 * pack's to say (`plan` is handed the current sheet); this panel only knows
 * that something is there.
 */
export interface ActorStatBlockPanelProps {
  actor: WorldActorRecord;
  /** The sheet and the ability list are stale; the page should reload them. */
  onApplied(): void;
}

function holdsAnything(slots: StatBlockSlots): boolean {
  return Object.values(slots).some(
    (slot) => slot && Object.keys(slot).length > 0,
  );
}

export function ActorStatBlockPanel({
  actor,
  onApplied,
}: ActorStatBlockPanelProps) {
  const source = resolveStatBlocks(actor.gameSystemId);
  const [blockId, setBlockId] = useState(source?.blocks[0]?.id ?? "");
  const [pending, setPending] = useState<StatBlockPlan | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [complaints, setComplaints] = useState<string[]>([]);

  const gameSystemId = actor.gameSystemId;
  if (!source || !gameSystemId || source.blocks.length === 0) {
    return null;
  }
  const chosen = source.blocks.find((block) => block.id === blockId);

  const write = async (plan: StatBlockPlan) => {
    setBusy(true);
    try {
      const outcome = await applyStatBlock(
        { id: actor.id, worldId: actor.worldId, gameSystemId },
        plan,
      );
      setComplaints(outcome.complaints);
      setStatus(
        outcome.applied
          ? `${plan.name} applied. Hit points are the average; change anything on the sheet.`
          : null,
      );
      if (outcome.applied) {
        onApplied();
      }
    } finally {
      setPending(null);
      setBusy(false);
    }
  };

  const begin = async () => {
    setStatus(null);
    setComplaints([]);
    setBusy(true);
    try {
      const data = await fetchActorSystemData(actor.id);
      const current: StatBlockSlots = {
        ability_data: data?.abilityData ?? undefined,
        resource_data: data?.resourceData ?? undefined,
        proficiency_data: data?.proficiencyData ?? undefined,
        trait_data: data?.traitData ?? undefined,
      };
      const plan = source.plan(blockId, current);
      if (!plan) {
        setComplaints(["That stat block is no longer available."]);
        return;
      }
      if (holdsAnything(current)) {
        setPending(plan);
        return;
      }
      await write(plan);
    } catch (error) {
      setComplaints([
        error instanceof Error ? error.message : "The sheet could not be read.",
      ]);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card className="grid gap-3 p-4" data-testid="actor-stat-block">
      <h2 className="text-sm font-semibold tracking-wide text-muted-foreground uppercase">
        Stat block
      </h2>
      <div className="flex flex-wrap items-end gap-2">
        <label className="grid min-w-0 flex-1 gap-1 text-sm font-medium">
          Creature
          <select
            className="h-9 min-w-0 rounded-md border border-input bg-background px-2 text-sm"
            value={blockId}
            disabled={busy || pending !== null}
            onChange={(event) => setBlockId(event.target.value)}
            data-testid="actor-stat-block-select"
          >
            {source.blocks.map((block) => (
              <option key={block.id} value={block.id}>
                {block.name}
              </option>
            ))}
          </select>
        </label>
        <Button
          type="button"
          variant="secondary"
          disabled={busy || pending !== null}
          onClick={() => void begin()}
          data-testid="actor-stat-block-apply"
        >
          Apply stat block…
        </Button>
      </div>
      {chosen ? (
        <p className="text-sm text-muted-foreground">{chosen.summary}</p>
      ) : null}

      {pending ? (
        <div
          role="alertdialog"
          aria-label="Replace this sheet's numbers"
          className="grid gap-2 rounded-md border border-border p-3 text-sm"
          data-testid="actor-stat-block-confirm"
        >
          <p>
            This sheet already has numbers on it. Applying {pending.name}{" "}
            replaces its scores, hit points, defence, proficiencies, size,
            speeds and senses. Notes are kept.
          </p>
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              disabled={busy}
              onClick={() => void write(pending)}
              data-testid="actor-stat-block-replace"
            >
              Replace
            </Button>
            <Button
              type="button"
              variant="secondary"
              disabled={busy}
              onClick={() => setPending(null)}
              data-testid="actor-stat-block-cancel"
            >
              Keep what is there
            </Button>
          </div>
        </div>
      ) : null}

      {status ? (
        <p
          role="status"
          className="text-sm"
          data-testid="actor-stat-block-status"
        >
          {status}
        </p>
      ) : null}
      {complaints.length > 0 ? (
        <ul
          role="alert"
          className="grid gap-1 text-sm text-destructive"
          data-testid="actor-stat-block-problems"
        >
          {complaints.map((complaint) => (
            <li key={complaint}>{complaint}</li>
          ))}
        </ul>
      ) : null}
    </Card>
  );
}
