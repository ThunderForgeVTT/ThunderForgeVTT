/**
 * The page's `<img>` tags, for the one address they would otherwise send
 * past the guard.
 *
 * An `<img src>` is fetched by the browser, not by `window.fetch`, so the
 * actor pictures the demo hands out (`/api/actor-assets/…`, the same address
 * the server gives) would go to whatever is serving the page. Before such an
 * address reaches an image element it is swapped for the drawing itself, as
 * an SVG `data:` URL: the roster, the sheets and the token panels show the
 * same face the engine draws. Any other address is left exactly as given.
 */
import { demoState } from "../backend/state";
import { drawArt, readArtPath } from "../seed/art";

/** The drawing an image address stands for, or `null` to leave it alone. */
export function inlineArt(value: string): string | null {
  let url: URL;
  try {
    url = new URL(value, window.location.href);
  } catch {
    return null;
  }
  if (url.origin !== window.location.origin) return null;
  const art = readArtPath(url.pathname);
  if (!art || art.meta) return null;
  let held;
  try {
    held = demoState().art[art.assetId];
  } catch {
    // Not loaded yet: nothing has been handed out to point at, then.
    return null;
  }
  if (!held) return null;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(drawArt(held))}`;
}

export function installImageGuard(): void {
  const setAttribute = Element.prototype.setAttribute;
  Element.prototype.setAttribute = function (name: string, value: string) {
    if (this instanceof HTMLImageElement && name.toLowerCase() === "src") {
      value = inlineArt(String(value)) ?? value;
    }
    setAttribute.call(this, name, value);
  };

  const src = Object.getOwnPropertyDescriptor(
    HTMLImageElement.prototype,
    "src",
  );
  if (src?.set && src.get) {
    const set = src.set;
    Object.defineProperty(HTMLImageElement.prototype, "src", {
      ...src,
      set(this: HTMLImageElement, value: string) {
        set.call(this, inlineArt(String(value)) ?? value);
      },
    });
  }
}
