/**
 * Making content from the actor's screen, without leaving it (spec 031
 * FR-039).
 *
 * # What the playtest found
 *
 * A Game Master building an NPC needed an item to give it, and the only way to
 * make one was the compendium — a different page, reached by leaving the one
 * they were halfway through filling in. They came back to a form they had to
 * find their place in again. The same is true of the lore entry describing the
 * character: writing it meant going to write it somewhere else.
 *
 * # Why this is not a link to the editor pages
 *
 * FR-035 puts NPC and item *creation* on dedicated editing pages with an
 * explicit save, and that is where a Game Master goes to author something
 * properly — an item with effects, permissions, a price, an icon. Nothing here
 * replaces it. What is offered on the actor's screen is the narrow case that
 * sent them away in the first place: a name, and the thing exists and is
 * already in this character's hands. It still saves explicitly, and the
 * compendium *lists* still carry no creation form, which is what FR-035
 * actually forbids.
 *
 * # Why these are functions and not handlers in the panels
 *
 * Each is two server calls where the second depends on the first, and
 * `apps/web`'s vitest environment is `node` — the sequence is the part worth
 * asserting, and it is not reachable through a rendered component. The panels
 * draw the outcome; the order and the partial failures live here.
 */

import type { InventoryEntryRecord } from "@/types/inventory";
import type { LoreEntryRecord } from "@/types/lore";
import type { WorldItemRecord } from "@/types/item";

/**
 * The in-text link that attaches a lore entry to an actor.
 *
 * `[[Title]]` is the wiki-link the lore pipeline already resolves at save time
 * (spec 012), against lore entries, actors and items by name. Attaching by
 * writing one means an entry attached from here is attached in exactly the
 * same way as one a Game Master typed by hand — there is no second kind of
 * link for the actor page to keep working.
 *
 * The catch, and it is why the panels re-read the actor afterwards rather than
 * announcing success: resolution prefers a lore entry of the same name, so an
 * entry titled after the character wins over the character. That is a real
 * outcome and the only honest way to detect it is to look.
 */
export function actorLink(actorLabel: string): string {
  return `[[${actorLabel}]]`;
}

/** Whether `content` already links to this actor, so attaching is a no-op. */
export function alreadyLinks(content: string, actorLabel: string): boolean {
  return content.includes(actorLink(actorLabel));
}

export interface CreateItemCalls {
  createItem: (input: {
    worldId: string;
    name: string;
    description?: string | null;
  }) => Promise<Pick<WorldItemRecord, "id" | "name">>;
  addItemToInventory: (
    actorId: string,
    itemId: string,
    quantity: number,
  ) => Promise<Pick<InventoryEntryRecord, "id">>;
}

export type CreateItemOutcome =
  | { kind: "given"; itemId: string; itemName: string }
  | { kind: "createdOnly"; itemId: string; message: string }
  | { kind: "refused"; message: string };

/**
 * Make an item and put it in this character's hands, in that order.
 *
 * The middle outcome is the one that has to be named: the item can be created
 * and then fail to be added. Reporting that as a flat failure would send the
 * Game Master to create a second copy of an item that already exists.
 */
export async function createAndGiveItem(
  calls: CreateItemCalls,
  input: {
    worldId: string;
    actorId: string;
    name: string;
    description?: string | null;
    quantity: number;
  },
): Promise<CreateItemOutcome> {
  let item: Pick<WorldItemRecord, "id" | "name">;
  try {
    item = await calls.createItem({
      worldId: input.worldId,
      name: input.name,
      description: input.description ?? null,
    });
  } catch (error) {
    return {
      kind: "refused",
      message: asMessage(error, "Could not make that item."),
    };
  }

  try {
    await calls.addItemToInventory(input.actorId, item.id, input.quantity);
    return { kind: "given", itemId: item.id, itemName: item.name };
  } catch {
    return {
      kind: "createdOnly",
      itemId: item.id,
      message: `${item.name} was made but could not be added. It is in the compendium — add it from the list below.`,
    };
  }
}

export interface LoreCalls {
  createLoreEntry: (input: {
    worldId: string;
    title: string;
    content?: string;
  }) => Promise<Pick<LoreEntryRecord, "id" | "title">>;
  updateLoreEntry: (input: {
    loreEntryId: string;
    content?: string;
    expectedCurrentRevisionId?: string | null;
  }) => Promise<Pick<LoreEntryRecord, "id" | "title">>;
}

export type LoreOutcome =
  | { kind: "linked"; entryId: string; title: string }
  | { kind: "alreadyLinked"; entryId: string; title: string }
  | { kind: "refused"; message: string };

/**
 * Write a new entry that mentions this character, so it files itself.
 *
 * The link goes in the body rather than into a field of its own because there
 * is no such field: an entry is attached to an actor by linking to it, and
 * inventing a second attachment here would mean the actor page showing
 * something the lore page could not.
 */
export async function createLoreEntryAbout(
  calls: LoreCalls,
  input: {
    worldId: string;
    actorLabel: string;
    title: string;
    content: string;
  },
): Promise<LoreOutcome> {
  const body = input.content.trim();
  const link = actorLink(input.actorLabel);
  try {
    const entry = await calls.createLoreEntry({
      worldId: input.worldId,
      title: input.title,
      content: body === "" ? link : `${body}\n\n${link}`,
    });
    return { kind: "linked", entryId: entry.id, title: entry.title };
  } catch (error) {
    return {
      kind: "refused",
      message: asMessage(error, "Could not write that entry."),
    };
  }
}

/**
 * Attach an entry that already exists by adding the link to its body.
 *
 * `expectedCurrentRevisionId` is the entry's own, so somebody editing that
 * entry in another tab wins the conflict rather than silently losing their
 * paragraph to this one-line append (FR-019). The Game Master is told to try
 * again, which is true and is the whole of what they can do.
 */
export async function attachLoreEntry(
  calls: LoreCalls,
  entry: Pick<
    LoreEntryRecord,
    "id" | "title" | "content" | "currentRevisionId"
  >,
  actorLabel: string,
): Promise<LoreOutcome> {
  if (alreadyLinks(entry.content, actorLabel)) {
    // Appending a second identical link would change the entry's text for no
    // effect: the backlink already exists.
    return { kind: "alreadyLinked", entryId: entry.id, title: entry.title };
  }

  try {
    await calls.updateLoreEntry({
      loreEntryId: entry.id,
      content: `${entry.content.trimEnd()}\n\n${actorLink(actorLabel)}`,
      expectedCurrentRevisionId: entry.currentRevisionId,
    });
    return { kind: "linked", entryId: entry.id, title: entry.title };
  } catch (error) {
    return {
      kind: "refused",
      message: asMessage(
        error,
        "Could not attach that entry — somebody may have just edited it.",
      ),
    };
  }
}

function asMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback;
}
