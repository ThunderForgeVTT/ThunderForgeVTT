/**
 * The cast's faces: every token placed for a member of the cast carries art
 * the engine can decode, and the guard answers that address with PNG bytes.
 *
 * The rasteriser is the browser's canvas, which Node does not have, so the
 * guard is handed a stand-in here; the demo's e2e asks the real page for the
 * same address and reads real PNG bytes back.
 */
import { beforeAll, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  // `state.ts` keeps the world in `window.localStorage`; this one is empty.
  Object.assign(globalThis, {
    window: {
      addEventListener() {},
      localStorage: {
        getItem: () => null,
        setItem() {},
        removeItem() {},
      },
      location: {
        href: "http://demo.test/demo/",
        origin: "http://demo.test",
      },
    },
  });
});

import { demoState, loadState, type Row } from "../backend/state";
import { inlineArt } from "../guard/images";
import { answerRest } from "../guard/rest";
import { DEMO_SCENES, buildSeed, type MapListing } from "./world";

const BASE = "/demo/";
const PNG_SIGNATURE = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

const maps: MapListing[] = DEMO_SCENES.map((scene) => ({
  name: scene.map,
  width: 4000,
  height: 3000,
  gridSize: 100,
  ambientLight: "#ffffff",
  hasPreview: false,
  byteSize: 1,
  walls: [],
  lights: [],
}));

const fetchStatic = (async () =>
  new Response(JSON.stringify(maps))) as unknown as typeof fetch;

/** Stands in for the canvas: records what it drew, answers a PNG header. */
const drawn: string[] = [];
const drawPng = async (svg: string): Promise<Blob> => {
  drawn.push(svg);
  return new Blob([new Uint8Array(PNG_SIGNATURE)], { type: "image/png" });
};

function ask(path: string, method = "GET"): Promise<Response> {
  const answer = answerRest(method, path, BASE, fetchStatic, drawPng, "gm");
  if (!answer) throw new Error(`the guard does not answer ${path}`);
  return Promise.resolve(answer);
}

beforeAll(async () => {
  await loadState(fetchStatic, BASE);
});

describe("the cast's token art", () => {
  it("gives every token placed for the cast a PNG the guard serves", async () => {
    const state = demoState();
    const cast = state.tokens.filter((token) => token.actorId);
    // Two heroes, three goblins, a hobgoblin and a wolf.
    expect(cast).toHaveLength(7);
    for (const token of cast) {
      expect(token.photoUrl).toMatch(
        /^\/api\/actor-assets\/[0-9a-f-]{36}\.png$/,
      );
      const answer = await ask(token.photoUrl as string);
      expect(answer.status).toBe(200);
      expect(answer.headers.get("content-type")).toBe("image/png");
      const bytes = new Uint8Array(await answer.arrayBuffer());
      expect([...bytes.slice(0, 8)]).toEqual(PNG_SIGNATURE);
    }
  });

  it("draws each token from its actor's own spec", async () => {
    const state = demoState();
    const label = (token: Row) =>
      state.actors.find((actor) => actor.id === token.actorId)?.label;
    for (const token of state.tokens.filter((t) => t.actorId)) {
      drawn.length = 0;
      await ask(token.photoUrl as string);
      expect(drawn).toHaveLength(1);
      expect(drawn[0]).toMatch(/^<svg /);
      expect(drawn[0]).toContain(`aria-label="${label(token)}"`);
    }
    const urls = new Set(state.tokens.map((t) => t.photoUrl).filter(Boolean));
    // One picture per actor: the goblins share theirs.
    expect(urls.size).toBe(5);
  });

  it("has no .meta for Bevy, and nothing for an id it never gave out", async () => {
    const [token] = demoState().tokens.filter((t) => t.actorId);
    expect((await ask(`${token.photoUrl}.meta`)).status).toBe(404);
    expect(
      (await ask("/api/actor-assets/d0000000-0000-4000-00ff-000000000001.png"))
        .status,
    ).toBe(404);
  });

  it("fills each actor's token and portrait, drawn inline for <img> tags", () => {
    for (const actor of demoState().actors) {
      const images = actor.images as Row[];
      expect(images.map((image) => image.role).sort()).toEqual([
        "portrait",
        "token",
      ]);
      for (const image of images) {
        for (const url of [image.url, image.thumbnailUrl] as string[]) {
          expect(inlineArt(url)).toMatch(/^data:image\/svg\+xml;/);
        }
      }
    }
    expect(inlineArt("/demo/maps/demo.webp")).toBeNull();
  });

  it("draws the same faces on every load", () => {
    const again = buildSeed(maps, BASE);
    expect(again.art).toEqual(demoState().art);
  });
});
