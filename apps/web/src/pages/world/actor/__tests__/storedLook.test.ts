import { describe, expect, it } from "vitest";
import { HERO_PARTS } from "@thunderforge/heroes";
import { openingLook } from "@/pages/world/actor/storedLook";

const NAME = { name: "Mirelda" };
const [firstHair, otherHair] = HERO_PARTS.hair;
const built = { name: "Mirelda", hair: otherHair };

describe("which look the builder opens on (spec 044 FR-036, FR-037)", () => {
  it("opens on the name when nothing was built", () => {
    expect(openingLook([], NAME)).toEqual({
      source: "name",
      valid: true,
      spec: NAME,
      note: null,
    });
    const files = [
      { role: "portrait", heroSpec: null },
      { role: "token", heroSpec: null },
    ];
    expect(openingLook(files, NAME).source).toBe("name");
  });

  it("prefers the portrait's spec and says nothing when both agree", () => {
    // JSONB hands keys back in its own order; that is not a difference.
    const reordered = { hair: otherHair, name: "Mirelda" };
    const look = openingLook(
      [
        { role: "token", heroSpec: reordered },
        { role: "portrait", heroSpec: built },
      ],
      NAME,
    );
    expect(look).toMatchObject({ source: "portrait", valid: true, note: null });
    expect(look.spec).toEqual(built);
  });

  it("says so when the roles differ, or one of them is not built", () => {
    const other = { name: "Mirelda", hair: firstHair };
    expect(
      openingLook(
        [
          { role: "portrait", heroSpec: built },
          { role: "token", heroSpec: other },
        ],
        NAME,
      ).note,
    ).toMatch(/different heroes/);
    expect(
      openingLook(
        [
          { role: "portrait", heroSpec: built },
          { role: "token", heroSpec: null },
        ],
        NAME,
      ).note,
    ).toMatch(/token is no longer a built hero/);
    const tokenOnly = openingLook(
      [
        { role: "portrait", heroSpec: null },
        { role: "token", heroSpec: built },
      ],
      NAME,
    );
    expect(tokenOnly.source).toBe("token");
    expect(tokenOnly.note).toMatch(/portrait is no longer a built hero/);
    expect(
      openingLook([{ role: "portrait", heroSpec: built }], NAME).note,
    ).toMatch(/Only the portrait was built/);
  });

  it("reports a stored spec that no longer validates instead of drawing it", () => {
    const lost = { name: "Mirelda", hair: `${firstHair}-that-was-removed` };
    const look = openingLook([{ role: "portrait", heroSpec: lost }], NAME);
    expect(look.valid).toBe(false);
    if (look.valid) return;
    expect(look.spec).toBe(lost);
    expect(look.problems.some((line) => line.startsWith("hair: "))).toBe(true);
  });
});
