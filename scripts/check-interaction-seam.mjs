#!/usr/bin/env node
/**
 * FR-039, written as a check anyone can run.
 *
 * Spec 030's whole architecture rests on one claim: `InteractionPlugin` owns
 * placement, triggering, permission and dispatch, and owns **no effect**.
 * Every effect is contributed by the subsystem that performs it.
 *
 * That claim is easy to state and easy to erode. Doors are the effect most
 * tempting to build into the core, because they are the most obviously spatial
 * thing on a map — and the moment one lands there, the core becomes the place
 * every future subsystem gets added too, which is exactly the coupling
 * Constitution Principle II forbids.
 *
 * A behavioural test cannot catch that erosion until it has already happened
 * and something breaks. This can catch it in the diff: if the words below turn
 * up in that plugin's source, somebody has taught it what an effect *is*.
 *
 * # Why words rather than an import graph
 *
 * An import check would pass against a plugin that had grown a `match` on
 * effect id strings, which is the likeliest shape the violation actually
 * takes: no new dependency, just knowledge. The words are cruder and catch it.
 *
 * # False positives are the point, not a flaw
 *
 * "highlight" contains "light" and "in flight" contains "light". A comment
 * that trips this check has to be reworded, which costs a minute — and the
 * alternative is a check with holes carved into it for convenience, which is a
 * check nobody trusts. See contracts/effect-registry.md.
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

/** The file that must know nothing about what an effect does. */
const GUARDED = "src/engine/src/plugins/interaction.rs";

/**
 * Subsystems the core must not name.
 *
 * `sound` is here although no audio subsystem exists — which is the point.
 * When one is built it contributes `audio.play` and this file does not change.
 */
const FORBIDDEN = ["light", "door", "sound"];

const source = readFileSync(path.join(repoRoot, GUARDED), "utf8");

const offences = [];
source.split("\n").forEach((line, index) => {
  const lowered = line.toLowerCase();
  for (const word of FORBIDDEN) {
    if (lowered.includes(word)) {
      offences.push({ line: index + 1, word, text: line.trim() });
    }
  }
});

if (offences.length > 0) {
  process.stdout.write(
    `\n${GUARDED} names ${offences.length} thing(s) it must know nothing about:\n\n`,
  );
  for (const offence of offences) {
    process.stdout.write(
      `  ${GUARDED}:${offence.line}  "${offence.word}"\n    ${offence.text}\n`,
    );
  }
  process.stdout.write(
    "\nThe interaction core dispatches effects and performs none (FR-039).\n" +
      "If this is a real effect, contribute it from the subsystem that owns it.\n" +
      "If it is a word in a comment, reword the comment.\n",
  );
  process.exit(1);
}

process.stdout.write(
  `[seam] ${GUARDED} names none of: ${FORBIDDEN.join(", ")}\n`,
);
