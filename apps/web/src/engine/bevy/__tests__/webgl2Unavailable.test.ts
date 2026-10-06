import { describe, expect, it } from "vitest";
import { boardSizeCeiling, webgl2Unavailable } from "../index";

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

describe("boardSizeCeiling", () => {
  const gl = (maxTextureSize: number) => ({
    MAX_TEXTURE_SIZE: 0x0d33,
    getParameter: (name: number) => (name === 0x0d33 ? maxTextureSize : null),
  });

  it("divides the texture limit by the device pixel ratio, with a margin", () => {
    expect(boardSizeCeiling(documentWhoseCanvasGives(gl(2048)), 1)).toBe(2046);
    expect(boardSizeCeiling(documentWhoseCanvasGives(gl(2048)), 2)).toBe(1022);
    expect(boardSizeCeiling(documentWhoseCanvasGives(gl(16384)), 1.25)).toBe(
      13105,
    );
  });

  it("has no answer without WebGL2", () => {
    expect(boardSizeCeiling(documentWhoseCanvasGives(null), 1)).toBeNull();
  });
});
