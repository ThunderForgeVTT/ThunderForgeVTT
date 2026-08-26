/**
 * One browser configuration for every sandbox harness.
 *
 * Plain `chromium.launch()` gets no GPU: Chromium falls back to software
 * rasterization and the engine runs at about 4fps with two sprites on
 * screen — measured, not assumed (`engine_stats()` reported 240.9ms frames
 * and 4.15fps). Every timing taken that way is a measurement of SwiftShader,
 * and a decode or upload hitch is invisible underneath a 240ms baseline.
 *
 * With these flags the same page runs at 16.6ms and 59.5fps, which is the
 * machine the app actually ships to.
 *
 * `channel: "chrome"` selects the full browser rather than the headless
 * shell, which is what makes the GPU path available at all.
 */
export const BROWSER_OPTIONS = {
  channel: "chrome",
  headless: true,
  args: [
    "--enable-gpu",
    "--enable-gpu-rasterization",
    // ANGLE over Vulkan is the combination that works headless on Linux.
    "--use-gl=angle",
    "--use-angle=vulkan",
    // VMs and older drivers are blocklisted by default; without this the
    // flags above are accepted and then quietly ignored.
    "--ignore-gpu-blocklist",
  ],
};

/**
 * Launches a GPU-accelerated browser, preferring full Chrome and falling
 * back to Playwright's bundled Chromium with the same flags.
 *
 * The channel is a preference, not a requirement: `channel: "chrome"` fails
 * outright when Chrome is not installed, and the bundled Chromium reaches
 * the same 59.5fps with these args. Reports which one it got, because a
 * silent fallback to software rendering is exactly the failure this module
 * exists to prevent.
 */
export async function launchGpuBrowser(chromium) {
  try {
    const browser = await chromium.launch(BROWSER_OPTIONS);
    return { browser, channel: "chrome" };
  } catch (error) {
    if (!/is not found at|Executable doesn't exist/.test(String(error))) {
      throw error;
    }
    const { channel: _channel, ...bundled } = BROWSER_OPTIONS;
    return { browser: await chromium.launch(bundled), channel: "bundled chromium" };
  }
}

/**
 * Fails loudly if the engine is not actually running on the GPU.
 *
 * The flags above are silently ignored on a machine whose driver Chromium
 * declines to use, and the harness would then report SwiftShader timings as
 * if they were real. Software rasterization is not marginal here — it is
 * ~240ms a frame against ~16ms — so any threshold in between separates them
 * cleanly.
 */
export async function assertGpuRendering(page, label = "harness") {
  const stats = await page.evaluate(() => window.__stress.engineStats());
  if (stats.frame_time_ms > 60) {
    throw new Error(
      `${label}: engine is at ${stats.frame_time_ms.toFixed(1)}ms/frame ` +
        `(${stats.fps.toFixed(1)}fps) — that is software rasterization, not the GPU. ` +
        `Timings taken now would be meaningless.`,
    );
  }
  return stats;
}
