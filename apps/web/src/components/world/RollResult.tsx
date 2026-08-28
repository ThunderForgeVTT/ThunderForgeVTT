/**
 * A roll's result, and — for the Game Master alone — a quiet note where the
 * server determined it differently (spec 028 US7, T102a; FR-065, FR-065a,
 * FR-066, FR-067).
 *
 * # The register, which is the whole of this task
 *
 * A mismatch has many innocent explanations — a stale client, an artefact of
 * a reconnect, a bug of ours — and one guilty one. Telling them apart takes
 * knowing the people at the table, which the software never will and the GM
 * already does. So this is a **difference, noted**: the result is rendered at
 * its ordinary weight, in its ordinary colour, and carries a dotted underline
 * and one muted word. Opening it shows two numbers and a sentence saying
 * nothing was changed.
 *
 * It deliberately is not a badge, not a colour, not an icon, not an alert.
 * A red mark would satisfy "rendered distinctly" and be the wrong answer: it
 * would make the software the accuser in a situation where it cannot know,
 * and would put an innocent player under suspicion in front of the one person
 * who can act on it. The visual weight is chosen to say "you may want to look
 * at this" and nothing stronger.
 *
 * # What is deliberately absent
 *
 * No accept, no reject, no report, no dismiss-with-consequence, no notice to
 * the player, no notice to the table (FR-065a, FR-066, FR-067). The outcome
 * stands exactly as it was; the only thing that changed is what one person
 * sees. There is no escalation path because inventing a technical answer to a
 * social question gets it wrong precisely in the cases that matter.
 *
 * Nothing here is transmitted anywhere. The comparison arrives with the roll
 * and is rendered; no view, no expansion and no dismissal is reported back
 * (FR-052, FR-054).
 */

import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import type { RollResolutionRecord } from "@/types/roll";
import { discrepancyToShow } from "./rollDiscrepancy";

export interface RollResultProps {
  resolution: RollResolutionRecord;
  /**
   * Whether the viewer is the Game Master. FR-067: the note exists for them
   * and for nobody else. Defaults to `false`, so a caller that has not
   * thought about roles shows an ordinary result rather than leaking a mark
   * against a player to the whole table.
   */
  isGameMaster?: boolean;
  /** Who rolled, when the caller knows. Named in the note, never elsewhere. */
  rolledBy?: string;
}

export function RollResult({ resolution, isGameMaster = false, rolledBy }: RollResultProps) {
  const discrepancy = discrepancyToShow(resolution.discrepancy, isGameMaster);

  const total = (
    <strong data-testid="roll-result-total">{resolution.resultValue}</strong>
  );

  return (
    <p data-testid="roll-result" className="flex flex-wrap items-baseline gap-1.5 text-sm">
      <span className="text-muted-foreground">{resolution.formula}:</span>
      {discrepancy ? (
        <Popover>
          {/*
            The total itself opens the note. A separate control beside it would
            read as an action to take about the roll, and there is no action to
            take — the thing worth looking at *is* the number.
          */}
          <PopoverTrigger
            data-testid="roll-discrepancy-marker"
            className="inline-flex items-baseline gap-1.5 rounded-sm underline decoration-dotted decoration-muted-foreground/70 underline-offset-4 outline-hidden focus-visible:ring-[3px] focus-visible:ring-ring/50"
            aria-label={`Result ${resolution.resultValue}. The server determined a different value; open for both.`}
          >
            {total}
            <span className="text-xs font-normal text-muted-foreground">noted</span>
          </PopoverTrigger>
          <PopoverContent
            align="start"
            data-testid="roll-discrepancy-details"
            className="text-xs"
          >
            <p className="font-medium">Two readings of this roll</p>
            <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-muted-foreground">
              <dt>Reported{rolledBy ? ` by ${rolledBy}` : ""}</dt>
              <dd data-testid="roll-discrepancy-claimed" className="text-foreground">
                {discrepancy.claimedValue}
              </dd>
              <dt>Determined here</dt>
              <dd data-testid="roll-discrepancy-determined" className="text-foreground">
                {discrepancy.determinedValue}
              </dd>
            </dl>
            {/*
              Said plainly, because the first question a GM will have is
              whether the software has already done something about it. It has
              not, and it will not (FR-066).
            */}
            <p className="text-muted-foreground">
              The result stands as rolled. Nothing has been changed or reported, and only you
              can see this note.
            </p>
          </PopoverContent>
        </Popover>
      ) : (
        total
      )}
    </p>
  );
}

export default RollResult;
