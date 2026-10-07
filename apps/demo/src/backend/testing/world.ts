/**
 * A fresh demo world for a handler test, asked through the real schema.
 *
 * Questions go through `runOperation`, the same path the page's requests
 * take, so a test proves the answer is one the server's schema accepts and
 * not only that a handler returned something.
 */
import "./window";
import { refused } from "../notInDemo";
import { runOperation } from "../execute";
import { subscribeToEvents, releaseEvents, type WorldEvent } from "../events";
import { demoState, forgetSavedWorld, loadState } from "../state";
import { DEMO_SCENES, type MapListing, type Viewer } from "../../seed/world";

const maps: MapListing[] = DEMO_SCENES.map((scene) => ({
  name: scene.map,
  width: 4000,
  height: 3000,
  gridSize: 100,
  ambientLight: "#ffffff",
  hasPreview: false,
  byteSize: 1,
  walls: [],
  lights: [],
}));

const fetchStatic = (async () =>
  new Response(JSON.stringify(maps))) as unknown as typeof fetch;

/** The seed, as a first visit gets it, seen by `viewer`. */
export async function freshWorld(viewer: Viewer = "gm") {
  forgetSavedWorld();
  refused.length = 0;
  const state = await loadState(fetchStatic, "/demo/");
  state.viewer = viewer;
  return state;
}

export function viewAs(viewer: Viewer): void {
  demoState().viewer = viewer;
}

export interface Answer<T> {
  data?: T;
  errors?: Array<{ message: string; extensions?: Record<string, unknown> }>;
}

export async function ask<T = Record<string, any>>( // eslint-disable-line @typescript-eslint/no-explicit-any
  query: string,
  variables: Record<string, unknown> = {},
): Promise<Answer<T>> {
  const result = await runOperation({ query, variables });
  return JSON.parse(JSON.stringify(result)) as Answer<T>;
}

/** Asks, and fails the test with the server's words if it was refused. */
export async function must<T = Record<string, any>>( // eslint-disable-line @typescript-eslint/no-explicit-any
  query: string,
  variables: Record<string, unknown> = {},
): Promise<T> {
  const answer = await ask<T>(query, variables);
  if (answer.errors?.length) {
    throw new Error(answer.errors.map((e) => e.message).join("; "));
  }
  return answer.data as T;
}

/** The answer's data, or the test fails with the errors. */
export const data = must;

/** The first refusal's words. */
export async function refusal(
  query: string,
  variables: Record<string, unknown> = {},
): Promise<string | undefined> {
  return (await ask(query, variables)).errors?.[0]?.message;
}

/** Every event released from now on, as a subscriber receives it. */
export function heard(): WorldEvent[] {
  const events: WorldEvent[] = [];
  subscribeToEvents((event) => events.push(event));
  return events;
}

export { refused, releaseEvents };
