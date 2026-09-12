import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  BESTIARY,
  createMonster,
  HERO_PARTS,
  monsterPack,
  monsterSpec,
  readCreature,
  SIZE_CATEGORIES,
  validateHero,
} from "./index.ts";

/** Every opened element is closed, in order. Not a full XML parser — the
 * output is ours and has no comments, CDATA or `>` inside attributes. */
function assertWellFormed(svg: string): void {
  const open: string[] = [];
  for (const [, closing, tag, selfClosing] of svg.matchAll(
    /<(\/?)([A-Za-z][\w:-]*)\b[^>]*?(\/?)>/g,
  )) {
    if (selfClosing) continue;
    if (closing) assert.equal(open.pop(), tag, `</${tag}> closes nothing open`);
    else open.push(tag);
  }
  assert.deepEqual(open, [], "every element is closed");
}

describe("the bestiary", () => {
  it("is creatures with their own slugs, each drawing a well-formed token", () => {
    const slugs = new Set(BESTIARY.map((entry) => entry.slug));
    assert.equal(slugs.size, BESTIARY.length);
    for (const { slug, source } of BESTIARY) {
      const result = validateHero(monsterSpec(source));
      assert.ok(result.ok, `${slug}: ${result.ok ? "" : result.problems}`);
      const monster = createMonster(source);
      assertWellFormed(monster.portrait());
      assertWellFormed(monster.token());
    }
  });

  it("draws no two creatures the same", () => {
    // A bestiary where the ghoul and the zombie come out identical is worse
    // than a shorter one: a Game Master cannot tell which token is which.
    const drawn = new Set(BESTIARY.map((e) => createMonster(e.source).token()));
    assert.equal(drawn.size, BESTIARY.length);
  });

  it("draws every creature from the parts a hero is drawn from", () => {
    // The style holds because a monster has no vocabulary of its own: if a
    // creature ever reaches for a part outside these lists, it has started a
    // second art style and this fails.
    for (const { slug, source } of BESTIARY) {
      const spec = createMonster(source).spec;
      for (const [field, options] of Object.entries(HERO_PARTS)) {
        const chosen = spec[field as keyof typeof spec];
        assert.ok(
          (options as readonly unknown[]).includes(chosen),
          `${slug}: ${field} is ${String(chosen)}`,
        );
      }
    }
  });
});

describe("a monster read out of a book", () => {
  it("needs only a name", () => {
    const monster = createMonster({ name: "Thing In The Well" });
    assertWellFormed(monster.token());
    assert.equal(monster.spec.size, "medium");
  });

  it("draws the same creature for the same name, every time", () => {
    const source = { name: "Grash the Unwashed", descriptor: "Large giant" };
    assert.equal(createMonster(source).token(), createMonster(source).token());
    assert.notEqual(
      createMonster(source).token(),
      createMonster({ ...source, name: "Brak the Unwashed" }).token(),
    );
  });

  it("reads the size and the creature type off a descriptor line", () => {
    const reading = readCreature({
      name: "Ancient Red Dragon",
      descriptor: "Gargantuan dragon, chaotic evil",
    });
    assert.equal(reading.family, "dragon");
    assert.equal(reading.size, "gargantuan");
    assert.equal(
      createMonster({
        name: "Ancient Red Dragon",
        descriptor: "Gargantuan dragon, chaotic evil",
      }).footprint,
      SIZE_CATEGORIES.gargantuan,
    );
  });

  it("knows a hobgoblin from a goblin", () => {
    // The trap in matching a creature by a word in its name: "hobgoblin"
    // contains "goblin", and the shorter match would draw the wrong monster
    // for a whole branch of the bestiary.
    assert.equal(
      readCreature({ name: "Hobgoblin Captain" }).creature,
      "hobgoblin",
    );
    assert.equal(readCreature({ name: "Goblin Boss" }).creature, "goblin");
    assert.notEqual(
      createMonster({ name: "Goblin" }).token(),
      createMonster({ name: "Hobgoblin" }).token(),
    );
  });

  it("takes a colour out of the creature's own name", () => {
    const red = createMonster({ name: "Red Dragon" }).spec;
    const blue = createMonster({ name: "Blue Dragon" }).spec;
    assert.notEqual(red.skin, blue.skin);
    assert.equal(
      readCreature({ name: "White Dragon Wyrmling" }).tint,
      "#e2e8ee",
    );
  });

  it("falls back to the creature type for a name nobody has written down", () => {
    const source = {
      name: "Skitterling Broodmother",
      descriptor: "Medium beast, unaligned",
    };
    const reading = readCreature(source);
    assert.equal(reading.family, "beast");
    assert.equal(reading.creature, null);
    assert.equal(createMonster(source).spec.muzzle, "snout");
  });

  it("falls back again to whatever the name itself says", () => {
    // A source that kept only the name is the common case for a badly
    // extracted page, and "Fire Giant" still says giant.
    assert.equal(readCreature({ name: "Fire Giant" }).family, "giant");
    assert.equal(readCreature({ name: "Air Elemental" }).family, "elemental");
    assert.equal(
      readCreature({ name: "Utterly Unknown" }).family,
      "monstrosity",
    );
  });

  it("carries a creature's size to whoever places the token", () => {
    assert.equal(createMonster({ name: "Ogre" }).footprint, 2);
    assert.equal(createMonster({ name: "Goblin" }).spec.size, "small");
    assert.match(createMonster({ name: "Ogre" }).token(), /data-size="large"/);
  });

  it("lets a caller overrule the book about the size", () => {
    const reading = readCreature({
      name: "Goblin",
      descriptor: "Small humanoid",
      size: "Huge",
    });
    assert.equal(reading.size, "huge");
  });
});

describe("a pack of one creature", () => {
  it("is six different goblins, and the same six the next time", () => {
    const pack = monsterPack({ name: "Goblin" }, 6);
    const drawn = new Set(pack.map((goblin) => goblin.token()));
    assert.equal(drawn.size, 6);
    assert.deepEqual(
      monsterPack({ name: "Goblin" }, 6).map((g) => g.token()),
      pack.map((g) => g.token()),
    );
  });

  it("is six goblins, not six different monsters", () => {
    // Variation that reached the parts that say "goblin" would give a pack
    // of strangers; only the rolled fields may move.
    for (const goblin of monsterPack({ name: "Goblin" }, 6)) {
      assert.equal(goblin.spec.ears, "pointed");
      assert.equal(goblin.spec.hide, "warts");
      assert.equal(goblin.spec.size, "small");
    }
  });
});
