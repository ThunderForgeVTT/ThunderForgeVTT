/**
 * A built hero's drawing as the file `uploadActorImage` takes (spec 044
 * FR-024, contract B4).
 *
 * The SVG goes up as it is. The server already rasterises an SVG upload to
 * WebP (`storage/svg.rs`), and that is the only road by which a built hero
 * becomes stored pixels — no canvas in the browser, no second endpoint.
 */
export function heroSvgFile(svg: string, role: string): File {
  return new File([svg], `hero-${role}.svg`, { type: "image/svg+xml" });
}
