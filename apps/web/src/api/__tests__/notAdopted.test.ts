import { describe, expect, it } from "vitest";
import { GraphQLRequestError } from "@/api/graphqlClient";
import { NOT_ADOPTED, playRefusalText } from "@/api/notAdopted";

describe("playRefusalText", () => {
  it("says why when the server refused staged content", () => {
    const err = new GraphQLRequestError("first; second", {
      codes: ["CONTENT_NOT_ADOPTED"],
    });
    expect(playRefusalText(err, "The attack failed")).toBe(NOT_ADOPTED);
  });

  it("keeps any other refusal's own words", () => {
    const err = new GraphQLRequestError("Out of range", {
      codes: ["FORBIDDEN"],
    });
    expect(playRefusalText(err, "The attack failed")).toBe("Out of range");
    expect(playRefusalText("nope", "The attack failed")).toBe(
      "The attack failed",
    );
  });
});
