/**
 * Host ports for a shard's processes, from the OS (spec 089, plan.md
 * Decision 4).
 *
 * Fixed bases (`5200 + index` and friends) made two checkouts' runs fight over
 * the same ports. Instead each port is bound on `127.0.0.1:0`, read, and
 * closed, and the number is handed to the child. All n are held open together
 * before any is closed, so the n are distinct.
 *
 * Something else can take a port between the close and the child's bind. The
 * runner waits on each child's URL, so that becomes a shard start failure,
 * and the shard is retried once with fresh ports.
 */

import { createServer } from "node:net";

function bindAny() {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => resolve(server));
  });
}

/** `n` distinct free ports on 127.0.0.1. */
export async function freePorts(n) {
  const servers = [];
  try {
    for (let i = 0; i < n; i += 1) servers.push(await bindAny());
    return servers.map((server) => server.address().port);
  } finally {
    await Promise.all(
      servers.map((server) => new Promise((resolve) => server.close(resolve))),
    );
  }
}
