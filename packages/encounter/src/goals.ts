/**
 * Spec 091: the room's four goals.
 *
 * The page announces each goal as a DOM event, `GOAL_EVENT`, with
 * `detail: { goal }`. Nothing is sent anywhere.
 */

/** The goals, in the order the room plays them. */
export const GOALS = [
  "door-reached",
  "door-opened",
  "chest-found",
  "chest-opened",
] as const;

/** One goal id. The union is closed: no other id exists. */
export type GoalId = (typeof GOALS)[number];

/** The DOM event a met goal fires. */
export const GOAL_EVENT = "thunderforge:encounter-goal";

export interface GoalEventDetail {
  goal: GoalId;
}

/** Whether a string is one of the four goal ids. */
export function isGoalId(value: unknown): value is GoalId {
  return (
    typeof value === "string" && (GOALS as readonly string[]).includes(value)
  );
}
