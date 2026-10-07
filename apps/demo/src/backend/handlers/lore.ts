/**
 * The lore wiki (spec 012, spec 031's tree and tags), by the server's rules.
 *
 * Every member reads every entry. The Game Master is Owner of all of them;
 * the player holds whatever the entry's ownership block grants them, and
 * Viewer otherwise. Editing needs Editor, deleting needs Owner, and writing a
 * new entry or changing who may touch one is the Game Master's alone.
 *
 * An entry's revisions and explicit grants are kept on the entry itself
 * (`revisions`, `permissions`): the world is one object, and a key the schema
 * does not know is never sent.
 */
import { GraphQLError } from "graphql";
import { DEMO_PLAYER, DEMO_USER } from "../../seed/world";
import { viewerIsGm, viewerUser, visibleActors } from "../actors";
import { now } from "../events";
import { entriesLinkingTo, renderLore, slugFor } from "../lore";
import { demoState, markChanged, type DemoState, type Row } from "../state";

type Args = Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
type Handler = (args: Args) => unknown;

const LEVELS = ["VIEWER", "EDITOR", "OWNER"] as const;
type Level = (typeof LEVELS)[number];

const MAX_TAG = 64;
const LINK_TARGET_LIMIT = 20;

function forbidden(message: string, code?: string): never {
  throw new GraphQLError(
    message,
    code ? { extensions: { code } } : undefined,
  );
}

function grants(entry: Row): Row[] {
  if (!Array.isArray(entry.permissions)) entry.permissions = [];
  return entry.permissions as Row[];
}

function revisions(entry: Row): Row[] {
  if (!Array.isArray(entry.revisions)) entry.revisions = [];
  return entry.revisions as Row[];
}

/** The viewer's effective level on an entry (FR-003). */
export function loreLevel(state: DemoState, entry: Row): Level {
  if (viewerIsGm(state)) return "OWNER";
  const own = grants(entry).find((g) => g.userId === viewerUser(state).id);
  return (own?.level as Level | undefined) ?? "VIEWER";
}

function findEntry(state: DemoState, id: string, missing = "Lore entry not found"): Row {
  return state.lore.find((e) => e.id === id) ?? forbidden(missing);
}

function need(state: DemoState, entry: Row, level: Level, code?: string): void {
  if (LEVELS.indexOf(loreLevel(state, entry)) < LEVELS.indexOf(level)) {
    forbidden(
      "You do not have sufficient permission on this lore entry",
      code,
    );
  }
}

function needDm(state: DemoState, what: string): void {
  if (!viewerIsGm(state)) {
    forbidden(`Only the DM (Owner or GM) may ${what}`);
  }
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

function revisionView(state: DemoState, revision: Row): Row {
  const gm = viewerIsGm(state);
  return {
    ...revision,
    renderedHtml: () =>
      renderLore(state, String(revision.contentMarkdown ?? ""), gm),
  };
}

function appendRevision(
  state: DemoState,
  entry: Row,
  content: string,
  restoredFrom: string | null = null,
): void {
  const revision: Row = {
    id: crypto.randomUUID(),
    loreEntryId: entry.id,
    contentMarkdown: content,
    authorId: viewerUser(state).id,
    restoredFromRevisionId: restoredFrom,
    createdAt: now(),
  };
  revisions(entry).push(revision);
  entry.content = content;
  entry.currentRevisionId = revision.id;
}

/** The server's tag: one space between words, lower case. */
function normaliseTag(tag: string): string {
  const clean = tag.split(/\s+/).filter(Boolean).join(" ").toLowerCase();
  if (clean.length === 0 || [...clean].length > MAX_TAG) {
    forbidden("a tag needs at least one character and at most 64");
  }
  return clean;
}

/** Shared by the seed, which writes its entries the way a GM would. */
export function newLoreEntry(
  state: DemoState,
  title: string,
  content: string,
  createdBy: string = DEMO_USER.id,
): Row {
  const at = now();
  const entry: Row = {
    id: crypto.randomUUID(),
    worldId: state.world.id,
    title,
    slug: slugFor(state, title),
    content: "",
    currentRevisionId: null,
    createdBy,
    createdAt: at,
    updatedAt: at,
    parentId: null,
    tags: [],
    revisions: [],
    permissions: [],
  };
  if (content !== "") appendRevision(state, entry, content);
  state.lore.push(entry);
  return entry;
}

function members(): string[] {
  return [DEMO_USER.id, DEMO_PLAYER.id];
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
  loreEntryRevisions: ({ loreEntryId }) => {
    const state = demoState();
    const entry = findEntry(state, loreEntryId);
    return [...revisions(entry)]
      .reverse()
      .map((revision) => revisionView(state, revision));
  },
  loreLinkTargets: ({ prefix }) => {
    const state = demoState();
    const wanted = String(prefix ?? "").trim().toLowerCase();
    const starts = (text: unknown) =>
      String(text).toLowerCase().startsWith(wanted);
    return [
      ...state.lore
        .filter((e) => starts(e.title))
        .map((e) => ({ id: e.id, title: e.title, kind: "LORE_ENTRY" })),
      ...visibleActors(state)
        .filter((a) => starts(a.label))
        .map((a) => ({ id: a.id, title: a.label, kind: "ACTOR" })),
    ]
      .sort((a, b) => String(a.title).localeCompare(String(b.title)))
      .slice(0, LINK_TARGET_LIMIT);
  },
  loreEntryPermissions: ({ loreEntryId }) => {
    const state = demoState();
    needDm(state, "view or change a lore entry's ownership block");
    return grants(findEntry(state, loreEntryId));
  },
};

export const loreMutations: Record<string, Handler> = {
  createLoreEntry: ({ input }) => {
    const state = demoState();
    needDm(state, "create lore entries");
    const title = String(input.title ?? "").trim();
    if (title === "") forbidden("A lore entry needs a title");
    const entry = newLoreEntry(
      state,
      title,
      (input.content as string | null | undefined) ?? "",
      viewerUser(state).id,
    );
    markChanged();
    return loreView(state, entry);
  },

  updateLoreEntry: ({ input }) => {
    const state = demoState();
    const entry = findEntry(state, input.loreEntryId);
    need(state, entry, "EDITOR", "FORBIDDEN");
    if (input.content != null) {
      if (
        (input.expectedCurrentRevisionId ?? null) !==
        (entry.currentRevisionId ?? null)
      ) {
        forbidden(
          "This entry was changed since you opened it. Reload it to see the newer version.",
          "CONFLICT",
        );
      }
      if (input.content !== entry.content) {
        appendRevision(state, entry, input.content);
      }
    }
    if (input.title != null && input.title !== entry.title) {
      entry.title = input.title;
      entry.slug = slugFor(state, input.title, entry.id as string);
    }
    entry.updatedAt = now();
    markChanged();
    return loreView(state, entry);
  },

  deleteLoreEntry: ({ loreEntryId }) => {
    const state = demoState();
    const entry = findEntry(state, loreEntryId);
    need(state, entry, "OWNER", "FORBIDDEN");
    for (const child of state.lore) {
      if (child.parentId === entry.id) child.parentId = entry.parentId ?? null;
    }
    state.lore.splice(state.lore.indexOf(entry), 1);
    markChanged();
    return true;
  },

  restoreLoreRevision: ({ revisionId }) => {
    const state = demoState();
    const entry =
      state.lore.find((e) => revisions(e).some((r) => r.id === revisionId)) ??
      forbidden("Lore revision not found");
    need(state, entry, "EDITOR", "FORBIDDEN");
    const revision = revisions(entry).find((r) => r.id === revisionId)!;
    appendRevision(
      state,
      entry,
      String(revision.contentMarkdown),
      revision.id as string,
    );
    entry.updatedAt = now();
    markChanged();
    return loreView(state, entry);
  },

  moveLoreEntry: ({ input }) => {
    const state = demoState();
    const entry = findEntry(state, input.loreEntryId, "lore entry not found");
    need(state, entry, "EDITOR");
    const parentId = (input.parentId as string | null | undefined) ?? null;
    if (parentId !== null) {
      const parent = state.lore.find((e) => e.id === parentId);
      if (!parent) {
        forbidden(
          "a lore entry can only sit under another entry in the same world",
        );
      }
      // Walk up from the new parent: meeting the entry means a loop.
      for (let at: Row | undefined = parent; at; ) {
        if (at.id === entry.id) {
          forbidden("that move would put the entry inside itself", "LORE_CYCLE");
        }
        at = at.parentId
          ? state.lore.find((e) => e.id === at!.parentId)
          : undefined;
      }
    }
    entry.parentId = parentId;
    entry.updatedAt = now();
    markChanged();
    return loreView(state, entry);
  },

  addLoreTag: ({ input }) => {
    const state = demoState();
    const entry = findEntry(state, input.loreEntryId);
    need(state, entry, "EDITOR");
    const tag = normaliseTag(String(input.tag));
    const tags = new Set((entry.tags as string[] | undefined) ?? []);
    tags.add(tag);
    entry.tags = [...tags].sort();
    markChanged();
    return entry.tags;
  },

  removeLoreTag: ({ input }) => {
    const state = demoState();
    const entry = findEntry(state, input.loreEntryId);
    need(state, entry, "EDITOR");
    const tag = normaliseTag(String(input.tag));
    entry.tags = ((entry.tags as string[] | undefined) ?? [])
      .filter((t) => t !== tag)
      .sort();
    markChanged();
    return entry.tags;
  },

  setLorePermission: ({ input }) => {
    const state = demoState();
    needDm(state, "view or change a lore entry's ownership block");
    const entry = findEntry(state, input.loreEntryId);
    if (!members().includes(input.userId)) {
      forbidden("That user is not a member of this world");
    }
    const rows = grants(entry);
    let row = rows.find((g) => g.userId === input.userId);
    if (!row) {
      row = { loreEntryId: entry.id, userId: input.userId };
      rows.push(row);
    }
    Object.assign(row, { level: input.level, updatedAt: now() });
    markChanged();
    return row;
  },

  removeLorePermission: ({ loreEntryId, userId }) => {
    const state = demoState();
    needDm(state, "view or change a lore entry's ownership block");
    const rows = grants(findEntry(state, loreEntryId));
    const index = rows.findIndex((g) => g.userId === userId);
    if (index < 0) return false;
    rows.splice(index, 1);
    markChanged();
    return true;
  },
};
