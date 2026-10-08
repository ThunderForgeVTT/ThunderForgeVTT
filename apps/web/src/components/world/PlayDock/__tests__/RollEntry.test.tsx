import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import type { WorldRollRecord } from "@/types/roll";

import { RollEntry } from "../RollEntry";

/**
 * Spec 084 FR-016: a roll in the feed names the facets that shaped it. Read
 * from server-rendered markup, as `apps/web`'s unit tests have no DOM.
 */
function roll(facets: WorldRollRecord["facets"]): WorldRollRecord {
  return {
    __typename: "WorldRoll",
    id: "roll-1",
    rollerId: "user-1",
    rollerName: "Pip",
    label: "Stealth",
    formula: "2d20kh1 + 5",
    bindings: [],
    resolution: {
      formula: "2d20kh1 + 5",
      dice: [
        {
          sidesKind: "NUMERIC",
          numericSides: 20,
          rolls: [4],
          kept: false,
          finalValue: 4,
        },
        {
          sidesKind: "NUMERIC",
          numericSides: 20,
          rolls: [17],
          kept: true,
          finalValue: 17,
        },
      ],
      resultKind: "TOTAL",
      resultValue: 22,
    } as unknown as WorldRollRecord["resolution"],
    visibility: "EVERYONE",
    createdAt: "2026-10-08T00:00:00Z",
    revealedAt: null,
    revealedByName: null,
    facets,
    rerollOf: null,
    rerolledBy: null,
    spent: null,
    rerollOffers: [],
    rerollUntil: null,
  };
}

function markup(entry: WorldRollRecord) {
  return renderToStaticMarkup(
    <RollEntry worldId="w" entry={entry} isGm={false} onRevealed={() => {}} />,
  );
}

describe("RollEntry facets", () => {
  it("tags a roll with each facet's label", () => {
    const html = markup(roll([{ id: "advantage", label: "Advantage" }]));
    expect(html.match(/data-testid="roll-facet"/g)).toHaveLength(1);
    expect(html).toContain('data-facet-id="advantage"');
    expect(html).toContain(">Advantage<");
  });

  it("draws no tag for a plain roll", () => {
    expect(markup(roll([]))).not.toContain('data-testid="roll-facet"');
  });
});

function oneDie(die: {
  rolls: number[];
  steps?: ("REROLL" | "EXPLODE")[];
  finalValue: number;
}): WorldRollRecord {
  const entry = roll([]);
  entry.resolution = {
    ...entry.resolution,
    dice: [
      {
        sidesKind: "NUMERIC",
        numericSides: 20,
        kept: true,
        steps: [],
        ...die,
      },
    ],
  };
  return entry;
}

function struck(html: string): string[] {
  return [...html.matchAll(/data-testid="roll-die-struck"[^>]*>([^<]*)</g)].map(
    (match) => match[1],
  );
}

describe("RollEntry rerolled dice", () => {
  it("strikes a rerolled 1 beside the value used", () => {
    const html = markup(
      oneDie({ rolls: [1, 14], steps: ["REROLL"], finalValue: 14 }),
    );
    expect(struck(html)).toEqual(["1"]);
    expect(html).toMatch(/roll-die-struck[^>]*>1<\/span><span>14</);
  });

  it("reads a chain stored before spec 083 as rerolls", () => {
    expect(struck(markup(oneDie({ rolls: [1, 2, 9], finalValue: 9 })))).toEqual(
      ["1", "2"],
    );
  });

  it("strikes a value the die did not end on", () => {
    expect(struck(markup(oneDie({ rolls: [1], finalValue: 3 })))).toEqual([
      "1",
    ]);
  });

  it("strikes nothing on a plain die or an explosion", () => {
    expect(struck(markup(oneDie({ rolls: [12], finalValue: 12 })))).toEqual([]);
    expect(
      struck(
        markup(oneDie({ rolls: [6, 4], steps: ["EXPLODE"], finalValue: 10 })),
      ),
    ).toEqual([]);
  });
});

describe("RollEntry rerolls", () => {
  const INSPIRATION = { id: "inspiration", label: "Heroic Inspiration" };

  it("strikes through a roll that was rerolled, and offers it nothing", () => {
    const html = markup({
      ...roll([]),
      rerolledBy: "roll-2",
      rerollOffers: [INSPIRATION],
      rerollUntil: "2999-01-01T00:00:00Z",
    });
    expect(html).toContain('data-testid="roll-rerolled"');
    expect(html).not.toContain("roll-reroll-inspiration");
  });

  it("marks a reroll with what it spent", () => {
    const html = markup({
      ...roll([INSPIRATION]),
      rerollOf: "roll-0",
      spent: INSPIRATION,
    });
    expect(html).toContain('data-testid="roll-spent"');
    expect(html).toContain("Rerolled with Heroic Inspiration");
    expect(html).not.toContain('data-testid="roll-rerolled"');
  });

  it("offers the maker a reroll while the window is open", () => {
    const html = markup({
      ...roll([]),
      rerollOffers: [INSPIRATION],
      rerollUntil: "2999-01-01T00:00:00Z",
    });
    expect(html).toContain('data-testid="roll-reroll-inspiration"');
    expect(html).toContain("Reroll (Heroic Inspiration)");
  });
});
