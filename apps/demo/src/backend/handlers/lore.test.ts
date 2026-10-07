/**
 * The lore wiki, asked through the server's schema: written, edited, moved,
 * tagged and restored by the server's rules, and rendered as it renders.
 */
import { beforeEach, describe, expect, it } from "vitest";
import { ask, freshWorld, must, refused, viewAs } from "../testing/world";
import { DEMO_PLAYER } from "../../seed/world";
import { demoState } from "../state";

const ENTRY = "id title slug content currentRevisionId parentId tags renderedHtml myPermissionLevel";
const LIST = `query ($worldId: UUID!) { worldLoreEntries(worldId: $worldId) { ${ENTRY} } }`;
const ONE = `query ($worldId: UUID!, $slug: String!) { loreEntry(worldId: $worldId, slug: $slug) { ${ENTRY} linkedFrom { title } } }`;
const CREATE = `mutation ($input: CreateLoreEntryInput!) { createLoreEntry(input: $input) { ${ENTRY} } }`;
const UPDATE = `mutation ($input: UpdateLoreEntryInput!) { updateLoreEntry(input: $input) { ${ENTRY} } }`;
const DELETE = `mutation ($id: UUID!) { deleteLoreEntry(loreEntryId: $id) }`;
const REVISIONS = `query ($id: UUID!) { loreEntryRevisions(loreEntryId: $id) { id contentMarkdown restoredFromRevisionId renderedHtml } }`;
const RESTORE = `mutation ($revisionId: UUID!) { restoreLoreRevision(revisionId: $revisionId) { content currentRevisionId } }`;
const MOVE = `mutation ($input: MoveLoreEntryInput!) { moveLoreEntry(input: $input) { id parentId } }`;
const ADD_TAG = `mutation ($input: LoreTagInput!) { addLoreTag(input: $input) }`;
const REMOVE_TAG = `mutation ($input: LoreTagInput!) { removeLoreTag(input: $input) }`;
const TARGETS = `query ($worldId: UUID!, $prefix: String!) { loreLinkTargets(worldId: $worldId, prefix: $prefix) { id title kind } }`;
const SET_PERMISSION = `mutation ($input: SetLorePermissionInput!) { setLorePermission(input: $input) { userId level } }`;

const worldId = () => demoState().world.id;

async function create(title: string, content = "") {
  return (await must(CREATE, { input: { worldId: worldId(), title, content } }))
    .createLoreEntry;
}

beforeEach(async () => {
  await freshWorld("gm");
});

describe("the seeded wiki", () => {
  it("opens on the ambush's entries, with links that reach a sheet and each other", async () => {
    const { worldLoreEntries } = await must(LIST, { worldId: worldId() });
    expect(worldLoreEntries.map((e: { title: string }) => e.title)).toEqual([
      "The Ambush",
      "The Grassy Path",
      "The Stoneward Oath",
    ]);
    const oath = worldLoreEntries.find(
      (e: { slug: string }) => e.slug === "the-stoneward-oath",
    );
    expect(oath.renderedHtml).toMatch(
      /<a class="lore-link" href="\/world\/[^"]+\/actor\/[^"]+\/view">Brannoc Stoneward<\/a>/,
    );
    const { loreEntry } = await must(ONE, {
      worldId: worldId(),
      slug: "the-grassy-path",
    });
    expect(loreEntry.linkedFrom.map((e: { title: string }) => e.title)).toEqual(
      ["The Ambush"],
    );
    expect(refused).toEqual([]);
  });
});

describe("rendering", () => {
  it("is markdown with its raw HTML dropped, and an unknown link marked broken", async () => {
    const entry = await create(
      "Rendering",
      "**bold** <script>alert(1)</script> [[Nowhere At All]] `[[Not A Link]]` [site](https://example.com)",
    );
    expect(entry.renderedHtml).toContain("<strong>bold</strong>");
    expect(entry.renderedHtml).not.toContain("<script");
    expect(entry.renderedHtml).toContain(
      '<span class="lore-link-broken" title="Unresolved link">Nowhere At All</span>',
    );
    expect(entry.renderedHtml).toContain("<code>[[Not A Link]]</code>");
    expect(entry.renderedHtml).toContain('rel="noopener noreferrer"');
  });

  it("gives a second entry of the same title the next free slug", async () => {
    expect((await create("Old Mill")).slug).toBe("old-mill");
    expect((await create("Old Mill")).slug).toBe("old-mill-2");
    expect((await create("!!!")).slug).toBe("entry");
  });
});

describe("editing", () => {
  it("saves a revision against the one it was opened at, and refuses a stale one", async () => {
    const entry = await create("Ledger", "first");
    const edited = (
      await must(UPDATE, {
        input: {
          loreEntryId: entry.id,
          content: "second",
          expectedCurrentRevisionId: entry.currentRevisionId,
        },
      })
    ).updateLoreEntry;
    expect(edited.content).toBe("second");
    expect(edited.currentRevisionId).not.toBe(entry.currentRevisionId);

    const stale = await ask(UPDATE, {
      input: {
        loreEntryId: entry.id,
        content: "third",
        expectedCurrentRevisionId: entry.currentRevisionId,
      },
    });
    expect(stale.errors?.[0]?.extensions?.code).toBe("CONFLICT");

    const { loreEntryRevisions } = await must(REVISIONS, { id: entry.id });
    expect(loreEntryRevisions.map((r: { contentMarkdown: string }) => r.contentMarkdown))
      .toEqual(["second", "first"]);

    const oldest = loreEntryRevisions[1];
    const restored = (await must(RESTORE, { revisionId: oldest.id }))
      .restoreLoreRevision;
    expect(restored.content).toBe("first");
    const after = (await must(REVISIONS, { id: entry.id })).loreEntryRevisions;
    expect(after).toHaveLength(3);
    expect(after[0].restoredFromRevisionId).toBe(oldest.id);
  });

  it("renames an entry and its slug follows", async () => {
    const entry = await create("Draft");
    const renamed = (
      await must(UPDATE, { input: { loreEntryId: entry.id, title: "Final Copy" } })
    ).updateLoreEntry;
    expect(renamed.slug).toBe("final-copy");
  });

  it("deletes an entry and lifts its children to its own parent", async () => {
    const parent = await create("Parent");
    const child = await create("Child");
    await must(MOVE, { input: { loreEntryId: child.id, parentId: parent.id } });
    expect(await must(DELETE, { id: parent.id })).toEqual({ deleteLoreEntry: true });
    const left = demoState().lore.find((e) => e.id === child.id)!;
    expect(left.parentId).toBeNull();
  });
});

describe("the tree", () => {
  it("refuses a move that would put an entry inside itself", async () => {
    const top = await create("Top");
    const under = await create("Under");
    await must(MOVE, { input: { loreEntryId: under.id, parentId: top.id } });
    const loop = await ask(MOVE, {
      input: { loreEntryId: top.id, parentId: under.id },
    });
    expect(loop.errors?.[0]?.message).toBe(
      "that move would put the entry inside itself",
    );
    expect(loop.errors?.[0]?.extensions?.code).toBe("LORE_CYCLE");
  });
});

describe("tags", () => {
  it("are tidied, kept once, sorted, and refused when empty", async () => {
    const entry = await create("Tagged");
    await must(ADD_TAG, { input: { loreEntryId: entry.id, tag: "  Old   Road " } });
    await must(ADD_TAG, { input: { loreEntryId: entry.id, tag: "old road" } });
    const { addLoreTag } = await must(ADD_TAG, {
      input: { loreEntryId: entry.id, tag: "Bandits" },
    });
    expect(addLoreTag).toEqual(["bandits", "old road"]);
    const { removeLoreTag } = await must(REMOVE_TAG, {
      input: { loreEntryId: entry.id, tag: "OLD ROAD" },
    });
    expect(removeLoreTag).toEqual(["bandits"]);
    const empty = await ask(ADD_TAG, {
      input: { loreEntryId: entry.id, tag: "   " },
    });
    expect(empty.errors?.[0]?.message).toBe(
      "a tag needs at least one character and at most 64",
    );
  });
});

describe("link targets", () => {
  it("offer entries and the actors the viewer may know, by what they start with", async () => {
    const gm = (await must(TARGETS, { worldId: worldId(), prefix: "the " }))
      .loreLinkTargets;
    expect(gm.map((t: { title: string }) => t.title)).toEqual([
      "The Ambush",
      "The Grassy Path",
      "The Stoneward Oath",
    ]);
    const heroes = (await must(TARGETS, { worldId: worldId(), prefix: "brann" }))
      .loreLinkTargets;
    expect(heroes).toMatchObject([{ title: "Brannoc Stoneward", kind: "ACTOR" }]);

    const hobgoblin = demoState().actors.find((a) => a.castKey === "hobgoblin")!;
    const prefix = String(hobgoblin.label).slice(0, 3);
    const gmSees = (await must(TARGETS, { worldId: worldId(), prefix }))
      .loreLinkTargets;
    expect(gmSees.some((t: { id: string }) => t.id === hobgoblin.id)).toBe(true);
    viewAs("player");
    const playerSees = (await must(TARGETS, { worldId: worldId(), prefix }))
      .loreLinkTargets;
    expect(playerSees.some((t: { id: string }) => t.id === hobgoblin.id)).toBe(
      false,
    );
  });
});

describe("a player", () => {
  it("reads every entry but may not write, edit or tag without a grant", async () => {
    const entry = await create("Player Notes", "hello");
    viewAs("player");
    const list = (await must(LIST, { worldId: worldId() })).worldLoreEntries;
    expect(list.every((e: { myPermissionLevel: string }) => e.myPermissionLevel === "VIEWER"))
      .toBe(true);

    const write = await ask(CREATE, {
      input: { worldId: worldId(), title: "Mine" },
    });
    expect(write.errors?.[0]?.message).toMatch(/^Only the DM \(Owner or GM\) may/);

    const edit = await ask(UPDATE, {
      input: { loreEntryId: entry.id, title: "Theirs" },
    });
    expect(edit.errors?.[0]?.message).toBe(
      "You do not have sufficient permission on this lore entry",
    );

    viewAs("gm");
    await must(SET_PERMISSION, {
      input: { loreEntryId: entry.id, userId: DEMO_PLAYER.id, level: "EDITOR" },
    });
    viewAs("player");
    const allowed = (
      await must(UPDATE, { input: { loreEntryId: entry.id, title: "Shared Notes" } })
    ).updateLoreEntry;
    expect(allowed.myPermissionLevel).toBe("EDITOR");
    const remove = await ask(DELETE, { id: entry.id });
    expect(remove.errors?.[0]?.message).toBe(
      "You do not have sufficient permission on this lore entry",
    );
    expect(refused).toEqual([]);
  });
});
