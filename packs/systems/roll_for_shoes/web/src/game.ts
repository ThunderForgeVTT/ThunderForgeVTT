/**
 * The rules of Roll for Shoes, as functions over plain data.
 *
 * No React, no network, no host. The sheet draws what is decided here, and
 * these decisions are tested in `game.test.ts` without a browser or a server —
 * which is the whole reason the game lives in its own module rather than
 * inside the component.
 *
 * The game itself is public domain (CC0 1.0), by Ben Wray — rollforshoes.com.
 */

/** The pack's id, which is also its directory name and its manifest `id`. */
export const SYSTEM_ID = "roll_for_shoes";

export interface Skill {
  /** Opaque, generated here, shown to nobody. Names are free text and repeat. */
  id: string;
  name: string;
  /** How many d6 the skill rolls. At least 1, with no upper bound. */
  level: number;
  /** The skill this one grew out of; `null` only for the first one. */
  parentId: string | null;
}

/**
 * The skill every character starts with.
 *
 * Stated here *and* in `system.json`, because the host surface gives a sheet
 * no way to read its own manifest. The pack's server crate reads the manifest
 * and asserts the two agree, which is the best this seam allows (research D6).
 */
export const STARTING_SKILL = { name: "Do Anything", level: 1 } as const;

export type Verdict = "success" | "failure" | "unjudged";

/** A rolled die, as the server reports it. */
export interface RolledDie {
  face: number;
}

/**
 * What the roll did.
 *
 * Beating the opposition means beating it — a tie is a failure, and therefore
 * earns its experience like any other failure. With no opposition named there
 * is nothing to judge against, and the roll is neither.
 */
export function verdict(total: number, opposition: number | null): Verdict {
  return judge(total, opposition, DEFAULT_SETTINGS.tieSucceeds);
}

/**
 * The comparison itself, with the tie rule as an argument.
 *
 * Both `verdict` and `resolve` go through here, so the core game and a world
 * that has changed the tie rule cannot drift apart into two comparisons: there
 * is one, and `tieSucceeds` is the only thing that varies.
 */
function judge(
  total: number,
  opposition: number | null,
  tieSucceeds: boolean,
): Verdict {
  if (opposition === null) {
    return "unjudged";
  }
  return (tieSucceeds ? total >= opposition : total > opposition)
    ? "success"
    : "failure";
}

/** Failure is the only thing that pays. */
export function xpAward(result: Verdict): number {
  return result === "failure" ? 1 : 0;
}

/** How many of this roll's dice count as sixes, rolled or bought. */
export function sixesShown(faces: number[], bought: number): number {
  return faces.filter((face) => face === 6).length + bought;
}

/**
 * Whether this roll earns a new skill.
 *
 * Read from the individual dice, never from the total: a sum cannot tell you
 * whether every die showed a six.
 */
export function isAdvancement(faces: number[], bought: number): boolean {
  return faces.length > 0 && sixesShown(faces, bought) >= faces.length;
}

/** Dice in this roll that are not sixes yet, and so could still be bought. */
export function remainingNonSixes(faces: number[], bought: number): number {
  return Math.max(0, faces.length - sixesShown(faces, bought));
}

export interface SpendRefusal {
  allowed: false;
  reason: string;
}

export interface SpendAllowed {
  allowed: true;
  /** The balance after this spend. */
  balance: number;
  /** How many dice have been bought after this spend. */
  bought: number;
}

/**
 * Spend one experience to turn one die into a six.
 *
 * It buys the *advancement* and nothing else: the verdict, the total and the
 * experience this roll already awarded are all untouched by it. Experience a
 * failure just paid may be spent on that same failure.
 */
export function spendXp(
  balance: number,
  faces: number[],
  bought: number,
): SpendAllowed | SpendRefusal {
  if (balance < 1) {
    return { allowed: false, reason: "Not enough XP — the balance is 0." };
  }
  if (remainingNonSixes(faces, bought) < 1) {
    return { allowed: false, reason: "Every die already shows a six." };
  }
  return { allowed: true, balance: balance - 1, bought: bought + 1 };
}

export interface GrantRefusal {
  granted: false;
  reason: string;
}

export interface GrantMade {
  granted: true;
  skill: Skill;
}

/**
 * The new skill an all-sixes roll earns: one level above the skill rolled, and
 * recorded as having grown out of it.
 *
 * Any non-empty name is accepted. Whether it is *more specific* than the skill
 * rolled, and whether it is relevant to what was attempted, is settled by the
 * people at the table — a product that arbitrated it would be playing their
 * game for them.
 */
export function grantSkill(
  parent: Skill,
  name: string,
  id: string,
): GrantMade | GrantRefusal {
  const trimmed = name.trim();
  if (trimmed.length === 0) {
    return { granted: false, reason: "Give the new skill a name." };
  }
  return {
    granted: true,
    skill: { id, name: trimmed, level: parent.level + 1, parentId: parent.id },
  };
}

/** A fresh opaque id for a skill. */
export function newSkillId(): string {
  const random = globalThis.crypto?.randomUUID?.();
  return (
    random ??
    `s-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
  );
}

/**
 * The skills a character holds.
 *
 * A character who has never been saved has nothing stored, and that reads as
 * the starting skill rather than as an empty sheet: making a character is
 * typing a name, and there is nothing else to fill in.
 */
export function skillsOf(
  traitData: Record<string, unknown> | null | undefined,
  settings?: WorldSettings,
): Skill[] {
  const stored = traitData?.["skills"];
  if (Array.isArray(stored) && stored.length > 0) {
    // **A character who has stored skills never reads the world's starting
    // skills** (FR-039). This is the whole of "changing the setting does not
    // alter anyone who already exists": the settings are consulted only on the
    // branch a stored character never reaches. Nothing has to compare a
    // creation date, and nothing has to be migrated when a Game Master
    // changes their mind.
    return stored as Skill[];
  }
  return startingSkills(settings);
}

/**
 * What a brand-new character is given.
 *
 * An empty list in the settings means the core rule, not a character with no
 * skills — the two are the same stored value and different games, and this is
 * the one place in the web module that tells them apart. (`lib.rs` makes the
 * same distinction for the manifest, for the same reason.)
 *
 * Each of these is a root: they are what the character was handed, so none
 * descends from another. That is why the server's T10 counts roots rather than
 * insisting on one, and why T11 no longer insists on level 1.
 */
export function startingSkills(settings?: WorldSettings): Skill[] {
  const declared = settings?.startingSkills ?? [];
  if (declared.length === 0) {
    return [
      {
        id: "starting-skill",
        name: STARTING_SKILL.name,
        level: STARTING_SKILL.level,
        parentId: null,
      },
    ];
  }
  return declared.map((skill, index) => ({
    // The first keeps the id the core skill has always had, so a world that
    // declares one starting skill is the core case with a different name
    // rather than a different shape.
    id: index === 0 ? "starting-skill" : `starting-skill-${index}`,
    name: skill.name,
    level: Math.max(1, Math.trunc(skill.level)),
    parentId: null,
  }));
}

/** The experience a character holds; nothing stored means none earned. */
export function xpOf(
  resourceData: Record<string, unknown> | null | undefined,
): number {
  const stored = resourceData?.["xp"];
  return typeof stored === "number" && Number.isFinite(stored)
    ? Math.max(0, Math.trunc(stored))
    : 0;
}

export interface LineageRow {
  skill: Skill;
  /** How far the skill sits from the starting one; the first is at 0. */
  depth: number;
}

/**
 * The lineage in reading order: the starting skill, then each skill beneath
 * the one it grew out of.
 *
 * The order is the character's history, which is why the sheet shows it nested
 * rather than as a flat list sorted by level.
 */
export function lineageOrder(skills: Skill[]): LineageRow[] {
  const childrenOf = new Map<string | null, Skill[]>();
  for (const skill of skills) {
    const key = skill.parentId ?? null;
    const siblings = childrenOf.get(key) ?? [];
    siblings.push(skill);
    childrenOf.set(key, siblings);
  }

  const rows: LineageRow[] = [];
  const seen = new Set<string>();

  const walk = (parentId: string | null, depth: number): void => {
    for (const skill of childrenOf.get(parentId) ?? []) {
      if (seen.has(skill.id)) {
        continue;
      }
      seen.add(skill.id);
      rows.push({ skill, depth });
      walk(skill.id, depth + 1);
    }
  };

  walk(null, 0);

  // Anything the walk could not reach — a parent that is not in the list —
  // is still the player's, and is shown rather than silently dropped.
  for (const skill of skills) {
    if (!seen.has(skill.id)) {
      rows.push({ skill, depth: 0 });
    }
  }

  return rows;
}

/* ------------------------------------------------------------------ *
 * The Extras (spec 062)
 *
 * Roll for Shoes publishes six core rules and a handful of "Extras" it
 * explicitly leaves to the table. Everything below is one of those, and
 * every one of them is off unless a world turns it on. `DEFAULT_SETTINGS`
 * is the core game, and passing it to anything here must produce exactly
 * what spec 061 produced.
 * ------------------------------------------------------------------ */

/**
 * A world's answers to the optional rules.
 *
 * Read from the pack's own table — `world: WorldRecord` does not carry these,
 * and no shared web file knows this system exists.
 */
export interface WorldSettings {
  difficultyMode: DifficultyMode;
  tieSucceeds: boolean;
  statusesEnabled: boolean;
  skillSlotsEnabled: boolean;
  /** Empty means the core default, `Do Anything 1` — never "no skills". */
  startingSkills: { name: string; level: number }[];
}

/** How the Game Master states the opposition. */
export type DifficultyMode = "free" | "rolled" | "target";

/**
 * The core game, exactly as spec 061 shipped it.
 *
 * A world that has never opened the settings panel has no stored row, and
 * reads as this. So does a world that predates the table entirely.
 */
export const DEFAULT_SETTINGS: WorldSettings = {
  difficultyMode: "free",
  tieSucceeds: false,
  statusesEnabled: false,
  skillSlotsEnabled: false,
  startingSkills: [],
};

/** How hard the Game Master calls it. A per-roll choice, not a setting. */
export type Band = "easy" | "moderate" | "hard" | "veryHard";

/** The bands in the order a Game Master reads them. */
export const BANDS: readonly Band[] = ["easy", "moderate", "hard", "veryHard"];

/**
 * How many d6 the Game Master rolls for each band, and what each band's
 * static target is.
 *
 * Two lookup tables rather than one formula. The dice happen to be 1–4 and the
 * targets happen to be their triples, so `level * 3` would reproduce both —
 * and would also answer for a fifth band, which does not exist. The tables
 * cannot answer a question the game does not ask.
 */
export const BAND_DICE: Record<Band, number> = {
  easy: 1,
  moderate: 2,
  hard: 3,
  veryHard: 4,
};

export const BAND_TARGET: Record<Band, number> = {
  easy: 3,
  moderate: 6,
  hard: 9,
  veryHard: 12,
};

/** What to call each band on screen. */
export const BAND_LABEL: Record<Band, string> = {
  easy: "Easy",
  moderate: "Moderate",
  hard: "Hard",
  veryHard: "Very Hard",
};

/**
 * A named condition a character carries, worth a plus or a minus.
 *
 * The names are the table's own words. This system ships no list of them and
 * judges none, exactly as it judges no skill name.
 */
export interface Status {
  /** Opaque, generated here, shown to nobody. Names are free text and repeat. */
  id: string;
  name: string;
  /** Applied flat to the roll's total. May be negative, and they sum. */
  modifier: number;
}

/** Everything one roll needs judging. */
export interface RollInput {
  /** The character's dice, as the server rolled them. */
  faces: number[];
  /** The statuses the character carries; empty when the world has none. */
  statuses: Status[];
  /** What the roll has to beat, or `null` when the GM has named nothing. */
  opposition: number | null;
  /** This world's tie rule. */
  tieSucceeds: boolean;
}

/** What one roll came to. Derived on every read, stored nowhere. */
export interface RollOutcome {
  /** The dice as rolled, with no adjustment. */
  sum: number;
  /** The statuses, summed. Zero when the world has none. */
  modifier: number;
  /** `sum + modifier` — what is compared against the opposition. */
  total: number;
  verdict: Verdict;
  xpAwarded: number;
}

/**
 * Judge one roll.
 *
 * **The order is the decision**: dice → sum → statuses → opposition →
 * comparison under the tie rule. Three of the Extras touch a single roll, and
 * left implicit, every pair of them becomes a question somebody has to
 * re-derive — does a status change the dice? does the tie rule apply before or
 * after it? Fixing the order here answers all of them once.
 *
 * `resolve` deliberately cannot see an advancement. It reads `faces` only to
 * sum them, and `isAdvancement` reads the same array for sixes; neither can
 * reach the other's answer, which is what makes "a status can neither create
 * nor destroy an advancement" structural rather than remembered.
 */
export function resolve(input: RollInput): RollOutcome {
  const sum = input.faces.reduce((running, face) => running + face, 0);
  const modifier = statusModifier(input.statuses);
  const total = sum + modifier;
  const result = judge(total, input.opposition, input.tieSucceeds);
  return {
    sum,
    modifier,
    total,
    verdict: result,
    // Not a second rule suppressing XP on a tie: a tie under `tieSucceeds` is
    // a success, and successes have never paid.
    xpAwarded: xpAward(result),
  };
}

/** What the statuses come to together. Empty is zero, and so is a set that cancels. */
export function statusModifier(statuses: Status[]): number {
  return statuses.reduce((running, status) => running + status.modifier, 0);
}

/**
 * The statuses a character carries; nothing stored means none.
 *
 * Deliberately unlike {@link skillsOf}, which reads an empty store as the
 * starting skill. A character with no skills is impossible — the game gives
 * everyone one — but a character with no statuses is the ordinary case, and is
 * every character in every world that has not turned statuses on. So absent
 * reads as none here, and there is no read-time default to undo.
 */
export function statusesOf(
  traitData: Record<string, unknown> | null | undefined,
): Status[] {
  const stored = traitData?.["statuses"];
  if (!Array.isArray(stored)) {
    return [];
  }
  return stored.filter(
    (entry): entry is Status =>
      typeof entry === "object" &&
      entry !== null &&
      typeof (entry as Status).id === "string" &&
      typeof (entry as Status).name === "string" &&
      Number.isInteger((entry as Status).modifier),
  );
}

export interface StatusRefusal {
  added: false;
  reason: string;
}

export interface StatusAdded {
  added: true;
  statuses: Status[];
}

/**
 * Write a status onto a character.
 *
 * The name is whatever the table wrote and the number is whatever the table
 * decided it is worth; neither is judged, exactly as a skill name is not. The
 * one refusal is a nameless status, because a row nobody can read is a row
 * nobody can decide to remove.
 *
 * Two statuses may share a name — saying "Wounded" twice is a table saying it
 * twice, and both apply. Only the id, which is shown to nobody, is unique.
 */
export function addStatus(
  statuses: Status[],
  name: string,
  modifier: number,
  id: string,
): StatusAdded | StatusRefusal {
  const trimmed = name.trim();
  if (trimmed.length === 0) {
    return { added: false, reason: "Give the status a name." };
  }
  if (!Number.isInteger(modifier)) {
    return { added: false, reason: "The modifier must be a whole number." };
  }
  return { added: true, statuses: [...statuses, { id, name: trimmed, modifier }] };
}

/** Take a status off. Removing one that is not there is not an error. */
export function removeStatus(statuses: Status[], id: string): Status[] {
  return statuses.filter((status) => status.id !== id);
}

/** A fresh opaque id for a status. Shown to nobody; names are the readable part. */
export function newStatusId(): string {
  const random = globalThis.crypto?.randomUUID?.();
  return (
    random ??
    `st-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
  );
}

// ---------------------------------------------------------------------------
// Skill slots — how many skills may sit at a level (US5)
// ---------------------------------------------------------------------------

/**
 * How many skills a character may hold at each level.
 *
 * Levels not named here are **uncapped**, and that is levels 1 and 5 upwards.
 * Level 1 is uncapped because the starting skill lives there and a world may
 * declare several; level 5 and up because a character who has climbed that far
 * has earned the room, which is the shape the rule has on the site.
 */
export const SLOT_CAPS: Readonly<Record<number, number>> = Object.freeze({
  2: 4,
  3: 3,
  4: 2,
});

/**
 * The cap at a level, or `null` where there is none.
 *
 * `null` rather than `Infinity` or a large number on purpose: "uncapped" is a
 * different thing from "a big allowance", and a caller that treats it as a
 * number will eventually compare against it and be wrong.
 */
export function capAtLevel(level: number): number | null {
  return SLOT_CAPS[level] ?? null;
}

/** What buying one more slot at this level costs: twice the level. */
export function slotCost(level: number): number {
  return level * 2;
}

/** How many skills the character already holds at a level. */
export function slotsUsed(skills: Skill[], level: number): number {
  return skills.filter((skill) => skill.level === level).length;
}

/** Slots bought with experience at a level; nothing stored means none. */
export function boughtSlotsAt(
  resourceData: Record<string, unknown> | null | undefined,
  level: number,
): number {
  const stored = resourceData?.["boughtSlots"];
  if (typeof stored !== "object" || stored === null) {
    return 0;
  }
  const at = (stored as Record<string, unknown>)[String(level)];
  return typeof at === "number" && Number.isInteger(at) && at > 0 ? at : 0;
}

/**
 * Room left at a level: `null` where there is no cap, otherwise how many more
 * skills will fit.
 *
 * It may come back **negative**, and that is not a bug. A character who
 * already holds five skills at level 2 was made in a world that had this
 * setting off, or before it existed; turning it on must not make them
 * unstorable or un-openable (FR-036). They simply have no room until they buy
 * some, which is what a negative number says.
 */
export function slotsAvailable(
  skills: Skill[],
  level: number,
  bought: number,
): number | null {
  const cap = capAtLevel(level);
  if (cap === null) {
    return null;
  }
  return cap + bought - slotsUsed(skills, level);
}

/** Whether a new skill at this level would fit. Uncapped levels always fit. */
export function hasRoomAt(
  skills: Skill[],
  level: number,
  bought: number,
): boolean {
  const left = slotsAvailable(skills, level, bought);
  return left === null || left > 0;
}

export interface SlotRefusal {
  bought: false;
  reason: string;
}

export interface SlotBought {
  bought: true;
  /** The balance after this purchase. */
  balance: number;
  /** How many slots have been bought at this level after this purchase. */
  slots: number;
}

/**
 * Buy one more slot at a level, for twice the level.
 *
 * This is the only thing in the game that spends experience on something other
 * than turning a die into a six, so it debits the same balance `spendXp` does
 * and refuses the same way — the ledger has to stay honest across both, and it
 * does so by there only being one balance (FR-035).
 */
export function buySlot(
  balance: number,
  level: number,
  boughtSoFar: number,
): SlotBought | SlotRefusal {
  if (capAtLevel(level) === null) {
    return {
      bought: false,
      reason: `Level ${level} has no limit — there is nothing to buy.`,
    };
  }
  const cost = slotCost(level);
  if (balance < cost) {
    return {
      bought: false,
      reason: `Not enough XP — a level ${level} slot costs ${cost}, and the balance is ${balance}.`,
    };
  }
  return { bought: true, balance: balance - cost, slots: boughtSoFar + 1 };
}

/** The stored map with one level's bought count replaced. */
export function withBoughtSlot(
  resourceData: Record<string, unknown> | null | undefined,
  level: number,
  slots: number,
): Record<string, number> {
  const stored = resourceData?.["boughtSlots"];
  const existing =
    typeof stored === "object" && stored !== null
      ? (stored as Record<string, number>)
      : {};
  return { ...existing, [String(level)]: slots };
}
