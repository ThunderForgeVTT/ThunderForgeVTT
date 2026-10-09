import assert from "node:assert/strict";
import { test } from "node:test";
import {
  MESSAGE_CAP,
  STACK_CAP,
  errorAttributes,
  reduceStack,
} from "./errors.ts";

test("the redactor is applied to the message and the stack", () => {
  const a = errorAttributes(
    "boundary",
    new Error("mail a@b.example now"),
    (s) => s.replaceAll("a@b.example", "[email]"),
  );
  assert.equal(a["error.message"], "mail [email] now");
  assert.equal(a["error.source"], "boundary");
  assert.equal(a["error.type"], "Error");
});

test("the message is cut at 512 and the stack at 4096", () => {
  const e = new Error("x".repeat(2000));
  e.stack = Array.from(
    { length: 1000 },
    (_, i) => `    at f (https://h.example/assets/a${i}.js:1:2)`,
  ).join("\n");
  const a = errorAttributes("onerror", e, (s) => s);
  assert.equal(String(a["error.message"]).length, MESSAGE_CAP);
  assert.equal(String(a["error.stack"]).length, STACK_CAP);
});

test("frames become path:line, with no origin, query or column", () => {
  const stack = [
    "TypeError: nope",
    "    at Board (https://vtt.example.org/assets/index-abc.js?v=3:10:20)",
    "    at https://vtt.example.org/assets/chunk.js:5:1",
    "render@http://localhost:5173/src/App.tsx?t=1#x:42:7",
  ].join("\n");
  assert.equal(
    reduceStack(stack),
    "/assets/index-abc.js:10\n/assets/chunk.js:5\n/src/App.tsx:42",
  );
});

test("a redactor that throws gives an empty message, never a throw", () => {
  const a = errorAttributes("onerror", "text", () => {
    throw new Error("bad rule");
  });
  assert.equal(a["error.message"], "");
});

test("the core removes any email and any UUID, whatever the port does", async () => {
  const { withPersonalDetailsRemoved } = await import("./errors.ts");
  const r = withPersonalDetailsRemoved((s) => s);
  assert.equal(
    r("a canary-7f3a@example.org in 0192F1C4-7d1e-7a2b-9c3d-4e5f60718293"),
    "a [redacted: email] in [redacted: id]",
  );
});
