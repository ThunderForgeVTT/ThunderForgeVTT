/**
 * Basic Game System — minimal entry module.
 *
 * This is a blank-slate starter template pack: it exists so a new system
 * pack can be forked from something real and internally consistent rather
 * than from an empty file. It intentionally does not implement any
 * system-specific mechanics beyond initializing without error.
 */

const SYSTEM_ID = "basic-game-system";

function init() {
  // eslint-disable-next-line no-console
  console.log(`${SYSTEM_ID} | Initializing Basic Game System`);
}

init();

export { SYSTEM_ID, init };
