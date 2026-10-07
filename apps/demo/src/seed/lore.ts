/**
 * The ambush's lore: three short entries the Game Master has already written,
 * so the wiki opens on something to read and links that go somewhere.
 *
 * Written for the demo; nothing here is drawn from a published adventure. No
 * entry is titled after a member of the cast, because a title would win the
 * `[[...]]` link that is meant to reach that character's sheet.
 */
import type { Row } from "../backend/state";

interface SeedEntry {
  title: string;
  slug: string;
  tags: string[];
  content: string;
}

const ENTRIES: SeedEntry[] = [
  {
    title: "The Grassy Path",
    slug: "the-grassy-path",
    tags: ["places"],
    content: [
      "A cart road worn into the downs, two ruts and a ridge of grass between them.",
      "",
      "Where it dips between the rocks, the brush on the bank grows high enough to",
      "hide a crouching goblin. Travellers who know the road walk that stretch fast",
      "and do not stop to water their horses.",
      "",
      "See also [[The Ambush]].",
    ].join("\n"),
  },
  {
    title: "The Ambush",
    slug: "the-ambush",
    tags: ["adventure", "secrets"],
    content: [
      "Goblins wait in the brush above [[The Grassy Path]], a hobgoblin keeps them",
      "in line, and a wolf lies behind the rocks for whoever runs.",
      "",
      "- The goblins break and flee once half of them fall.",
      "- The hobgoblin fights on until it is alone.",
      "- The wolf answers only to the hobgoblin's whistle.",
    ].join("\n"),
  },
  {
    title: "The Stoneward Oath",
    slug: "the-stoneward-oath",
    tags: ["characters"],
    content: [
      "[[Brannoc Stoneward]] swore to see every traveller on the road home, and",
      "keeps the oath on a cord at his neck: a pebble from each road he has walked.",
      "",
      "[[Elowen Vire]] laughs at the pebbles, and has never once asked him to stop.",
    ].join("\n"),
  },
];

/**
 * The seeded entries, each with the one revision its text was saved as.
 * `id(kind, n)` is the seed's own identifier scheme.
 */
export function seedLore(
  worldId: string,
  createdBy: string,
  at: string,
  id: (kind: number, index: number) => string,
): Row[] {
  return ENTRIES.map((entry, index) => {
    const entryId = id(20, index + 1);
    const revisionId = id(21, index + 1);
    return {
      id: entryId,
      worldId,
      title: entry.title,
      slug: entry.slug,
      content: entry.content,
      currentRevisionId: revisionId,
      createdBy,
      createdAt: at,
      updatedAt: at,
      parentId: null,
      tags: [...entry.tags].sort(),
      permissions: [],
      revisions: [
        {
          id: revisionId,
          loreEntryId: entryId,
          contentMarkdown: entry.content,
          authorId: createdBy,
          restoredFromRevisionId: null,
          createdAt: at,
        },
      ],
    };
  });
}
