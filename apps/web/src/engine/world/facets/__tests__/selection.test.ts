import { describe, expect, it } from "vitest";

import { createSelectionFacet } from "../selection";
import { createTokenControlFacet } from "../tokenControl";
import { createLocalAdjudicator } from "../adjudicator";
import { createWorldStore } from "../../store";
import type { FacetContext } from "../types";

/** Two tokens on one spot, as the engine reports a press on them. */
function pile() {
  const store = createWorldStore({
    worldId: "w1",
    initialTokens: [
      { id: "top", x: 0, y: 0, z: 1 },
      { id: "under", x: 0, y: 0, z: 0 },
    ],
  });
  const context: FacetContext = {
    worldId: "w1",
    sceneId: "s1",
    principal: { userId: "u-gm", authority: "gm" },
    adjudicator: createLocalAdjudicator(),
  };
  const selection = createSelectionFacet(
    store,
    context,
    createTokenControlFacet(store, context),
  );
  return { store, selection };
}

const ids = (stack: { members: { token: { id: string } }[] } | null) =>
  stack?.members.map((member) => member.token.id) ?? null;

describe("disambiguate", () => {
  it("offers the pile a click picked up", () => {
    const { store, selection } = pile();
    store.dispatch(
      { type: "select_tokens", tokenIds: ["top", "under"] },
      "bevy",
    );
    expect(ids(selection.disambiguate())).toEqual(["top", "under"]);
  });

  it("still offers the pile after the picker chose one of it", () => {
    const { store, selection } = pile();
    store.dispatch(
      { type: "select_tokens", tokenIds: ["top", "under"] },
      "bevy",
    );
    selection.selectOne("under");
    expect(selection.selectedIds()).toEqual(["under"]);
    expect(ids(selection.disambiguate())).toEqual(["top", "under"]);

    // The press before the next double-click selects the chosen token alone,
    // and reports the pile it pressed on.
    store.dispatch(
      {
        type: "select_tokens",
        tokenIds: ["under"],
        stackIds: ["top", "under"],
      },
      "bevy",
    );
    expect(selection.selectedIds()).toEqual(["under"]);
    expect(ids(selection.disambiguate())).toEqual(["top", "under"]);
  });

  it("offers nothing for a lone token or an empty board", () => {
    const { store, selection } = pile();
    store.dispatch({ type: "select_tokens", tokenIds: ["top"] }, "bevy");
    expect(selection.disambiguate()).toBeNull();
    store.dispatch({ type: "select_token", tokenId: "under" }, "ui");
    expect(selection.disambiguate()).toBeNull();
    selection.clear();
    expect(selection.disambiguate()).toBeNull();
  });

  it("drops a removed token from the pile", () => {
    const { store, selection } = pile();
    store.dispatch(
      { type: "select_tokens", tokenIds: ["top", "under"] },
      "bevy",
    );
    store.dispatch({ type: "remove_token", tokenId: "under" }, "sync");
    expect(selection.disambiguate()).toBeNull();
  });
});
