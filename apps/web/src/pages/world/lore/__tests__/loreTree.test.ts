import { describe, expect, it } from "vitest";
import {
  allTagsOf,
  ancestorsOf,
  buildLoreTree,
  descendantIdsOf,
  filterLoreEntries,
  flattenLoreTree,
  matchesLoreQuery,
  normaliseTag,
  validMoveTargets,
  type LoreTreeEntry,
} from "@/pages/world/lore/loreTree";

function entry(
  id: string,
  title: string,
  parentId: string | null = null,
  tags: string[] = [],
): LoreTreeEntry {
  return { id, title, slug: title.toLowerCase(), parentId, tags };
}

/** Realms > Kingdoms > {Veldrath, Tarn}, plus an unrelated root. */
const WORLD: LoreTreeEntry[] = [
  entry("veldrath", "Veldrath", "kingdoms", ["ruins", "elven"]),
  entry("realms", "Realms"),
  entry("gods", "Gods", null, ["divine"]),
  entry("tarn", "Tarn", "kingdoms"),
  entry("kingdoms", "Kingdoms", "realms"),
];

function titlesInOrder(entries: readonly LoreTreeEntry[]): string[] {
  return flattenLoreTree(buildLoreTree(entries)).map(
    (node) => node.entry.title,
  );
}

describe("buildLoreTree", () => {
  it("nests children under their parent regardless of input order", () => {
    const tree = buildLoreTree(WORLD);
    expect(tree.map((node) => node.entry.title)).toEqual(["Gods", "Realms"]);

    const realms = tree[1];
    expect(realms.children.map((node) => node.entry.title)).toEqual([
      "Kingdoms",
    ]);
    expect(realms.children[0].children.map((node) => node.entry.title)).toEqual(
      ["Tarn", "Veldrath"],
    );
  });

  it("reports the depth a row should be indented to", () => {
    const depths = Object.fromEntries(
      flattenLoreTree(buildLoreTree(WORLD)).map((node) => [
        node.entry.title,
        node.depth,
      ]),
    );
    expect(depths).toEqual({
      Gods: 0,
      Realms: 0,
      Kingdoms: 1,
      Tarn: 2,
      Veldrath: 2,
    });
  });

  it("shows an entry whose parent is absent rather than hiding it", () => {
    // The parent is under a takedown placeholder, or the caller passed a
    // subset. Either way the child is still somebody's page.
    const orphan = [entry("child", "Child", "missing-parent")];
    expect(titlesInOrder(orphan)).toEqual(["Child"]);
  });

  it("terminates on a cycle instead of descending forever", () => {
    // The server refuses to write this. A walk that trusts that and is wrong
    // hangs the tab, so the walk does not trust it.
    const looped = [
      entry("a", "A", "b"),
      entry("b", "B", "a"),
      entry("free", "Free"),
    ];
    expect(titlesInOrder(looped)).toEqual(["Free"]);
  });
});

describe("ancestorsOf", () => {
  it("gives the breadcrumb root-first, without the entry itself", () => {
    expect(ancestorsOf(WORLD, "veldrath").map((e) => e.title)).toEqual([
      "Realms",
      "Kingdoms",
    ]);
  });

  it("is empty for a root", () => {
    expect(ancestorsOf(WORLD, "realms")).toEqual([]);
  });
});

describe("validMoveTargets", () => {
  it("never offers the entry itself or anything beneath it", () => {
    // Exactly the moves the server refuses with LORE_CYCLE.
    expect(descendantIdsOf(WORLD, "kingdoms")).toEqual(
      new Set(["kingdoms", "tarn", "veldrath"]),
    );
    expect(validMoveTargets(WORLD, "kingdoms").map((e) => e.title)).toEqual([
      "Gods",
      "Realms",
    ]);
  });

  it("offers everything else to a leaf", () => {
    expect(validMoveTargets(WORLD, "tarn").map((e) => e.title)).toEqual([
      "Gods",
      "Kingdoms",
      "Realms",
      "Veldrath",
    ]);
  });
});

describe("matchesLoreQuery", () => {
  const veldrath = entry("veldrath", "Veldrath", null, ["ruins", "elven"]);

  it("keeps everything when nothing has been typed", () => {
    expect(matchesLoreQuery(veldrath, "   ")).toBe(true);
  });

  it("finds an entry by title, whatever the case", () => {
    expect(matchesLoreQuery(veldrath, "VELD")).toBe(true);
  });

  it("finds an entry by a tag, which is the other half of FR-038", () => {
    expect(matchesLoreQuery(veldrath, "ruins")).toBe(true);
    expect(matchesLoreQuery(veldrath, "dwarven")).toBe(false);
  });

  it("restricts the search to tags behind a # prefix", () => {
    const named = entry("ruins", "Ruins", null, ["divine"]);
    expect(matchesLoreQuery(named, "ruins")).toBe(true);
    expect(matchesLoreQuery(named, "#ruins")).toBe(false);
    expect(matchesLoreQuery(named, "#divine")).toBe(true);
  });

  it("treats a bare # as 'anything that is tagged at all'", () => {
    expect(matchesLoreQuery(veldrath, "#")).toBe(true);
    expect(matchesLoreQuery(entry("tarn", "Tarn"), "#")).toBe(false);
  });
});

describe("filterLoreEntries", () => {
  it("keeps the branch a match hangs from", () => {
    // Without the ancestors, Veldrath would appear at the top level and the
    // filter would look like it had moved the entry.
    expect(titlesInOrder(filterLoreEntries(WORLD, "veldrath"))).toEqual([
      "Realms",
      "Kingdoms",
      "Veldrath",
    ]);
  });

  it("finds a buried entry by its tag alone", () => {
    expect(filterLoreEntries(WORLD, "elven").map((e) => e.title)).toEqual([
      "Veldrath",
      "Realms",
      "Kingdoms",
    ]);
  });

  it("returns every entry for an empty query", () => {
    expect(filterLoreEntries(WORLD, "")).toHaveLength(WORLD.length);
  });
});

describe("normaliseTag", () => {
  it("agrees with the server on what one tag is", () => {
    expect(normaliseTag("  Ancient   RUINS ")).toBe("ancient ruins");
  });
});

describe("allTagsOf", () => {
  it("lists each tag once, alphabetically", () => {
    expect(allTagsOf(WORLD)).toEqual(["divine", "elven", "ruins"]);
  });
});
