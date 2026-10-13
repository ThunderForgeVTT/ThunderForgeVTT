import type { Skill } from "./game.ts";

export const root: Skill = {
  id: "s1",
  name: "Do Anything",
  level: 1,
  parentId: null,
};

/** Every dice pool of one to three d6. A pool is what a skill level buys. */
export function everyPool(): number[][] {
  const pools: number[][] = [];
  for (const a of [1, 2, 3, 4, 5, 6]) {
    pools.push([a]);
    for (const b of [1, 2, 3, 4, 5, 6]) {
      pools.push([a, b]);
      for (const c of [1, 2, 3, 4, 5, 6]) {
        pools.push([a, b, c]);
      }
    }
  }
  return pools;
}
