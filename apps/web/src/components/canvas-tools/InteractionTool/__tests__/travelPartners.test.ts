import { describe, expect, it, vi } from "vitest";
import type { Interactive } from "@/api/interactives";
import type { SceneLevel } from "@/api/levels";
import {
  TRAVEL_EFFECT_ID,
  linkBack,
  partnerOf,
  travelChoices,
} from "../travelPartners";

/**
 * Pointing a transition at its other end, and back.
 *
 * "Link both ways" is a second write to a second interactive. What is pinned
 * here is when it is made, and above all when it is *not*: a partner that
 * already does something else is somebody's work.
 */

function interactive(
  over: Partial<Interactive> & { interactiveId: string },
): Interactive {
  return {
    sceneId: "scene-1",
    levelId: "tavern",
    subjectKind: "region",
    subjectRef: null,
    geometry: null,
    trigger: "enter",
    effectId: null,
    effectConfig: null,
    activation: "anyone",
    fireMode: "always",
    firedAt: null,
    available: true,
    canActivate: true,
    ...over,
  };
}

function level(levelId: string, name: string): SceneLevel {
  return {
    levelId,
    sceneId: "scene-1",
    name,
    sortOrder: 0,
    isEntry: false,
    hidden: false,
    backgroundAssetId: null,
    backgroundUrl: null,
    width: 1000,
    height: 1000,
    ambientLight: "bright",
    tokenCount: 0,
  };
}

const travelTo = (partner: string) => ({
  effectId: TRAVEL_EFFECT_ID,
  effectConfig: { partner },
});

describe("travelChoices", () => {
  it("lists every level's interactives under that level's name", () => {
    const choices = travelChoices(
      [
        {
          level: level("tavern", "Tavern"),
          interactives: [interactive({ interactiveId: "aaaaaaaa-1" })],
        },
        {
          level: level("upstairs", "Upstairs"),
          interactives: [
            interactive({ interactiveId: "bbbbbbbb-2", subjectKind: "door" }),
          ],
        },
      ],
      null,
    );

    expect(choices).toEqual([
      { id: "aaaaaaaa-1", label: "Area aaaaaaaa", group: "Tavern" },
      { id: "bbbbbbbb-2", label: "Door bbbbbbbb", group: "Upstairs" },
    ]);
  });

  it("never offers an interactive as its own partner", () => {
    const choices = travelChoices(
      [
        {
          level: level("tavern", "Tavern"),
          interactives: [
            interactive({ interactiveId: "self" }),
            interactive({ interactiveId: "other" }),
          ],
        },
      ],
      "self",
    );
    expect(choices.map((choice) => choice.id)).toEqual(["other"]);
  });

  it("lists bare when no level list was read", () => {
    const [choice] = travelChoices(
      [{ level: null, interactives: [interactive({ interactiveId: "x" })] }],
      null,
    );
    expect(choice).toEqual({ id: "x", label: "Area x" });
  });
});

describe("partnerOf", () => {
  it("is the partner of a transition, and nothing for anything else", () => {
    expect(
      partnerOf(interactive({ interactiveId: "a", ...travelTo("b") })),
    ).toBe("b");
    expect(partnerOf(interactive({ interactiveId: "a" }))).toBeNull();
    expect(
      partnerOf(
        interactive({
          interactiveId: "a",
          effectId: "lore.open",
          effectConfig: { partner: "b" },
        }),
      ),
    ).toBeNull();
  });
});

describe("linkBack", () => {
  it("points the partner back at what was saved", async () => {
    const updateInteractive = vi.fn().mockResolvedValue(undefined);
    const saved = interactive({ interactiveId: "down", ...travelTo("up") });

    const outcome = await linkBack({ updateInteractive }, saved, [
      saved,
      interactive({ interactiveId: "up", levelId: "upstairs" }),
    ]);

    expect(outcome).toEqual({ kind: "linked" });
    expect(updateInteractive).toHaveBeenCalledWith("up", {
      effectId: TRAVEL_EFFECT_ID,
      effectConfig: { partner: "down" },
    });
  });

  it("does nothing for an interactive that is not a transition", async () => {
    const updateInteractive = vi.fn();
    const outcome = await linkBack(
      { updateInteractive },
      interactive({ interactiveId: "book", effectId: "lore.open" }),
      [],
    );
    expect(outcome).toEqual({ kind: "nothing" });
    expect(updateInteractive).not.toHaveBeenCalled();
  });

  it("does nothing when the partner already leads back", async () => {
    const updateInteractive = vi.fn();
    const saved = interactive({ interactiveId: "down", ...travelTo("up") });
    const outcome = await linkBack({ updateInteractive }, saved, [
      interactive({ interactiveId: "up", ...travelTo("down") }),
    ]);
    expect(outcome).toEqual({ kind: "nothing" });
    expect(updateInteractive).not.toHaveBeenCalled();
  });

  it("re-points a partner that is a transition to somewhere else", async () => {
    const updateInteractive = vi.fn().mockResolvedValue(undefined);
    const saved = interactive({ interactiveId: "down", ...travelTo("up") });
    const outcome = await linkBack({ updateInteractive }, saved, [
      interactive({ interactiveId: "up", ...travelTo("cellar") }),
    ]);
    expect(outcome).toEqual({ kind: "linked" });
  });

  it("refuses to overwrite a partner that already does something else", async () => {
    const updateInteractive = vi.fn();
    const saved = interactive({ interactiveId: "down", ...travelTo("lever") });
    const outcome = await linkBack({ updateInteractive }, saved, [
      interactive({ interactiveId: "lever", effectId: "door.toggle" }),
    ]);
    expect(outcome.kind).toBe("refused");
    expect(updateInteractive).not.toHaveBeenCalled();
  });

  it("says so when the server refuses the second write", async () => {
    const updateInteractive = vi.fn().mockRejectedValue(new Error("no"));
    const saved = interactive({ interactiveId: "down", ...travelTo("up") });
    const outcome = await linkBack({ updateInteractive }, saved, [
      interactive({ interactiveId: "up" }),
    ]);
    expect(outcome.kind).toBe("refused");
  });
});
