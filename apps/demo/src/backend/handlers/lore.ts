/**
 * The lore wiki (spec 012, spec 031's tree and tags), by the server's rules.
 *
 * Every member reads every entry. The Game Master is Owner of all of them;
 * the player holds whatever the entry's ownership block grants them, and
 * Viewer otherwise.
 *
 * An entry's explicit grants are kept on the entry itself (`permissions`):
 * the world is one object, and a key the schema does not know is never sent.
 */
import { viewerIsGm, viewerUser } from "../actors";
import { entriesLinkingTo, renderLore } from "../lore";
import { demoState, type DemoState, type Row } from "../state";

type Args = Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
type Handler = (args: Args) => unknown;

type Level = "VIEWER" | "EDITOR" | "OWNER";

function grants(entry: Row): Row[] {
  if (!Array.isArray(entry.permissions)) entry.permissions = [];
  return entry.permissions as Row[];
}

/** The viewer's effective level on an entry (FR-003). */
export function loreLevel(state: DemoState, entry: Row): Level {
  if (viewerIsGm(state)) return "OWNER";
  const own = grants(entry).find((g) => g.userId === viewerUser(state).id);
  return (own?.level as Level | undefined) ?? "VIEWER";
}

/**
 * The entry as this viewer is sent it. `renderedHtml` and `linkedFrom` are
 * worked out on read, as the server does, and only when the query asks.
 */
export function loreView(state: DemoState, entry: Row): Row {
  const gm = viewerIsGm(state);
  return {
    id: entry.id,
    worldId: entry.worldId,
    title: entry.title,
    slug: entry.slug,
    content: entry.content,
    currentRevisionId: entry.currentRevisionId ?? null,
    createdBy: entry.createdBy,
    createdAt: entry.createdAt,
    updatedAt: entry.updatedAt,
    parentId: entry.parentId ?? null,
    moderated: false,
    moderationCaseId: null,
    tags: [...((entry.tags as string[] | undefined) ?? [])].sort(),
    myPermissionLevel: loreLevel(state, entry),
    renderedHtml: () => renderLore(state, String(entry.content ?? ""), gm),
    linkedFrom: () =>
      entriesLinkingTo(state, "LORE_ENTRY", entry.id as string, gm).map((e) =>
        loreView(state, e),
      ),
  };
}

/** Every entry with a resolved link to an actor, as `loreLinkedFrom`. */
export function loreLinkingToActor(state: DemoState, actorId: string): Row[] {
  return entriesLinkingTo(state, "ACTOR", actorId, viewerIsGm(state)).map(
    (entry) => loreView(state, entry),
  );
}

export const loreQueries: Record<string, Handler> = {
  worldLoreEntries: () => {
    const state = demoState();
    return [...state.lore]
      .sort((a, b) =>
        String(a.title).localeCompare(String(b.title), undefined, {
          sensitivity: "base",
        }),
      )
      .map((entry) => loreView(state, entry));
  },
  loreEntry: ({ slug }) => {
    const state = demoState();
    const entry = state.lore.find((e) => e.slug === slug);
    return entry ? loreView(state, entry) : null;
  },
};
