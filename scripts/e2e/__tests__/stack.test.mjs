/**
 * Unit tests for `scripts/e2e/stack.mjs`'s pure parts (spec 089 T010, T011):
 * the compose project a checkout owns, and reading a published port back.
 *
 * Nothing here starts Docker. The paths are real temporary directories,
 * because the project name hashes the realpath and a made-up path has none.
 */

import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, describe, test } from "node:test";

import { parsePort, projectName } from "../stack.mjs";

describe("projectName", () => {
  let dir;

  before(() => {
    dir = mkdtempSync(join(tmpdir(), "tf-stack-"));
  });
  after(() => rmSync(dir, { recursive: true, force: true }));

  test("is tf-, the sanitised basename and a 6-hex hash", () => {
    const root = join(dir, "Thunder_Forge VTT.088");
    mkdirSync(root);
    assert.match(projectName(root), /^tf-thunder-forge-vtt-088-[0-9a-f]{6}$/);
  });

  test("is stable for one path", () => {
    const root = join(dir, "stable");
    mkdirSync(root);
    assert.equal(projectName(root), projectName(root));
  });

  test("differs for two checkouts with the same name", () => {
    const a = join(dir, "a", "ThunderForgeVTT");
    const b = join(dir, "b", "ThunderForgeVTT");
    mkdirSync(a, { recursive: true });
    mkdirSync(b, { recursive: true });
    assert.notEqual(projectName(a), projectName(b));
  });

  test("hashes the real path, so a symlink names the same stack", () => {
    const real = join(dir, "real");
    const link = join(dir, "link");
    mkdirSync(real);
    symlinkSync(real, link);
    assert.equal(projectName(link).slice(-6), projectName(real).slice(-6));
  });
});

describe("parsePort", () => {
  test("reads an IPv4 loopback binding", () => {
    assert.equal(parsePort("127.0.0.1:49153\n"), 49153);
  });

  test("reads an IPv6 loopback binding", () => {
    assert.equal(parsePort("[::1]:49154"), 49154);
  });

  test("takes the loopback line when docker prints several", () => {
    assert.equal(parsePort("127.0.0.1:49155\n[::1]:49155\n"), 49155);
  });

  test("refuses a binding on every interface", () => {
    assert.throws(() => parsePort("0.0.0.0:49156"), /loopback/);
    assert.throws(() => parsePort("[::]:49156"), /loopback/);
  });

  test("refuses empty output", () => {
    assert.throws(() => parsePort(""), /no published port/);
    assert.throws(() => parsePort("\n"), /no published port/);
  });

  test("refuses a port that is not a number", () => {
    assert.throws(() => parsePort("127.0.0.1:abc"), /not a port/);
  });
});
