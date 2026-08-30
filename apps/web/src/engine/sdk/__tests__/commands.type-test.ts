/**
 * Spec 029 T057 — the compiler is part of the SDK's guarantee.
 *
 * There is no runtime here on purpose. Every assertion in this file is made
 * by `tsc`, and the file's value is entirely in the `@ts-expect-error` lines:
 * each one *fails the build* if the error it expects stops happening. So if
 * the command types are ever loosened — a field made optional, a union
 * widened to `string`, an interface given an index signature — this file
 * turns red rather than quietly permitting the drift again.
 *
 * That is the specific failure this feature exists to retire. The engine
 * deserializes what it recognises and ignores the rest, so before these types
 * a misspelled field produced no display, no error, and nothing to debug.
 * Three defects in spec 029 had exactly that shape.
 *
 * Runtime drift — a stale bundle, a hand-built payload — is beyond anything
 * the compiler can see, and is covered by `e2e/status-sdk.spec.ts`, which
 * asserts the engine *reports* what it refuses. The two halves are
 * deliberate: the compiler catches what it can see, the engine says what it
 * could not.
 */

import {
  encodeStatusCommand,
  SDK_VERSION,
  type StatusCommand,
} from "../commands";

// --- what must compile ---------------------------------------------------

const clear: StatusCommand = {
  type: "clear_token_status",
  tokenId: "a-token",
};

const set: StatusCommand = {
  type: "set_token_status",
  tokenId: "a-token",
  resources: [],
};

// The version travels with every command, never per call site.
const encoded: string = encodeStatusCommand(clear);
void encoded;
void set;
void (SDK_VERSION satisfies number);

// --- what must NOT compile -----------------------------------------------

// A misspelled field is the original defect. It must not be accepted.
const misspelled: StatusCommand = {
  type: "clear_token_status",
  // @ts-expect-error `tokenID` is not `tokenId`
  tokenID: "a-token",
};
void misspelled;

// A wrong type on a field the engine reads as a string.
const wrongType: StatusCommand = {
  type: "clear_token_status",
  // @ts-expect-error `tokenId` is a string, not a number
  tokenId: 12345,
};
void wrongType;

// A command the engine has never heard of.
const unknownCommand: StatusCommand = {
  // @ts-expect-error there is no such command
  type: "set_token_health",
  tokenId: "a-token",
};
void unknownCommand;

// `resources` belongs to `set`, not to `clear` — the union is discriminated,
// and a field from the wrong arm must not slip through.
const wrongArm: StatusCommand = {
  type: "clear_token_status",
  tokenId: "a-token",
  // @ts-expect-error `clear_token_status` carries no resources
  resources: [],
};
void wrongArm;

// A required field left out entirely.
// @ts-expect-error `resources` is required on `set_token_status`
const incomplete: StatusCommand = {
  type: "set_token_status",
  tokenId: "a-token",
};
void incomplete;

// Raw JSON must not be encodable — the point of the wrapper is that no
// caller hand-builds a payload.
// @ts-expect-error a string is not a command
void encodeStatusCommand('{"type":"clear_token_status"}');
