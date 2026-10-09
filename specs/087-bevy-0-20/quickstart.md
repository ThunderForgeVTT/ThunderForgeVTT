# Quickstart: Bevy 0.20

## Set up the worktree

```sh
git worktree add ../ThunderForgeVTT-087 -b 087-bevy-0-20 main
cd ../ThunderForgeVTT-087
```

The worktree's web app loads the engine from **main's** `dist/engine`.
After each engine build here, copy it across:

```sh
ENGINE_PROFILE=release node scripts/build.mjs --only-wasm   # or dev for speed
cp -r dist/engine/. ../ThunderForgeVTT/dist/engine/
```

e2e runs share the lock with main. Wait for it and never bypass it. When
087 is done or set aside, rebuild main's own engine.

## Take the baseline (on 0.19.1, before any change)

1. **Frame rate.** With a release engine copied across, run
   `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1 pnpm e2e:engine-limits` three
   times. Record each `[engine] tokens=… fps=…` line, and the median per
   level, in research.md under **Baseline**.
2. **Size.** Measure the release wasm, raw and brotli:

   ```sh
   node -e 'const f=require("fs"),z=require("zlib");const p=process.argv[1];const b=f.readFileSync(p);console.log(b.length, z.brotliCompressSync(b,{params:{[z.constants.BROTLI_PARAM_QUALITY]:11}}).length)' dist/engine/<name>_bg.wasm
   ```

3. **Render probe.** Open the 3200-token scene that engine-limits makes,
   call `set_render_probe` with `true`, and copy three probe lines.
4. **Pictures.** Capture each of these:
   - a lit scene with darkness and walls;
   - three tokens stacked on one square, with the top one selected;
   - nameplates and a dice readout;
   - a sprite of a known colour.

## Check the upgrade

1. **It builds.** Then check for duplicates:

   ```sh
   cargo build -p thunderforge-engine
   cargo tree -d | grep -E "^(glam|wgpu) "
   make lint
   ```

   The `cargo tree` line must print nothing.

2. **The units.** Run each of these:

   ```sh
   cargo test -p thunderforge-canvas-core
   cargo test -p thunderforge-engine
   RUST_MIN_STACK=16777216 cargo test -p thunderforge-server
   cargo test -p thunderforge-combat
   pnpm -F @thunderforge/web test
   ```

   `cargo test -p thunderforge-server` needs the `thunderforge-canvas-assets`
   RustFS bucket.

3. **The board looks the same.** Take the four captures again, and
   compare each with its baseline:
   - **Darkness.** Same falloff, same shadows.
   - **The stack.** Same order. Clicking it picks the token drawn on top.
     The handles are above the token.
   - **Text.** Same size and place.
   - **The sprite.** Same pixel, within 1/255 on each channel.

4. **The probe.** On the same scene, compare its lines with the baseline
   (contracts/render-probe.md).

5. **Frame rate and size.** Repeat steps 1 and 2 of the baseline on 0.20.
   Record both sets of numbers side by side, with the difference.

6. **The slices.** Run each slice listed in spec.md **Proof**. Then run
   `pnpm e2e:which --diff`, and run each slice it names other than the full
   suite.
