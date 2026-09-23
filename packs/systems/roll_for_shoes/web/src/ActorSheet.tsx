import { useState } from "react";
import {
  GraphQLRequestError,
  Input,
  StatusBadge,
  postGraphQL,
  useActorSystemData,
  useUpdateResourceData,
  useUpdateTraitData,
  type ActorSheetProps,
} from "@thunderforge/host";

import {
  SYSTEM_ID,
  grantSkill,
  isAdvancement,
  newSkillId,
  skillsOf,
  spendXp,
  verdict,
  xpAward,
  xpOf,
  type Skill,
  type Verdict,
} from "./game.ts";
import { AdvancementPrompt } from "./components/AdvancementPrompt.tsx";
import { RollResult } from "./components/RollResult.tsx";
import { SkillLineage } from "./components/SkillLineage.tsx";
import {
  cardClass,
  cardTitleClass,
  fieldClass,
  hintClass,
  textareaClass,
} from "./components/styles.ts";

/**
 * The whole of Roll for Shoes.
 *
 * This is a *contributed* sheet rather than a declared one, and it has to be:
 * the declarative sheet format holds shapes of values — a number, a track, a
 * line of text — and a character here is a lineage of skills that did not
 * exist until someone invented them mid-scene. No declaration can express
 * that, and none should try.
 *
 * The rules themselves live in `game.ts` as plain functions, tested without a
 * browser. What is left here is reading, writing and drawing.
 *
 * The game is public domain (CC0 1.0), by Ben Wray — rollforshoes.com.
 */

interface Attempt {
  skill: Skill;
  faces: number[];
  total: number;
  opposition: number | null;
  result: Verdict;
  /** Dice bought with experience, this attempt. */
  bought: number;
  /** Whether the advancement this attempt earned has been answered. */
  advancementAnswered: boolean;
}

interface DieOutcome {
  finalValue: number;
}

interface RollResolution {
  dice: DieOutcome[];
  resultValue: number;
}

const ROLL_SKILL = `
  mutation RollSkill($input: RollDiceInput!) {
    rollDice(input: $input) {
      formula
      dice { numericSides rolls kept finalValue }
      resultKind
      resultValue
    }
  }
`;

export function ActorSheet({ actor, canEdit }: ActorSheetProps) {
  const { data, loading, refetch } = useActorSystemData(actor.id, SYSTEM_ID);
  const { updateTraits } = useUpdateTraitData(actor.id, SYSTEM_ID);
  const { updateResources } = useUpdateResourceData(actor.id, SYSTEM_ID);

  const [opposition, setOpposition] = useState("");
  const [attempt, setAttempt] = useState<Attempt | null>(null);
  const [description, setDescription] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const traitData = (data?.trait_data ?? {}) as Record<string, unknown>;
  const resourceData = (data?.resource_data ?? {}) as Record<string, unknown>;
  const skills = skillsOf(traitData);
  const xp = xpOf(resourceData);
  const storedDescription =
    typeof traitData["description"] === "string"
      ? traitData["description"]
      : "";

  /** Every refusal reaches the player. None is swallowed, none is a crash. */
  const reportRefusal = (thrown: unknown): void => {
    if (thrown instanceof GraphQLRequestError) {
      setError(thrown.message);
      return;
    }
    setError(
      thrown instanceof Error ? thrown.message : "Something went wrong.",
    );
  };

  const saveDescription = async (next: string): Promise<void> => {
    setError(null);
    try {
      await updateTraits({ ...traitData, description: next, skills });
      await refetch();
    } catch (thrown) {
      reportRefusal(thrown);
    }
  };

  const roll = async (skill: Skill): Promise<void> => {
    setError(null);
    setBusy(true);
    const against = opposition.trim() === "" ? null : Number(opposition);

    try {
      const { rollDice } = await postGraphQL<{ rollDice: RollResolution }>(
        ROLL_SKILL,
        {
          input: {
            worldId: actor.worldId,
            formula: "(LEVEL)d6",
            bindings: [{ name: "LEVEL", value: skill.level }],
          },
        },
      );

      const faces = rollDice.dice.map((die) => die.finalValue);
      const total = rollDice.resultValue;
      const result = verdict(total, Number.isFinite(against) ? against : null);

      setAttempt({
        skill,
        faces,
        total,
        opposition: against,
        result,
        bought: 0,
        advancementAnswered: false,
      });

      // Failure is the only thing that pays, and it pays whatever else the
      // roll did — a roll can fail and earn a new skill in the same breath.
      if (xpAward(result) > 0) {
        await updateResources({ ...resourceData, xp: xp + xpAward(result) });
        await refetch();
      }
    } catch (thrown) {
      // A pool above the dice engine's ceiling lands here, and is shown as
      // what it is: this roll was refused. The character is not capped.
      reportRefusal(thrown);
    } finally {
      setBusy(false);
    }
  };

  const buyASix = async (): Promise<void> => {
    if (!attempt) {
      return;
    }
    setError(null);

    const spend = spendXp(xp, attempt.faces, attempt.bought);
    if (!spend.allowed) {
      // Refused before anything is sent — there is nothing to ask the server.
      setError(spend.reason);
      return;
    }

    setBusy(true);
    try {
      await updateResources({ ...resourceData, xp: spend.balance });
      await refetch();
      // The verdict, the total and the experience this roll awarded are all
      // untouched. Only the count of bought dice moves.
      setAttempt({ ...attempt, bought: spend.bought });
    } catch (thrown) {
      reportRefusal(thrown);
    } finally {
      setBusy(false);
    }
  };

  const learnIt = async (name: string): Promise<void> => {
    if (!attempt) {
      return;
    }
    setError(null);

    const granted = grantSkill(attempt.skill, name, newSkillId());
    if (!granted.granted) {
      setError(granted.reason);
      return;
    }

    setBusy(true);
    try {
      await updateTraits({
        ...traitData,
        description: description ?? storedDescription,
        skills: [...skills, granted.skill],
      });
      await refetch();
      setAttempt({ ...attempt, advancementAnswered: true });
    } catch (thrown) {
      reportRefusal(thrown);
    } finally {
      setBusy(false);
    }
  };

  const advancementOffered =
    attempt !== null &&
    !attempt.advancementAnswered &&
    isAdvancement(attempt.faces, attempt.bought);

  return (
    <div data-testid="rfs-sheet" className="grid gap-4">
      {error ? (
        <StatusBadge variant="danger" data-testid="rfs-error">
          {error}
        </StatusBadge>
      ) : null}

      <div className={cardClass}>
        <div className="flex items-baseline justify-between gap-3">
          <h2 className={cardTitleClass}>{actor.label}</h2>
          <p className="text-sm">
            <span className={hintClass}>XP </span>
            <span data-testid="rfs-xp" className="font-semibold tabular-nums">
              {xp}
            </span>
          </p>
        </div>

        {canEdit ? (
          <textarea
            className={`${textareaClass} mt-2`}
            rows={2}
            placeholder="Who are they?"
            data-testid="rfs-description"
            value={description ?? storedDescription}
            onChange={(event) => setDescription(event.target.value)}
            onBlur={(event) => void saveDescription(event.target.value)}
          />
        ) : (
          <p data-testid="rfs-description" className={`${hintClass} mt-2`}>
            {storedDescription}
          </p>
        )}
      </div>

      <div className={cardClass}>
        <SkillLineage
          skills={skills}
          busy={busy || loading}
          onRoll={(skill) => void roll(skill)}
        />

        <label className="mt-3 grid gap-1">
          <span className={hintClass}>What has to be beaten</span>
          <Input
            type="number"
            inputMode="numeric"
            className={fieldClass}
            placeholder="The Game Master&rsquo;s number"
            data-testid="rfs-opposition"
            value={opposition}
            onChange={(event) => setOpposition(event.target.value)}
          />
        </label>
      </div>

      {attempt ? (
        <div className={cardClass}>
          <RollResult
            skill={attempt.skill}
            faces={attempt.faces}
            total={attempt.total}
            opposition={attempt.opposition}
            result={attempt.result}
            bought={attempt.bought}
            xp={xp}
            busy={busy}
            onSpendXp={() => void buyASix()}
          />
        </div>
      ) : null}

      {advancementOffered && attempt ? (
        <div className={cardClass}>
          <AdvancementPrompt
            parent={attempt.skill}
            busy={busy}
            onConfirm={(name) => void learnIt(name)}
            onDecline={() =>
              setAttempt({ ...attempt, advancementAnswered: true })
            }
          />
        </div>
      ) : null}
    </div>
  );
}

export default ActorSheet;
