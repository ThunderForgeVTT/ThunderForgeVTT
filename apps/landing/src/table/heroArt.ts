// A hero drawn by `@thunderforge/heroes` is an SVG; the engine wants pixels.
// The browser does the rasterizing, and the PNG bytes go to the engine as
// base64 in `upsert_token`'s `photoData`.

export function svgUrl(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

/** The SVG at `px` square, as base64 PNG bytes (no `data:` prefix). */
export async function rasterize(svg: string, px: number): Promise<string> {
  const img = new Image();
  img.decoding = "async";
  img.src = svgUrl(svg);
  await img.decode();
  const canvas = document.createElement("canvas");
  canvas.width = px;
  canvas.height = px;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("no 2d canvas");
  ctx.drawImage(img, 0, 0, px, px);
  const url = canvas.toDataURL("image/png");
  return url.slice(url.indexOf(",") + 1);
}
