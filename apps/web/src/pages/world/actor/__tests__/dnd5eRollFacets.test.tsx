import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import RollFacetsSection from "../../../../../../../packs/systems/dnd5e/web/src/components/RollFacetsSection.tsx";
import {
  calculateLuckPointsLeft,
  sheetFacets,
  toggleFacet,
} from "../../../../../../../packs/systems/dnd5e/web/src/derived-data.ts";

/**
 * Spec 084 US2: the 5e sheet's roll facets. What the sheet records, and what
 * it shows; the server reads the record when it rolls.
 */

const noop = () => {};

function render(facets: string[], canEdit = true, luckPointsLeft = 3) {
  return renderToStaticMarkup(
    <RollFacetsSection
      facets={facets}
      luckPointsLeft={luckPointsLeft}
      canEdit={canEdit}
      disabled={false}
      onToggle={noop}
      onResetLuck={noop}
    />,
  );
}

describe("the 5e sheet's roll facets", () => {
  it("offers the three facets, ticked as the sheet records them", () => {
    const html = render(["halfling_luck"]);
    for (const id of ["halfling_luck", "great_weapon_fighting", "lucky"]) {
      expect(html).toContain(`data-testid="roll-facet-${id}"`);
    }
    expect(html).toMatch(
      /<input[^>]*checked=""[^>]*data-testid="roll-facet-halfling_luck"|<input[^>]*data-testid="roll-facet-halfling_luck"[^>]*checked=""/,
    );
    expect(html).not.toMatch(
      /<input[^>]*data-testid="roll-facet-lucky"[^>]*checked=""/,
    );
  });

  it("shows the Luck Points left and a Reset only when Lucky is ticked", () => {
    expect(render(["halfling_luck"])).not.toContain("dnd5e-luck-points");
    const lucky = render(["lucky"], true, 2);
    expect(lucky).toContain("Luck Points left: 2");
    expect(lucky).toContain('data-testid="dnd5e-luck-reset"');
  });

  it("is read-only for someone who may not edit the sheet", () => {
    const html = render(["lucky"], false);
    expect(html).not.toContain("<input");
    expect(html).not.toContain("dnd5e-luck-reset");
    expect(html).toContain("Lucky: yes");
    expect(html).toContain("dnd5e-luck-points");
  });

  it("ticks and unticks a facet, and reads only known ids once", () => {
    expect(toggleFacet([], "lucky")).toEqual(["lucky"]);
    expect(toggleFacet(["lucky", "halfling_luck"], "lucky")).toEqual([
      "halfling_luck",
    ]);
    expect(sheetFacets(["lucky", "advantage", "lucky", 3])).toEqual(["lucky"]);
    expect(sheetFacets(undefined)).toEqual([]);
  });

  it("counts Luck Points left from the proficiency bonus", () => {
    expect(calculateLuckPointsLeft(3, 0)).toBe(3);
    expect(calculateLuckPointsLeft(3, 2)).toBe(1);
    expect(calculateLuckPointsLeft(2, 5)).toBe(0);
  });
});
