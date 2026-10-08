/**
 * Spec 084 T066 (research R6): an item is marked with the properties its
 * system declares, one checkbox each, and `setItemAttack` sends them.
 *
 * `apps/web` has no jsdom, so the checklist is server-rendered and read as
 * markup, as `InstanceSettingsPanel.test.tsx` does.
 */

import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

const posted: { query: string; variables: unknown }[] = [];
vi.mock("@/api/graphqlClient", () => ({
  postGraphQL: async (query: string, variables: unknown) => {
    posted.push({ query, variables });
    return query.includes("systemItemProperties")
      ? { systemItemProperties: FIVE_E }
      : { setItemAttack: true };
  },
}));

const FIVE_E = [
  { id: "heavy", label: "Heavy" },
  { id: "two_handed", label: "Two-Handed" },
  { id: "versatile", label: "Versatile" },
];

import { getSystemItemProperties, setItemAttack } from "@/api/attacks";
import {
  ItemPropertyChecklist,
  toggleProperty,
} from "../ItemPropertyChecklist";

beforeEach(() => {
  posted.length = 0;
});

describe("item properties", () => {
  it("shows a checkbox per declared property, ticked when the item has it", () => {
    const html = renderToStaticMarkup(
      <ItemPropertyChecklist
        declared={FIVE_E}
        selected={["two_handed"]}
        disabled={false}
        onChange={() => {}}
      />,
    );
    for (const { id, label } of FIVE_E) {
      expect(html).toContain(`data-testid="item-property-${id}"`);
      expect(html).toContain(label);
    }
    const ticked = html.match(/<input[^>]*checked=""[^>]*>/g) ?? [];
    expect(ticked).toHaveLength(1);
    expect(ticked[0]).toContain('data-testid="item-property-two_handed"');
  });

  it("shows nothing for a system that declares none", () => {
    const html = renderToStaticMarkup(
      <ItemPropertyChecklist
        declared={[]}
        selected={[]}
        disabled={false}
        onChange={() => {}}
      />,
    );
    expect(html).toBe("");
  });

  it("keeps the system's order and drops what it does not declare", () => {
    expect(toggleProperty(FIVE_E, ["two_handed"], "heavy", true)).toEqual([
      "heavy",
      "two_handed",
    ]);
    expect(
      toggleProperty(FIVE_E, ["heavy", "two_handed"], "heavy", false),
    ).toEqual(["two_handed"]);
    expect(toggleProperty(FIVE_E, ["glowing"], "heavy", true)).toEqual([
      "heavy",
    ]);
  });

  it("reads the declared properties and saves the item's through setItemAttack", async () => {
    expect(await getSystemItemProperties("w1")).toEqual(FIVE_E);
    expect(posted[0].variables).toEqual({ worldId: "w1" });

    await setItemAttack("i1", {
      reach: 5,
      rangeNormal: null,
      rangeLong: null,
      needsLineOfSight: true,
      actionCost: "ACTION",
      legendaryCost: 1,
      multiattack: [],
      properties: ["heavy", "two_handed"],
    });
    expect(posted[1].query).toContain("setItemAttack");
    expect(posted[1].variables).toMatchObject({
      itemId: "i1",
      attack: { properties: ["heavy", "two_handed"] },
    });
  });
});
