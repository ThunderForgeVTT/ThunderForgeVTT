#!/usr/bin/env node
/**
 * SC-010, written as a check anyone can run.
 *
 * Spec 032 says a pack author can produce a working pack from the published
 * contract alone, and that the contract has **zero references to documents
 * that do not exist**. The second half is the testable one, and it is the
 * half that rots: a contract is written once and the tree moves under it, so
 * a link that was right in September is a dead end by March and the author
 * who finds it has no way to tell whether the document was renamed or never
 * written.
 *
 * A prose review cannot catch this reliably — every link looks plausible, and
 * checking them by hand is exactly the chore that stops being done. So it is
 * checked in the diff instead, which is the argument
 * `check-system-registry.mjs` and `check-interaction-seam.mjs` each make for
 * their own invariant.
 *
 * # What is checked
 *
 * Every Markdown link in a pack contract that points at a path in this
 * repository. External links are not followed — a check that hits the network
 * fails for reasons that have nothing to do with the change being made.
 * Anchors are stripped before resolving, because `#section` addresses a place
 * within a document rather than a document.
 */

import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

/**
 * The author-facing contracts, one per kind of pack.
 *
 * Named rather than discovered: these two are *the* published contract, and a
 * new one appearing is a decision somebody should make deliberately rather
 * than something a glob quietly starts enforcing.
 */
const CONTRACTS = ["packs/systems/README.md", "packs/interface/README.md"];

/** `[text](target)`, ignoring images and reference-style definitions. */
const LINK = /\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g;

const failures = [];

for (const contract of CONTRACTS) {
  const full = path.join(repoRoot, contract);
  if (!existsSync(full)) {
    failures.push(`${contract} does not exist, and it is the contract itself`);
    continue;
  }

  const source = readFileSync(full, "utf8");
  const from = path.dirname(full);

  source.split("\n").forEach((line, index) => {
    for (const match of line.matchAll(LINK)) {
      const target = match[1];

      // Not a path in this repository: another site, a mail link, or an
      // anchor within this same document.
      if (/^[a-z][a-z0-9+.-]*:/i.test(target) || target.startsWith("#")) {
        continue;
      }

      const [pathPart] = target.split("#");
      if (pathPart === "") continue;

      const resolved = path.resolve(from, pathPart);
      if (!existsSync(resolved)) {
        failures.push(
          `${contract}:${index + 1} links to ${target}, which does not exist`,
        );
      }
    }
  });
}

if (failures.length > 0) {
  process.stdout.write(
    `[pack-docs] a published pack contract must not reference documents that\n` +
      `do not exist (spec 032 SC-010).\n\n`,
  );
  for (const failure of failures) {
    process.stdout.write(`  ${failure}\n`);
  }
  process.stdout.write(
    `\nFix the link, or write the document it promises. A dead link in a\n` +
      `contract tells an author to go and read something that is not there.\n`,
  );
  process.exit(1);
}

process.stdout.write(
  `[pack-docs] ${CONTRACTS.length} pack contracts, every referenced document present\n`,
);
