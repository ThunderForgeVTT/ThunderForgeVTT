import { describe, expect, it } from "vitest";

import { heroControlsFor } from "../heroControls";

describe("what a Players card offers for its hero", () => {
  it("offers the holder the sheet and the builder", () => {
    expect(
      heroControlsFor({
        myPermissionLevel: "EDITOR",
        myMayChangeImagery: true,
      }),
    ).toEqual({ sheet: true, builder: true });
    expect(
      heroControlsFor({ myPermissionLevel: "OWNER", myMayChangeImagery: true }),
    ).toEqual({ sheet: true, builder: true });
  });

  it("offers a viewer nothing", () => {
    expect(
      heroControlsFor({
        myPermissionLevel: "VIEWER",
        myMayChangeImagery: false,
      }),
    ).toEqual({ sheet: false, builder: false });
  });

  it("keeps the sheet when the world holds player art back", () => {
    expect(
      heroControlsFor({
        myPermissionLevel: "EDITOR",
        myMayChangeImagery: false,
      }),
    ).toEqual({ sheet: true, builder: false });
  });

  it("offers nothing for an actor this viewer was not sent", () => {
    expect(heroControlsFor(undefined)).toEqual({
      sheet: false,
      builder: false,
    });
  });
});
