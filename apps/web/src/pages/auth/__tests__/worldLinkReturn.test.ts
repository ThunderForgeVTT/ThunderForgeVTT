import { describe, expect, it } from "vitest";
import { isWorldLinkReturn } from "@/pages/auth/LoginView";

describe("isWorldLinkReturn (spec 088, FR-009)", () => {
  it("is a sign-in on the way to a world link", () => {
    expect(isWorldLinkReturn("?returnTo=%2Fjoin%2FABC123")).toBe(true);
  });

  it("is not any other return, or none", () => {
    expect(isWorldLinkReturn("?returnTo=%2Fworlds")).toBe(false);
    expect(isWorldLinkReturn("?returnTo=%2Finvite%2FABC")).toBe(false);
    expect(isWorldLinkReturn("")).toBe(false);
    expect(isWorldLinkReturn("?returnTo=https%3A%2F%2Fevil%2Fjoin%2F")).toBe(
      false,
    );
  });
});
