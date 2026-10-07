/**
 * What the per-area handler files share: refusing in the server's words,
 * finding a scene and its levels, and the type a handler is.
 */
import { GraphQLError } from "graphql";
import type { DemoState, Row } from "../state";

export type Args = Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
export type Handler = (args: Args) => unknown;

/** A refusal, in the sentence the server answers with. */
export function refuse(message: string): never {
  throw new GraphQLError(message);
}

export function sceneOf(state: DemoState, sceneId: string): Row {
  return (
    state.scenes.find((s) => s.sceneId === sceneId) ?? refuse("Scene not found")
  );
}

/** A scene's levels, lowest first. */
export function levelsOf(state: DemoState, sceneId: string): Row[] {
  return state.levels
    .filter((level) => level.sceneId === sceneId)
    .sort((a, b) => (a.sortOrder as number) - (b.sortOrder as number));
}

/** The fields of `input` that were actually sent. */
export function given(input: Args): Row {
  return Object.fromEntries(
    Object.entries(input).filter(([, value]) => value !== undefined),
  );
}

/** `ambient_light`'s check: the light, trimmed and lower-cased, or `null`. */
export function ambientOf(light: unknown): string | null {
  if (typeof light !== "string") return null;
  const level = light.trim().toLowerCase();
  return ["bright", "dim", "dark"].includes(level) ? level : null;
}
