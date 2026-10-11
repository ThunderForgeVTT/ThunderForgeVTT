/**
 * Unit tests for `scripts/e2e/ports.mjs`: each shard's host ports come from
 * the OS (spec 089, plan.md Decision 4).
 */

import assert from "node:assert/strict";
import { createServer } from "node:net";
import { after, test } from "node:test";

import { freePorts } from "../ports.mjs";

const listeners = [];

after(() => {
  for (const server of listeners) server.close();
});

function listen(port = 0) {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => {
      listeners.push(server);
      resolve(server.address().port);
    });
  });
}

test("returns n distinct ports, each bindable on 127.0.0.1", async () => {
  const ports = await freePorts(6);
  assert.equal(ports.length, 6);
  assert.equal(new Set(ports).size, 6);
  for (const port of ports) {
    assert.ok(Number.isInteger(port) && port > 0 && port <= 65535);
    assert.equal(await listen(port), port);
  }
});

test("never returns a port a listener holds", async () => {
  const held = new Set();
  for (let i = 0; i < 20; i += 1) held.add(await listen());
  const ports = await freePorts(50);
  for (const port of ports) assert.ok(!held.has(port), `${port} is held`);
});

test("zero ports is an empty list", async () => {
  assert.deepEqual(await freePorts(0), []);
});
