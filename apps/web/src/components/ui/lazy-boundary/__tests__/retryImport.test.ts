import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { retryImport } from "../retryImport";

const dropped = () =>
  new TypeError("Failed to fetch dynamically imported module: /x.js");

describe("retryImport", () => {
  beforeEach(() => {
    vi.spyOn(Math, "random").mockReturnValue(0);
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("downloads again after a dropped connection", async () => {
    const load = vi
      .fn()
      .mockRejectedValueOnce(dropped())
      .mockResolvedValueOnce({ default: "Page" });
    await expect(retryImport(load)()).resolves.toEqual({ default: "Page" });
    expect(load).toHaveBeenCalledTimes(2);
  });

  it("gives up after four tries, so the boundary can say so", async () => {
    const load = vi.fn().mockRejectedValue(dropped());
    await expect(retryImport(load)()).rejects.toThrow(/dynamically imported/);
    expect(load).toHaveBeenCalledTimes(4);
  });

  it("does not retry a module that throws", async () => {
    const load = vi
      .fn()
      .mockRejectedValue(new TypeError("x is not a function"));
    await expect(retryImport(load)()).rejects.toThrow(/not a function/);
    expect(load).toHaveBeenCalledTimes(1);
  });
});
