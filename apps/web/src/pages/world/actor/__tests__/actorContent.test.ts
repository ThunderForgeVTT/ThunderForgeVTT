import { describe, expect, it } from "vitest";
import {
  actorLink,
  attachLoreEntry,
  createAndGiveItem,
  createLoreEntryAbout,
  type CreateItemCalls,
  type LoreCalls,
} from "../actorContent";

/**
 * Spec 031 FR-039 — creating and attaching from the actor's screen.
 *
 * The claims are about sequence and about the states in between: an item that
 * exists but was not handed over, an entry that already links here, and an
 * append that lost a race with somebody editing the same entry. None of those
 * can be provoked from the interface, and none of them are visible to a test
 * that only checks the happy path returned true.
 */

const ENTRY = {
  id: "entry-1",
  title: "The Ledger",
  content: "Debts nobody wishes to settle.",
  currentRevisionId: "rev-1",
};

describe("createAndGiveItem", () => {
  const calls = (overrides: Partial<CreateItemCalls> = {}) => {
    const seen: string[] = [];
    return {
      seen,
      calls: {
        createItem: async ({ name }) => {
          seen.push(`createItem:${name}`);
          return { id: "item-1", name };
        },
        addItemToInventory: async (actorId, itemId, quantity) => {
          seen.push(`add:${actorId}:${itemId}:${quantity}`);
          return { id: "entry-1" };
        },
        ...overrides,
      } satisfies CreateItemCalls,
    };
  };

  it("makes the item, then puts it in the character's hands", async () => {
    const { calls: injected, seen } = calls();

    const outcome = await createAndGiveItem(injected, {
      worldId: "world-1",
      actorId: "actor-1",
      name: "Rusted Key",
      quantity: 2,
    });

    expect(outcome).toEqual({
      kind: "given",
      itemId: "item-1",
      itemName: "Rusted Key",
    });
    expect(seen).toEqual(["createItem:Rusted Key", "add:actor-1:item-1:2"]);
  });

  it("admits the item exists when only handing it over failed", async () => {
    const { calls: injected } = calls({
      addItemToInventory: async () => {
        throw new Error("no");
      },
    });

    const outcome = await createAndGiveItem(injected, {
      worldId: "world-1",
      actorId: "actor-1",
      name: "Rusted Key",
      quantity: 1,
    });

    // Told as itself, so the Game Master does not go and make a second copy of
    // an item that is already in the compendium.
    expect(outcome.kind).toBe("createdOnly");
    expect(outcome.kind === "createdOnly" && outcome.itemId).toBe("item-1");
  });
});

describe("lore attached from the actor's screen", () => {
  const calls = (overrides: Partial<LoreCalls> = {}) => {
    const created: { title: string; content?: string }[] = [];
    const updated: { loreEntryId: string; content?: string }[] = [];
    return {
      created,
      updated,
      calls: {
        createLoreEntry: async (input) => {
          created.push(input);
          return { id: "new-entry", title: input.title };
        },
        updateLoreEntry: async (input) => {
          updated.push(input);
          return { id: input.loreEntryId, title: "The Ledger" };
        },
        ...overrides,
      } satisfies LoreCalls,
    };
  };

  it("writes the character's name into the entry, which is what attaches it", async () => {
    const { calls: injected, created } = calls();

    const outcome = await createLoreEntryAbout(injected, {
      worldId: "world-1",
      actorLabel: "Ysolde",
      title: "The Ledger",
      content: "Debts.",
    });

    expect(outcome.kind).toBe("linked");
    expect(created[0].content).toBe(`Debts.\n\n${actorLink("Ysolde")}`);
  });

  it("writes only the link when nothing else was typed", async () => {
    const { calls: injected, created } = calls();

    await createLoreEntryAbout(injected, {
      worldId: "world-1",
      actorLabel: "Ysolde",
      title: "The Ledger",
      content: "   ",
    });

    expect(created[0].content).toBe(actorLink("Ysolde"));
  });

  it("appends the link to an existing entry against its own revision", async () => {
    const { calls: injected, updated } = calls();

    const outcome = await attachLoreEntry(injected, ENTRY, "Ysolde");

    expect(outcome.kind).toBe("linked");
    expect(updated[0]).toMatchObject({
      loreEntryId: "entry-1",
      // The revision it was read at, so somebody editing the same entry in
      // another tab wins the conflict rather than losing their paragraph.
      expectedCurrentRevisionId: "rev-1",
    });
    expect(updated[0].content).toContain(actorLink("Ysolde"));
  });

  it("changes nothing when the entry already links here", async () => {
    const { calls: injected, updated } = calls();

    const outcome = await attachLoreEntry(
      injected,
      { ...ENTRY, content: `Debts. ${actorLink("Ysolde")}` },
      "Ysolde",
    );

    expect(outcome.kind).toBe("alreadyLinked");
    expect(updated).toHaveLength(0);
  });

  it("says an append lost a race rather than reporting nothing", async () => {
    const { calls: injected } = calls({
      updateLoreEntry: async () => {
        throw new Error("CONFLICT");
      },
    });

    const outcome = await attachLoreEntry(injected, ENTRY, "Ysolde");

    expect(outcome.kind).toBe("refused");
  });
});
