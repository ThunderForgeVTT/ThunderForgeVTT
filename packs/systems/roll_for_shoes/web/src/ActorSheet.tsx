import { useEffect, useRef, useState } from "react";
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
  BAND_DICE,
  BAND_TARGET,
  DEFAULT_SETTINGS,
  SYSTEM_ID,
  addStatus,
  boughtSlotsAt,
  buySlot,
  grantSkill,
  newSkillId,
  newStatusId,
  removeStatus,
  resolve,
  skillsOf,
  slotCost,
  slotsAvailable,
  spendXp,
  statusesOf,
  withBoughtSlot,
  xpAward,
  xpOf,
  type Band,
  type Skill,
  type Status,
  type Verdict,
  type WorldSettings,
} from "./game.ts";
import { fetchWorldSettings } from "./settings.ts";
import {
  boughtLine,
  learnedLine,
  recallAttempt,
  rememberAttempt,
  rollLine,
  tellTheTable,
  watchSheet,
  type Attempt,
} from "./table.ts";
import { AdvancementPrompt } from "./components/AdvancementPrompt.tsx";
import { DifficultyPicker } from "./components/DifficultyPicker.tsx";
import { StatusList } from "./components/StatusList.tsx";
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

  const [settings, setSettings] = useState<WorldSettings>(DEFAULT_SETTINGS);
  const [opposition, setOpposition] = useState("");
  const [band, setBand] = useState<Band | null>(null);
  const [gmDice, setGmDice] = useState<number[] | null>(null);
  // Begun from what this tab last rolled for this character: the dock
  // unmounts the sheet when its tab changes, and an advancement nobody has
  // answered yet must still be there when the player comes back.
  const [attempt, setAttemptState] = useState<Attempt | null>(() =>
    recallAttempt(actor.id),
  );
  const setAttempt = (next: Attempt | null): void => {
    rememberAttempt(actor.id, next);
    setAttemptState(next);
  };
  // `loading` until the world's rules are known, `failed` when they could not
  // be read. Neither rolls: see the effect below.
  const [settingsState, setSettingsState] = useState<
    "loading" | "ready" | "failed"
  >("loading");
  const [settingsTry, setSettingsTry] = useState(0);
  const advancementRef = useRef<HTMLDivElement | null>(null);
  const [description, setDescription] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Read when the sheet opens. The settings describe how the table plays, not
  // what this character is, so they do not change under a roll.
  //
  // A failed read is said, and holds the sheet. It used to fall back to the
  // core game in silence, on the reasoning that an unconfigured world and an
  // unreadable one want the same sheet — but they do not: a character nobody
  // has opened yet is given this world's starting skills on first write, and
  // writing the core game's `Do Anything` into a world that starts people
  // elsewhere is permanent. The server answers an unconfigured world with the
  // defaults itself, so a failure here is only ever a failure.
  useEffect(() => {
    let cancelled = false;
    void fetchWorldSettings(actor.worldId)
      .then((stored) => {
        if (!cancelled) {
          setSettings(stored);
          setSettingsState("ready");
        }
      })
      .catch(() => {
        if (!cancelled) {
          setSettingsState("failed");
        }
      });
    return () => {
      cancelled = true;
    };
  }, [actor.worldId, settingsTry]);

  // Someone else with this character open — the Game Master, usually — writes
  // whole slots, as this sheet does. Re-reading on the server's word keeps the
  // next write here from being made over a copy that is already stale.
  useEffect(
    () => watchSheet(actor.worldId, actor.id, () => void refetch()),
    [actor.worldId, actor.id, refetch],
  );

  // The dock draws this sheet in a short scrolling box, and the prompt is the
  // last thing in it: a new skill offered below the fold is a skill missed.
  const offered =
    attempt !== null &&
    !attempt.advancementAnswered &&
    attempt.faces.length > 0 &&
    attempt.faces.filter((face) => face === 6).length + attempt.bought >=
      attempt.faces.length;
  useEffect(() => {
    if (offered) {
      advancementRef.current?.scrollIntoView({ block: "nearest" });
    }
  }, [offered]);

  /**
   * Say it in the world's chat. A refusal is shown, and changes nothing else:
   * the dice were the server's and the sheet has them.
   */
  const say = async (line: string): Promise<void> => {
    try {
      await tellTheTable(actor.worldId, line);
    } catch (thrown) {
      setError(
        `The table was not told: ${
          thrown instanceof Error ? thrown.message : "the message was refused."
        }`,
      );
    }
  };

  const traitData = (data?.trait_data ?? {}) as Record<string, unknown>;
  const resourceData = (data?.resource_data ?? {}) as Record<string, unknown>;
  // The settings are handed over so a character nobody has opened yet is given
  // what this world starts people with. A character who has stored skills does
  // not reach that branch, which is the whole of "changing the setting never
  // alters anyone who already exists" (FR-039).
  const skills = skillsOf(traitData, settings);
  const statuses = statusesOf(traitData);
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

  /**
   * One d6 pool from the dice engine.
   *
   * The character's pool and the Game Master's go through the same call with
   * the same shape of formula, because they are the same act — dice are dice.
   * What differs is only where the answer is put afterwards, and that
   * difference is the whole of FR-014: these two arrays are never merged.
   */
  const rollPool = async (
    name: string,
    count: number,
  ): Promise<{ faces: number[] }> => {
    const { rollDice } = await postGraphQL<{ rollDice: RollResolution }>(
      ROLL_SKILL,
      {
        input: {
          worldId: actor.worldId,
          formula: `(${name})d6`,
          bindings: [{ name, value: count }],
        },
      },
    );
    return { faces: rollDice.dice.map((die) => die.finalValue) };
  };

  /**
   * The Game Master picked how hard it is.
   *
   * Both modes finish by writing a number into the opposition field, which the
   * player can still read and still change. The band is a way of arriving at
   * the opposition, not a second kind of it.
   */
  const chooseBand = async (chosen: Band): Promise<void> => {
    setError(null);
    setBand(chosen);

    if (settings.difficultyMode === "target") {
      setGmDice(null);
      setOpposition(String(BAND_TARGET[chosen]));
      return;
    }

    setBusy(true);
    try {
      const { faces } = await rollPool("BAND", BAND_DICE[chosen]);
      setGmDice(faces);
      setOpposition(String(faces.reduce((running, face) => running + face, 0)));
    } catch (thrown) {
      reportRefusal(thrown);
    } finally {
      setBusy(false);
    }
  };

  const roll = async (skill: Skill): Promise<void> => {
    setError(null);
    setBusy(true);
    const against = opposition.trim() === "" ? null : Number(opposition);

    try {
      // `faces` is the character's pool and nothing else. The Game Master's
      // dice live in their own state and never reach this array, which is what
      // keeps a six of theirs from earning anybody a skill (FR-014).
      const { faces } = await rollPool("LEVEL", skill.level);
      // Every roll goes through the one resolver, whatever this world has
      // turned on. With nothing on it is spec 061's comparison exactly, and
      // `game.test.ts` checks that exhaustively rather than by example.
      const outcome = resolve({
        faces,
        // A world that has not turned statuses on rolls with none, whatever a
        // character happens to have stored from a world that had — the setting
        // decides whether they apply, never whether they exist.
        statuses: settings.statusesEnabled ? statuses : [],
        opposition: Number.isFinite(against) ? against : null,
        tieSucceeds: settings.tieSucceeds,
      });
      const result = outcome.verdict;

      const made: Attempt = {
        skill,
        faces,
        total: outcome.total,
        modifier: outcome.modifier,
        opposition: against,
        result,
        bought: 0,
        advancementAnswered: false,
      };
      setAttempt(made);
      await say(rollLine(actor.label, made));

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
      const boughtInto = { ...attempt, bought: spend.bought };
      setAttempt(boughtInto);
      await say(boughtLine(actor.label, boughtInto));
    } catch (thrown) {
      reportRefusal(thrown);
    } finally {
      setBusy(false);
    }
  };

  /**
   * Statuses are stored on the character, next to the skills.
   *
   * They go through the same `updateTraits` every other trait write uses, so a
   * refusal — a paused world, a lost permission — is badged like any other
   * rather than leaving the sheet showing a status the server never took.
   */
  const writeStatuses = async (next: Status[]): Promise<void> => {
    setBusy(true);
    try {
      await updateTraits({
        ...traitData,
        description: description ?? storedDescription,
        skills,
        statuses: next,
      });
      await refetch();
    } catch (thrown) {
      reportRefusal(thrown);
    } finally {
      setBusy(false);
    }
  };

  const addAStatus = async (name: string, modifier: number): Promise<void> => {
    setError(null);
    const added = addStatus(statuses, name, modifier, newStatusId());
    if (!added.added) {
      setError(added.reason);
      return;
    }
    await writeStatuses(added.statuses);
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
      await say(learnedLine(actor.label, granted.skill));
    } catch (thrown) {
      reportRefusal(thrown);
    } finally {
      setBusy(false);
    }
  };

  const advancementOffered = offered;

  // Where the new skill would sit, and whether there is room for it there.
  //
  // `null` room means no cap, and that is every world that has not turned
  // skill slots on — so with the setting off this is exactly the prompt spec
  // 061 shipped, by the same route rather than by a second branch.
  const newLevel = attempt ? attempt.skill.level + 1 : 1;
  const boughtHere = boughtSlotsAt(resourceData, newLevel);
  const roomForIt = settings.skillSlotsEnabled
    ? slotsAvailable(skills, newLevel, boughtHere)
    : null;

  /**
   * Buy room at the level the advancement wants to sit at.
   *
   * Against the same `resource_data.xp` a die-into-a-six is bought from, which
   * is what keeps the ledger honest across both kinds of spend (FR-035): there
   * is one balance, so there is nothing to reconcile.
   */
  const buyRoom = async (): Promise<void> => {
    setError(null);
    const purchase = buySlot(xp, newLevel, boughtHere);
    if (!purchase.bought) {
      setError(purchase.reason);
      return;
    }

    setBusy(true);
    try {
      await updateResources({
        ...resourceData,
        xp: purchase.balance,
        boughtSlots: withBoughtSlot(resourceData, newLevel, purchase.slots),
      });
      await refetch();
    } catch (thrown) {
      reportRefusal(thrown);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div data-testid="rfs-sheet" className="grid gap-4">
      {error ? (
        <StatusBadge variant="danger" data-testid="rfs-error">
          {error}
        </StatusBadge>
      ) : null}

      {settingsState === "failed" ? (
        <div className="flex flex-wrap items-center gap-2">
          <StatusBadge variant="danger" data-testid="rfs-settings-failed">
            This world&rsquo;s rules could not be read, so nothing can be rolled
            yet.
          </StatusBadge>
          <button
            type="button"
            className="text-sm underline"
            data-testid="rfs-settings-retry"
            onClick={() => {
              setSettingsState("loading");
              setSettingsTry((tries) => tries + 1);
            }}
          >
            Try again
          </button>
        </div>
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
          busy={busy || loading || settingsState !== "ready"}
          onRoll={(skill) => void roll(skill)}
        />

        {settings.difficultyMode === "free" ? null : (
          <DifficultyPicker
            mode={settings.difficultyMode}
            chosen={band}
            gmDice={gmDice}
            busy={busy || loading || settingsState !== "ready"}
            onChoose={(chosen) => void chooseBand(chosen)}
          />
        )}

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

      {settings.statusesEnabled ? (
        <div className={cardClass}>
          <StatusList
            statuses={statuses}
            canEdit={canEdit}
            busy={busy || loading || settingsState !== "ready"}
            onAdd={(name, modifier) => void addAStatus(name, modifier)}
            onRemove={(id) => void writeStatuses(removeStatus(statuses, id))}
          />
        </div>
      ) : null}

      {attempt ? (
        <div className={cardClass}>
          <RollResult
            skill={attempt.skill}
            faces={attempt.faces}
            total={attempt.total}
            modifier={attempt.modifier}
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
        <div className={cardClass} ref={advancementRef}>
          <AdvancementPrompt
            parent={attempt.skill}
            busy={busy}
            room={roomForIt}
            xp={xp}
            slotCost={slotCost(newLevel)}
            onBuySlot={
              settings.skillSlotsEnabled ? () => void buyRoom() : undefined
            }
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
