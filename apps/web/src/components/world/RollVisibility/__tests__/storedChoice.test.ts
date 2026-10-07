import { describe, expect, it } from "vitest";

import { choicesFor, readChoice, writeChoice } from "../storedChoice";

function memory(): Storage {
  const held = new Map<string, string>();
  return {
    get length() {
      return held.size;
    },
    clear: () => held.clear(),
    getItem: (key) => held.get(key) ?? null,
    key: (at) => [...held.keys()][at] ?? null,
    removeItem: (key) => void held.delete(key),
    setItem: (key, value) => void held.set(key, value),
  };
}

const broken: Storage = {
  length: 0,
  clear() {},
  key: () => null,
  removeItem() {},
  getItem() {
    throw new Error("SecurityError");
  },
  setItem() {
    throw new Error("QuotaExceededError");
  },
};

describe("the stored roll visibility (research R8)", () => {
  it("offers a player the GM's eyes and a GM a roll of their own", () => {
    expect(choicesFor(false)).toEqual(["EVERYONE", "GM_EYES"]);
    expect(choicesFor(true)).toEqual(["EVERYONE", "GM_ONLY"]);
  });

  it("is in the open until something is chosen", () => {
    expect(readChoice(false, memory())).toBe("EVERYONE");
  });

  it("remembers a choice the role may make", () => {
    const storage = memory();
    writeChoice("GM_EYES", storage);
    expect(readChoice(false, storage)).toBe("GM_EYES");
  });

  it("never hands a role a choice it may not make", () => {
    const storage = memory();
    writeChoice("GM_EYES", storage);
    expect(readChoice(true, storage)).toBe("EVERYONE");
    writeChoice("GM_ONLY", storage);
    expect(readChoice(false, storage)).toBe("EVERYONE");
    storage.setItem("thunderforge.rollVisibility", "nonsense");
    expect(readChoice(false, storage)).toBe("EVERYONE");
  });

  it("rolls in the open when storage throws", () => {
    expect(() => writeChoice("GM_EYES", broken)).not.toThrow();
    expect(readChoice(false, broken)).toBe("EVERYONE");
  });
});
