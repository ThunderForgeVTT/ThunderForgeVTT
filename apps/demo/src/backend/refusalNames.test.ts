/**
 * A refusal names the part of ThunderForge the visitor reached for, never
 * the GraphQL field that asked.
 */
import { describe, expect, it } from "vitest";
import { refusalArea } from "./refusalNames";
import { refusingXmlHttpRequest } from "../guard/xhr";

describe("a refused area's name", () => {
  it("is words for a field, and leaves words alone", () => {
    expect(refusalArea("generateInviteCode")).toBe("Invite links");
    expect(refusalArea("deleteWorld")).toBe("Deleting a world");
    expect(refusalArea("beginLoreRepositoryConnection")).toBe(
      "Syncing lore with a repository",
    );
    expect(refusalArea("startCombatEncounter")).toBe("Combat");
    expect(refusalArea("somethingNobodyNamed")).toBe("That feature");
    expect(refusalArea("Importing a book")).toBe("Importing a book");
  });

  it("never echoes a field name back", () => {
    for (const field of [
      "generateInviteCode",
      "worldBookList",
      "uploadLoreImage",
      "zzzUnknownField",
    ]) {
      expect(refusalArea(field)).not.toContain(field);
    }
  });
});

describe("the book importer's upload", () => {
  it("is answered as a refused request, not thrown at", async () => {
    let told = 0;
    const Xhr = refusingXmlHttpRequest(
      () =>
        JSON.stringify({
          errors: [{ message: "Importing a book is not part of the demo." }],
        }),
      () => {
        told += 1;
      },
    );
    const request = new Xhr();
    request.open("POST", "/api/graphql");
    request.setRequestHeader("Content-Type", "application/json");
    const loaded = new Promise<void>((resolve) => {
      request.onload = () => resolve();
    });
    request.send("{}");
    await loaded;
    expect(request.status).toBe(404);
    expect(JSON.parse(request.responseText).errors[0].message).toBe(
      "Importing a book is not part of the demo.",
    );
    expect(told).toBe(1);
  });
});
