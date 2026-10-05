/**
 * Sending the engine a command it must refuse, on purpose.
 *
 * Spec 029 FR-022: the engine reports what it cannot accept rather than
 * ignoring it. That guarantee only matters under conditions the application
 * cannot produce on its own — a bundle older than the engine, a hand-built
 * command, a field that changed shape between deploys. The bridge stamps
 * every real command correctly, so nothing the app does can exercise the
 * refusal path.
 *
 * # Why this is not the escape hatch `probe.ts` warns against
 *
 * `probe.ts` refuses to dispatch because a debugging surface that can mutate
 * state lets a test pass against a situation the app cannot reach. The
 * distinction here is that this does not *simulate* a refusal or stub a
 * result: it hands the real engine a real malformed payload and the engine
 * really refuses it. It is fault injection, not a stub. Nothing it can send
 * enters world state — refusals are routed away from the store by `index.ts`,
 * which is itself part of what wants proving.
 *
 * Development only. `import.meta.env.DEV` is a compile-time constant, so this
 * module is dropped from a production bundle entirely.
 */

type CommandSink = {
  apply_world_command?: (json: string) => void;
  debug_panic?: () => void;
};

/**
 * Hand the engine a raw payload. Answers whether it could be delivered —
 * not whether the engine accepted it, which is the whole question and is
 * answered on the report channel, not here.
 */
export async function injectRawEngineCommand(
  payload: string,
): Promise<boolean> {
  if (!import.meta.env.DEV) return false;
  try {
    const engine = (await import("@thunderforge/engine/engine")) as CommandSink;
    if (!engine.apply_world_command) return false;
    engine.apply_world_command(payload);
    return true;
  } catch {
    return false;
  }
}

/**
 * Make the engine panic (spec 070). Answers whether it could be asked —
 * a release engine exports no such thing — not whether the page noticed,
 * which is what the caller is there to find out.
 *
 * The same kind of thing as the function above: a real panic in the real
 * engine, reported through the real hook.
 */
export async function crashEngine(): Promise<boolean> {
  if (!import.meta.env.DEV) return false;
  let engine: CommandSink;
  try {
    engine = (await import("@thunderforge/engine/engine")) as CommandSink;
  } catch {
    return false;
  }
  if (!engine.debug_panic) return false;
  try {
    engine.debug_panic();
  } catch {
    // The trap, surfacing as a RuntimeError. Expected: it is the crash.
  }
  return true;
}
