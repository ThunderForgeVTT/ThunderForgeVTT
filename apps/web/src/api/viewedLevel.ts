/**
 * Which level of each scene this browser is showing.
 *
 * A scene owns ordered levels, and everything on the board — tokens, walls,
 * lights, shapes, interactives — stands on exactly one of them. The engine is
 * shown one level at a time, so every per-scene read has to name that level
 * and everything a Game Master creates has to land on it.
 *
 * # Why this is a registry and not an argument
 *
 * About twenty call sites read or create per scene, and most of them are
 * panels that know a scene id and nothing else: the token panel, the combat
 * tracker, the context menu, the shape tool. Threading a level id through
 * every one of them would be twenty places to forget it, and the one that
 * forgot would not fail — the server answers a read with no level for the
 * *entry* level, so a Game Master on the first floor would silently be listing
 * and placing on the ground floor. One place that knows, consulted by the api
 * functions themselves, cannot be forgotten.
 *
 * The world page's level state (`useSceneLevels`) is the only writer. An api
 * function still takes an explicit level where a caller really means another
 * one — the travel picker lists every level's interactives — and the explicit
 * argument wins.
 *
 * Module state rather than React state because the readers are plain
 * functions called from sync code that has no component to read context from.
 */
const viewed = new Map<string, string>();

/** Say which level of a scene is on screen. `null` forgets the scene. */
export function setViewedLevel(sceneId: string, levelId: string | null): void {
  if (levelId === null) {
    viewed.delete(sceneId);
  } else {
    viewed.set(sceneId, levelId);
  }
}

/**
 * The level of a scene on screen, or `undefined` when nothing has said.
 *
 * `undefined` rather than `null` on purpose: it is passed straight through as
 * a GraphQL variable, where an absent level means "the one I would open on" —
 * exactly what a caller that has not been told should ask for.
 */
export function viewedLevelId(sceneId: string): string | undefined {
  return viewed.get(sceneId);
}

/**
 * A create input, placed on the level being viewed unless it names its own.
 *
 * The server puts a thing with no level on the entry level, which is right
 * for a script and wrong for a Game Master looking at the first floor.
 */
export function onViewedLevel<T extends { sceneId: string; levelId?: string }>(
  input: T,
): T {
  if (input.levelId) {
    return input;
  }
  const levelId = viewedLevelId(input.sceneId);
  return levelId ? { ...input, levelId } : input;
}
