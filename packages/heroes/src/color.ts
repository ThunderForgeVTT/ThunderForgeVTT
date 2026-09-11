/** A colour as a hero spec writes it: `#rrggbb`, nothing else. Anything that
 * reaches an SVG attribute passes this first, so a spec from a saved builder
 * session or an import cannot carry markup into the drawing. */
export const HEX_COLOR = /^#[0-9a-fA-F]{6}$/;

/** A darker shade of `hex`, for outlines and shadows inside a part. */
export function shade(hex: string, amount = 0.25): string {
  const n = parseInt(hex.slice(1), 16);
  const r = Math.round(((n >> 16) & 255) * (1 - amount));
  const g = Math.round(((n >> 8) & 255) * (1 - amount));
  const b = Math.round((n & 255) * (1 - amount));
  return `#${((1 << 24) | (r << 16) | (g << 8) | b).toString(16).slice(1)}`;
}
