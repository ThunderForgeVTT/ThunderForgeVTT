import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  createHero,
  HERO_PARTS,
  HeroSpecError,
  PRESET_HEROES,
  renderPortrait,
  renderToken,
  validateHero,
  type HeroSpec,
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

const ids = (svg: string) =>
  new Set([...svg.matchAll(/ id="([^"]+)"/g)].map((m) => m[1]));

describe("the presets", () => {
  it("are valid heroes, each with its own slug", () => {
    const slugs = new Set(PRESET_HEROES.map((p) => p.slug));
    assert.equal(slugs.size, PRESET_HEROES.length);
    for (const { slug, spec } of PRESET_HEROES) {
      const result = validateHero(spec);
      assert.ok(result.ok, `${slug}: ${result.ok ? "" : result.problems}`);
    }
  });

  it("each draw a well-formed portrait and token", () => {
    for (const { spec } of PRESET_HEROES) {
      const hero = createHero(spec);
      assertWellFormed(hero.portrait());
      assertWellFormed(hero.token());
      assert.match(hero.portrait(), new RegExp(`<title>${spec.name}, `));
    }
  });
});

describe("the factory", () => {
  it("needs only a name", () => {
    const hero = createHero({ name: "Nobody" });
    assertWellFormed(hero.token());
    assert.equal(hero.spec.hair, "short");
    assert.equal(hero.spec.ring, hero.spec.outfit);
  });

  it("offers no choice that draws the same as another", () => {
    // What a builder shows is what it gets: every option of every part
    // changes the drawing, so no control in it is a dead one.
    const base: HeroSpec = { name: "Probe" };
    for (const [field, options] of Object.entries(HERO_PARTS)) {
      const drawn = new Set(
        options.map((option) =>
          renderToken({ ...base, [field]: option } as HeroSpec),
        ),
      );
      assert.equal(drawn.size, options.length, `every ${field} draws apart`);
    }
  });

  it("draws a hero the same way every time, and two heroes' ids never clash", () => {
    const pip = PRESET_HEROES[0]!.spec;
    assert.equal(renderPortrait(pip), renderPortrait(pip));
    const other = renderPortrait({ ...pip, glow: "#000000" });
    const shared = [...ids(renderPortrait(pip))].filter((id) =>
      ids(other).has(id),
    );
    assert.deepEqual(shared, []);
    assert.ok(
      renderToken(pip, { idPrefix: "builder" }).includes('id="builder-'),
    );
  });

  it("writes a name as text, never as markup", () => {
    const svg = renderPortrait({
      name: `Bob <script>alert(1)</script> & "co"`,
      title: "the <b>Bold</b>",
    });
    assert.ok(!svg.includes("<script"));
    assert.ok(!svg.includes("<b>"));
    assert.ok(svg.includes("&lt;script&gt;"));
    assertWellFormed(svg);
  });
});

describe("validateHero", () => {
  it("takes a colour only as #rrggbb", () => {
    const result = validateHero({
      name: "x",
      skin: `#ffffff" onload="alert(1)`,
    });
    assert.equal(result.ok, false);
    assert.ok(!result.ok && result.problems.some((p) => p.startsWith("skin:")));
    assert.throws(() => createHero({ name: "x", skin: "red" }), HeroSpecError);
  });

  it("refuses an unknown part or field rather than drawing the default", () => {
    const result = validateHero({ name: "x", hair: "mohawk", hat: "helm" });
    assert.ok(!result.ok);
    assert.ok(result.problems.some((p) => p.startsWith("hair:")));
    assert.ok(result.problems.some((p) => p.startsWith("hat:")));
  });

  it("needs a name, and an object", () => {
    assert.equal(validateHero({ name: "  " }).ok, false);
    assert.equal(validateHero(null).ok, false);
    assert.equal(validateHero(["name"]).ok, false);
  });

  it("refuses an id prefix that is not a plain name", () => {
    assert.throws(() =>
      renderToken({ name: "x" }, { idPrefix: `a" onload="x` }),
    );
  });
});
