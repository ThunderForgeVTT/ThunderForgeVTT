/**
 * What a roll owes the rest of the table.
 *
 * A roll in this game is a public act: the Game Master names what has to be
 * beaten, everyone watches the dice, and a row of sixes is the table's event
 * as much as the player's. A sheet that keeps its result to the browser it
 * was rolled in leaves the Game Master taking the player's word for it.
 *
 * So three things live here, and each is the sheet keeping faith with
 * somebody who is not looking at it:
 *
 * - the roll is **said in the world's chat**, where everyone already reads;
 * - the attempt is **remembered across the sheet closing**, because the dock
 *   unmounts a sheet when its tab changes and an advancement nobody answered
 *   is a skill lost;
 * - the sheet **re-reads when the server says it changed**, so a Game Master
 *   and a player with the same character open do not overwrite each other
 *   from stale copies.
 *
 * All of it goes through `@thunderforge/host`, as the rest of the pack does.
 */

import {
  postGraphQL,
  subscribeToWorldEvents,
  type WorldEventLike,
} from "@thunderforge/host";

import { isAdvancement, type Skill, type Verdict } from "./game.ts";

export interface Attempt {
  skill: Skill;
  faces: number[];
  total: number;
  /** What the statuses came to for this roll; already inside `total`. */
  modifier: number;
  opposition: number | null;
  result: Verdict;
  /** Dice bought with experience, this attempt. */
  bought: number;
  /** Whether the advancement this attempt earned has been answered. */
  advancementAnswered: boolean;
  /**
   * When it was rolled. Two rolls can fall identically, and this is what
   * tells the sheet the second is a new one. Absent on an attempt remembered
   * by a build from before it was recorded.
   */
  at?: number;
}

function signed(value: number): string {
  return value < 0 ? `−${Math.abs(value)}` : `+${value}`;
}

/**
 * The roll, as one line anybody at the table can read without the sheet.
 *
 * It says the dice and not only the total, because the dice are what decide
 * an advancement and the total alone would hide it.
 */
export function rollLine(who: string, attempt: Attempt): string {
  const dice = attempt.faces.join(" + ");
  const summed =
    attempt.faces.length > 1 || attempt.modifier !== 0
      ? `${dice}${
          attempt.modifier !== 0
            ? ` ${signed(attempt.modifier)} (statuses)`
            : ""
        } = ${attempt.total}`
      : dice;

  let line = `${who} rolls ${attempt.skill.name} (${attempt.skill.level}d6): ${summed}`;
  if (attempt.result === "unjudged" || attempt.opposition === null) {
    line += ".";
  } else if (attempt.result === "success") {
    line += ` against ${attempt.opposition} — succeeds.`;
  } else {
    line += ` against ${attempt.opposition} — fails, and earns 1 XP.`;
  }
  if (isAdvancement(attempt.faces, 0)) {
    line += " All sixes: a new skill is on offer.";
  }
  return line;
}

/** A die bought into a six, said aloud for the same reason the roll is. */
export function boughtLine(who: string, attempt: Attempt): string {
  const line = `${who} spends XP to turn a die into a six on ${attempt.skill.name}.`;
  return isAdvancement(attempt.faces, attempt.bought)
    ? `${line} All sixes now: a new skill is on offer.`
    : line;
}

export function learnedLine(who: string, learned: Skill): string {
  return `${who} learns ${learned.name} (${learned.level}d6).`;
}

/**
 * A skill changed by hand, said aloud.
 *
 * A hand edit can make a character better at something with no dice
 * involved, and anyone who may edit the character can make one. The table
 * is told for the same reason it is told about a roll: so that nobody has
 * to take anybody's word for what the sheet says.
 */
export function byHandLine(who: string, what: string): string {
  return `${who}'s skills were changed by hand: ${what}.`;
}

const SAY = `
  mutation RollForShoesSay($input: SendChatMessageInput!) {
    sendChatMessage(input: $input) {
      id
    }
  }
`;

/**
 * Say a line in the world's chat.
 *
 * Throws on refusal. The caller decides what that means, and for a roll it
 * means the roll still stands — the dice came from the server and the sheet
 * has them; only the telling failed.
 */
export async function tellTheTable(
  worldId: string,
  body: string,
): Promise<void> {
  await postGraphQL(SAY, { input: { worldId, body } });
}

const attemptKey = (actorId: string): string => `rfs-attempt:${actorId}`;

/**
 * The attempt this browser tab last made for this character, if any.
 *
 * `sessionStorage`, not the server: an attempt is a moment at one seat, not a
 * fact about the character. Everything it caused that *is* a fact — the
 * experience, the new skill — is already written where facts go.
 */
export function recallAttempt(actorId: string): Attempt | null {
  try {
    const stored = sessionStorage.getItem(attemptKey(actorId));
    if (!stored) {
      return null;
    }
    const parsed = JSON.parse(stored) as Partial<Attempt> | null;
    if (
      !parsed ||
      !Array.isArray(parsed.faces) ||
      typeof parsed.total !== "number" ||
      typeof parsed.skill?.id !== "string"
    ) {
      return null;
    }
    return parsed as Attempt;
  } catch {
    // No storage, or something else's leavings under this key: no attempt.
    return null;
  }
}

export function rememberAttempt(
  actorId: string,
  attempt: Attempt | null,
): void {
  try {
    if (attempt) {
      sessionStorage.setItem(attemptKey(actorId), JSON.stringify(attempt));
    } else {
      sessionStorage.removeItem(attemptKey(actorId));
    }
  } catch {
    // A browser that refuses storage keeps the attempt for as long as the
    // sheet is open, which is what it did before this existed.
  }
}

/** The server's code for "an actor's sheet changed". */
const ACTOR_SHEET_CHANGED = 26;

function actorOf(event: WorldEventLike): string | null {
  const raw = event.token_event ?? event.tokenEvent;
  let payload: unknown = raw;
  if (typeof raw === "string") {
    try {
      payload = JSON.parse(raw);
    } catch {
      return null;
    }
  }
  if (payload && typeof payload === "object" && "actorId" in payload) {
    const id = (payload as { actorId: unknown }).actorId;
    return typeof id === "string" ? id : null;
  }
  return null;
}

/**
 * Call `onChanged` whenever the server reports this character's sheet
 * changed, by anyone. Returns the function that stops watching.
 *
 * The iterator is held and returned explicitly rather than looped with
 * `for await`, for the reason the Genie pack found: on a quiet world a
 * `for await` never reaches its next check, and the subscription outlives
 * the sheet.
 */
export function watchSheet(
  worldId: string,
  actorId: string,
  onChanged: () => void,
): () => void {
  const iterator = subscribeToWorldEvents(worldId)[Symbol.asyncIterator]();
  let stopped = false;

  void (async () => {
    try {
      while (!stopped) {
        const { value: event, done } = await iterator.next();
        if (done || stopped || !event) {
          break;
        }
        const code = event.event_code ?? event.eventCode;
        if (code === ACTOR_SHEET_CHANGED && actorOf(event) === actorId) {
          onChanged();
        }
      }
    } catch (error) {
      console.error("Roll for Shoes sheet sync error:", error);
    }
  })();

  return () => {
    stopped = true;
    void iterator.return?.();
  };
}
