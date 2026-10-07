/**
 * An SVG drawing as PNG bytes, made by the browser's own canvas.
 *
 * The engine decodes PNG and WebP and nothing else, and the demo has no
 * server to turn a hero's SVG into either (the real one does it in
 * `storage/svg.rs`). Each drawing is rasterised once per page and kept in
 * memory; the same SVG is the same picture, so the same bytes.
 */

/** Pixels across: twice the drawing's own size, so a zoomed token stays sharp. */
const PNG_SIZE = 512;

const drawn = new Map<string, Promise<Blob>>();

async function rasterize(svg: string): Promise<Blob> {
  const image = new Image();
  image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
  await image.decode();
  const canvas = document.createElement("canvas");
  canvas.width = PNG_SIZE;
  canvas.height = PNG_SIZE;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("the demo could not draw a token: no canvas");
  context.drawImage(image, 0, 0, PNG_SIZE, PNG_SIZE);
  return new Promise((resolve, reject) =>
    canvas.toBlob(
      (blob) =>
        blob ? resolve(blob) : reject(new Error("the canvas wrote no PNG")),
      "image/png",
    ),
  );
}

export function svgToPng(svg: string): Promise<Blob> {
  let png = drawn.get(svg);
  if (!png) {
    png = rasterize(svg);
    // A failure is not remembered: the next ask tries again.
    png.catch(() => drawn.delete(svg));
    drawn.set(svg, png);
  }
  return png;
}
