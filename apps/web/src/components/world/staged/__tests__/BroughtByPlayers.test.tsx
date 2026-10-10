/**
 * Spec 048 T060: the queue a Game Master or Trusted Player decides from.
 *
 * Rendered to markup, as elsewhere in `apps/web`, which has no jsdom. What
 * matters is what is on the screen: one heading per player, the actions a
 * piece's state allows and no others, and the note when two players brought
 * different pieces of the same name.
 */
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { StagedPiece } from "@/api/stagedContent";
import { BroughtByPlayersView } from "../BroughtByPlayers";
import { differsFromPlayer, groupByPlayer } from "../grouping";

const ana = { id: "u-ana", username: "ana", displayName: "Ana" };
const bo = { id: "u-bo", username: "bo", displayName: "Bo" };

function piece(over: Partial<StagedPiece>): StagedPiece {
  return {
    id: "s-1",
    kind: "feat",
    name: "Alert",
    fieldValues: {},
    state: "PENDING",
    broughtBy: ana,
    actors: [{ id: "a-1", label: "Wren" }],
    differsFrom: null,
    decidedBy: null,
    decidedAt: null,
    adoptedAbilityId: null,
    adoptedItemId: null,
    ...over,
  };
}

const noop = () => {};
function render(pieces: StagedPiece[]) {
  return renderToStaticMarkup(
    <BroughtByPlayersView
      pieces={pieces}
      busy={false}
      error={null}
      onAdopt={noop}
      onAdoptAll={noop}
      onDecline={noop}
      onRevisit={noop}
    />,
  );
}

describe("groupByPlayer", () => {
  it("groups by who brought it, pending first, then by name", () => {
    const groups = groupByPlayer([
      piece({ id: "1", name: "Tough", state: "DECLINED" }),
      piece({ id: "2", name: "Lucky", broughtBy: bo }),
      piece({ id: "3", name: "Sentinel" }),
      piece({ id: "4", name: "Alert" }),
    ]);
    expect(groups.map((group) => group.player.id)).toEqual(["u-ana", "u-bo"]);
    expect(groups[0].pieces.map((p) => p.name)).toEqual([
      "Alert",
      "Sentinel",
      "Tough",
    ]);
    expect(groups[0].pending).toBe(2);
    expect(groups[1].pending).toBe(1);
  });
});

describe("differsFromPlayer", () => {
  it("names the other player, or says another player when not listed", () => {
    const theirs = piece({ id: "b", broughtBy: bo });
    const mine = piece({ id: "a", differsFrom: "b" });
    expect(differsFromPlayer(mine, [mine, theirs])).toBe("Bo");
    expect(differsFromPlayer(mine, [mine])).toBe("another player");
    expect(differsFromPlayer(theirs, [mine, theirs])).toBeNull();
  });
});

describe("BroughtByPlayersView", () => {
  it("draws a heading per player and Adopt all only past one pending", () => {
    const html = render([
      piece({ id: "1", name: "Alert" }),
      piece({ id: "2", name: "Sentinel" }),
      piece({ id: "3", name: "Lucky", broughtBy: bo }),
    ]);
    expect(html).toContain(">Ana</h3>");
    expect(html).toContain(">Bo</h3>");
    expect(html).toContain("Adopt all from Ana");
    expect(html).not.toContain("Adopt all from Bo");
  });

  it("offers each state only its own actions", () => {
    const html = render([
      piece({ id: "p", name: "Alert" }),
      piece({ id: "d", name: "Tough", state: "DECLINED" }),
      piece({ id: "a", name: "Lucky", state: "ADOPTED" }),
    ]);
    expect(html).toContain('aria-label="Adopt Alert"');
    expect(html).toContain('aria-label="Decline Alert"');
    expect(html).toContain('aria-label="Revisit Tough"');
    expect(html).not.toContain('aria-label="Adopt Tough"');
    expect(html).not.toContain('aria-label="Decline Tough"');
    expect(html).not.toMatch(/aria-label="(Adopt|Decline|Revisit) Lucky"/);
  });

  it("notes where a piece differs from another character's", () => {
    const html = render([
      piece({ id: "a", differsFrom: "b" }),
      piece({ id: "b", broughtBy: bo }),
    ]);
    expect(html).toContain("Differs from the Alert Bo brought.");
    expect(html.match(/data-testid="staged-differs"/g)).toHaveLength(1);
  });

  it("says so when nobody has brought anything", () => {
    expect(render([])).toContain("Nobody has brought anything in yet.");
  });
});
