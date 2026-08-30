import { describe, expect, it } from "vitest";
import {
  asSdkError,
  encodeStatusCommand,
  SDK_VERSION,
  type StatusCommand,
} from "../commands";

/**
 * Spec 029 T016 — the runtime half of the SDK's guarantee.
 *
 * The compile-time half lives in `commands.type-test.ts`, where a wrong field
 * name is a build failure. This covers what the compiler cannot see: that the
 * version actually travels, and that an engine refusal is recognised as one
 * rather than mistaken for a world event.
 */

describe("encodeStatusCommand", () => {
  it("stamps the version on every command", () => {
    const commands: StatusCommand[] = [
      { type: "clear_token_status", tokenId: "t1" },
      { type: "set_token_status", tokenId: "t1", resources: [] },
      { type: "set_display_appearance", appearance: { barHeight: 14 } },
    ];

    for (const command of commands) {
      const encoded = JSON.parse(encodeStatusCommand(command)) as {
        sdkVersion?: number;
      };
      expect(
        encoded.sdkVersion,
        `${command.type} must carry the version — the engine reads an absent one as "no claim" rather than as agreement, so a missed stamp is not even reported`,
      ).toBe(SDK_VERSION);
    }
  });

  it("preserves the command's own fields alongside the version", () => {
    const encoded = JSON.parse(
      encodeStatusCommand({ type: "clear_token_status", tokenId: "abc" }),
    ) as Record<string, unknown>;

    expect(encoded.type).toBe("clear_token_status");
    expect(encoded.tokenId).toBe("abc");
  });

  it("does not let a command override the version it is stamped with", () => {
    // A caller cannot smuggle a different version past the wrapper: the stamp
    // is applied after the spread, so this is the shape that guarantees one
    // bundle speaks one version.
    const sneaky = {
      type: "clear_token_status",
      tokenId: "t1",
      sdkVersion: 999,
    } as unknown as StatusCommand;

    const encoded = JSON.parse(encodeStatusCommand(sneaky)) as {
      sdkVersion?: number;
    };
    expect(encoded.sdkVersion).toBe(SDK_VERSION);
  });
});

describe("asSdkError", () => {
  it("recognises a refusal", () => {
    const error = asSdkError({
      type: "sdkError",
      code: "versionMismatch",
      message: "bundle is older than the engine",
      command: "set_token_status",
    });

    expect(error?.code).toBe("versionMismatch");
    expect(error?.command).toBe("set_token_status");
  });

  it("does not claim a world event is a refusal", () => {
    // The consequence of getting this wrong is not a missed error message: the
    // bridge routes anything that is not an SDK error into the world store, so
    // a false positive here would silently drop a real world command.
    for (const notAnError of [
      { type: "upsert_token", token: {} },
      { type: "set_token_status", tokenId: "t1", resources: [] },
      { type: "" },
      {},
      null,
      undefined,
      "sdkError",
      42,
    ]) {
      expect(
        asSdkError(notAnError),
        `${JSON.stringify(notAnError)}`,
      ).toBeNull();
    }
  });
});
