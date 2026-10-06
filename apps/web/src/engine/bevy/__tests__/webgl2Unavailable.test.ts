import { describe, expect, it } from "vitest";
import { webgl2Unavailable } from "../index";

function documentWhoseCanvasGives(context: unknown): Document {
  return {
    createElement: () => ({ getContext: () => context }),
  } as unknown as Document;
}

describe("webgl2Unavailable", () => {
  it("is quiet when the browser hands out a WebGL2 context", () => {
    expect(webgl2Unavailable(documentWhoseCanvasGives({}))).toBeNull();
  });

  it("names the missing WebGL2 when the browser returns null", () => {
    expect(webgl2Unavailable(documentWhoseCanvasGives(null))).toMatch(/WebGL2/);
  });

  it("treats a throwing getContext the same as a refusal", () => {
    const doc = {
      createElement: () => ({
        getContext: () => {
          throw new Error("WebGL is currently disabled");
        },
      }),
    } as unknown as Document;
    expect(webgl2Unavailable(doc)).toMatch(/WebGL2/);
  });
});
