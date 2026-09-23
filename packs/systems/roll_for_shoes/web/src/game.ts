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
  if (opposition === null) {
    return "unjudged";
  }
  return total > opposition ? "success" : "failure";
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
): Skill[] {
  const stored = traitData?.["skills"];
  if (Array.isArray(stored) && stored.length > 0) {
    return stored as Skill[];
  }
  return [
    {
      id: "starting-skill",
      name: STARTING_SKILL.name,
      level: STARTING_SKILL.level,
      parentId: null,
    },
  ];
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
