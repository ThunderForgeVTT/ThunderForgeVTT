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

import {
  parsePort,
  parseSize,
  projectName,
  summarizeStacks,
} from "../stack.mjs";

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

describe("summarizeStacks (T040)", () => {
  const projects = [
    { Name: "slam-usa", Status: "running(8)", ConfigFiles: "/x/compose.yaml" },
    { Name: "tf-keen-oak-c82e03", Status: "running(2)", ConfigFiles: "/w/keen/compose.e2e.yml" },
    { Name: "tf-old-088-aaaaaa", Status: "exited(2)", ConfigFiles: "/w/old/compose.e2e.yml" },
  ];
  const labels = [
    "tf-keen-oak-c82e03\t/w/keen",
    "tf-keen-oak-c82e03\t/w/keen",
    "tf-old-088-aaaaaa\t/w/old",
    "\t/w/keen", // a mailpit: no compose project
  ].join("\n");
  const volumes = [
    { Name: "tf-keen-oak-c82e03_db", Labels: "com.docker.compose.project=tf-keen-oak-c82e03,com.docker.compose.volume=db", Size: "180MB" },
    { Name: "tf-keen-oak-c82e03_rustfs_data", Labels: "com.docker.compose.project=tf-keen-oak-c82e03,com.docker.compose.volume=rustfs_data", Size: "2.1MB" },
    { Name: "tf-gone-bbbbbb_db", Labels: "com.docker.compose.project=tf-gone-bbbbbb,com.docker.compose.volume=db", Size: "1.5GB" },
    { Name: "thunderforge_db", Labels: "com.docker.compose.project=thunderforge", Size: "3GB" },
  ];
  const exists = (path) => path === "/w/keen";
  const stacks = summarizeStacks({ projects, labels, volumes }, exists);
  const byName = Object.fromEntries(stacks.map((s) => [s.project, s]));

  test("lists only tf- projects, including ones left with volumes alone", () => {
    assert.deepEqual(
      stacks.map((s) => s.project),
      ["tf-gone-bbbbbb", "tf-keen-oak-c82e03", "tf-old-088-aaaaaa"],
    );
    assert.equal(byName["tf-gone-bbbbbb"].status, "volumes only");
  });

  test("names each checkout from its label, and whether it still exists", () => {
    assert.equal(byName["tf-keen-oak-c82e03"].checkout, "/w/keen");
    assert.equal(byName["tf-keen-oak-c82e03"].gone, false);
    assert.equal(byName["tf-old-088-aaaaaa"].gone, true);
    // No container left to carry the label: unknown, which is not gone.
    assert.equal(byName["tf-gone-bbbbbb"].checkout, null);
    assert.equal(byName["tf-gone-bbbbbb"].gone, null);
  });

  test("sizes the volumes", () => {
    const keen = byName["tf-keen-oak-c82e03"];
    assert.deepEqual(
      keen.volumes.map((v) => [v.name, v.size]),
      [
        ["tf-keen-oak-c82e03_db", "180MB"],
        ["tf-keen-oak-c82e03_rustfs_data", "2.1MB"],
      ],
    );
    assert.equal(keen.bytes, 180e6 + 2.1e6);
    assert.equal(byName["tf-old-088-aaaaaa"].bytes, 0);
  });

  test("parses docker's sizes", () => {
    assert.equal(parseSize("0B"), 0);
    assert.equal(parseSize("512kB"), 512e3);
    assert.equal(parseSize("1.5GB"), 1.5e9);
    assert.equal(parseSize("N/A"), 0);
  });
});
