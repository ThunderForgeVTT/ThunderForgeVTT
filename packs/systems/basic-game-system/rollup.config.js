import resolve from "@rollup/plugin-node-resolve";

/**
 * Minimal build config for the Basic Game System starter pack.
 *
 * Bundles the entry module referenced by system.json's `esmodules` into
 * `dist/`, alongside a plain copy of the stylesheet referenced by
 * `styles`. Kept intentionally small — this pack has no dependencies and
 * no bespoke build steps to add.
 */
export default {
  input: "module/main.mjs",
  output: {
    file: "dist/module/main.mjs",
    format: "es",
    sourcemap: true,
  },
  plugins: [resolve()],
};
