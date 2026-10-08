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
