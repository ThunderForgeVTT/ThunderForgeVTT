import { describe, expect, it } from "vitest";
import { knownSettings } from "@/api/worldSystemSettings";

/**
 * Spec 067. The form is built from what the server answers, so a type this
 * build has no control for must not be drawn as some other control.
 */

function payload(key: string, kind: string) {
  return {
    key,
    label: key,
    description: null,
    kind,
    options: [],
    min: null,
    max: null,
    maxLength: null,
    defaultValue: true,
    value: true,
    isDefault: true,
  };
}

describe("world system settings", () => {
  it("keeps the settings this build can draw, in the order given", () => {
    const read = knownSettings([
      payload("b", "integer"),
      payload("a", "boolean"),
    ]);
    expect(read.map((setting) => setting.key)).toEqual(["b", "a"]);
  });

  it("leaves out a type it has no control for", () => {
    const read = knownSettings([
      payload("known", "boolean"),
      payload("newer", "colour"),
    ]);
    expect(read.map((setting) => setting.key)).toEqual(["known"]);
  });
});
