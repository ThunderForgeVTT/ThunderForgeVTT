import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { EditorLoading } from "../EditorLoading";
import { NotLoaded } from "../LazyBoundary";
import { isChunkLoadError } from "../chunkLoadError";

describe("isChunkLoadError", () => {
  it("knows each browser's wording for a chunk that is gone", () => {
    for (const message of [
      "Failed to fetch dynamically imported module: https://x/assets/chunks/Editor-abc.js",
      "error loading dynamically imported module",
      "Importing a module script failed.",
      "Unable to preload CSS for /assets/static/Editor-abc.css",
    ]) {
      expect(isChunkLoadError(new TypeError(message))).toBe(true);
    }
  });

  it("does not take a bug for a failed download", () => {
    expect(isChunkLoadError(new TypeError("x is not a function"))).toBe(false);
    expect(
      isChunkLoadError("Failed to fetch dynamically imported module"),
    ).toBe(false);
    expect(isChunkLoadError(null)).toBe(false);
  });
});

describe("NotLoaded", () => {
  it("says what is missing and offers a reload", () => {
    const markup = renderToStaticMarkup(<NotLoaded what="The editor" />);
    expect(markup).toContain('role="alert"');
    expect(markup).toContain("The editor did not load.");
    expect(markup).toContain("Reload");
  });
});

describe("EditorLoading", () => {
  it("shows the text, and is nothing anybody can type into", () => {
    const markup = renderToStaticMarkup(<EditorLoading value={"# A crypt"} />);
    expect(markup).toContain("# A crypt");
    expect(markup).toContain('aria-busy="true"');
    expect(markup).not.toMatch(/<textarea|<input|contenteditable/);
  });
});
