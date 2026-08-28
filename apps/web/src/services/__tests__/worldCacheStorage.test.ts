import { describe, expect, it } from "vitest";
import {
  formatBytes,
  summariseUsage,
  userScopeName,
  type StoredBlob,
} from "../worldCacheStorage";

/**
 * The parts of the storage panel that have arithmetic in them (spec 028, US5).
 *
 * The walking and the deleting need a real OPFS and are covered by
 * `world-cache-storage-ui.spec.ts`. What is here is everything that can be
 * wrong while the filesystem behaves perfectly: a scope name that disagrees
 * with the Rust, a total that double-counts, an ordering that makes rows jump
 * between refreshes.
 */

describe("userScopeName", () => {
  /**
   * This has to agree with `UserScope::for_user` exactly. It hashes the
   * uuid's **raw 16 bytes**, not its text, and a plausible-looking
   * implementation that hashed the string would produce a valid-looking
   * directory name that simply never matches anything on disk — so the panel
   * would report an empty cache with total confidence.
   *
   * The expected value is the first 32 hex characters of SHA-256 over the
   * sixteen zero bytes, which is a fixed, checkable constant.
   */
  it("hashes the uuid's bytes, not its text", async () => {
    const nil = await userScopeName("00000000-0000-0000-0000-000000000000");
    const sixteenZeroBytes =
      "374708fff7719dd5979ec875d56cd2286f6d3cf7ec317a3b25632aab28ec37bb";

    expect(nil).toBe(sixteenZeroBytes.slice(0, 32));
    expect(nil).toHaveLength(32);
  });

  it("accepts a uuid with or without dashes, and rejects anything else", async () => {
    const dashed = await userScopeName("01a04542-33f9-7751-8d52-6e6b141229e5");
    const bare = await userScopeName("01a0454233f977518d526e6b141229e5");

    expect(dashed).toBe(bare);
    expect(await userScopeName("not-a-uuid")).toBeNull();
    expect(await userScopeName("")).toBeNull();
  });
});

describe("summariseUsage", () => {
  const blob = (worldId: string, bytes: number): StoredBlob => ({ worldId, bytes });

  it("totals each world separately and the whole store together", () => {
    const summary = summariseUsage([
      blob("world-a", 100),
      blob("world-a", 250),
      blob("world-b", 50),
    ]);

    expect(summary.totalBytes).toBe(400);
    expect(summary.worlds).toEqual([
      { worldId: "world-a", bytes: 350, blobs: 2 },
      { worldId: "world-b", bytes: 50, blobs: 1 },
    ]);
  });

  /**
   * The per-world figures must add up to the total. A panel whose rows sum to
   * something other than the headline is worse than no panel: it invites the
   * user to believe one of the two numbers, with no way to tell which.
   */
  it("reports a total that is the sum of its rows", () => {
    const summary = summariseUsage([
      blob("a", 7),
      blob("b", 11),
      blob("c", 13),
      blob("a", 17),
    ]);

    const summed = summary.worlds.reduce((total, world) => total + world.bytes, 0);
    expect(summary.totalBytes).toBe(summed);
  });

  /** Ties break on id, so a refresh cannot reorder rows that are equal. */
  it("orders by size and breaks ties deterministically", () => {
    const summary = summariseUsage([blob("zeta", 100), blob("alpha", 100), blob("mid", 500)]);

    expect(summary.worlds.map((w) => w.worldId)).toEqual(["mid", "alpha", "zeta"]);
  });

  it("reports an empty store as zero rather than failing", () => {
    expect(summariseUsage([])).toEqual({ totalBytes: 0, worlds: [] });
  });
});

describe("formatBytes", () => {
  it("uses binary units, the ones storage quotas are quoted in", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1024)).toBe("1 KiB");
    expect(formatBytes(179_424)).toBe("175 KiB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5 MiB");
  });

  /** Below a gigabyte a decimal is noise; above it, it is the whole point. */
  it("keeps a decimal only where it carries information", () => {
    expect(formatBytes(1.7 * 1024 ** 3)).toBe("1.7 GiB");
    expect(formatBytes(1.7 * 1024 ** 2)).toBe("2 MiB");
  });

  it("does not render a negative or absent figure as a size", () => {
    expect(formatBytes(-1)).toBe("0 B");
  });
});
