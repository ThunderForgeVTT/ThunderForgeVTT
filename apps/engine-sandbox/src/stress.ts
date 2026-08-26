/**
 * Load generators for the engine sandbox.
 *
 * The point is not to make pretty scenes — it is to find the knee in each
 * curve *before* a real table does. A VTT's worst moments are predictable:
 * a battle map with the whole party plus two dozen enemies, every one of them
 * carrying a torch, half the room walled off for line of sight, and someone
 * zooming out to see the whole board. Each of those is a different cost:
 *
 * - **Tokens** cost sprite draws and, once vision is on, a visibility
 *   resolution per token per frame.
 * - **Lights** cost a fragment-shader iteration each, plus a shadow quad per
 *   (light x wall) — the only term here that grows multiplicatively.
 * - **Walls** cost segment intersections inside every occlusion query.
 * - **Zoom** cost is grid cells emitted per frame, which grows as the square
 *   of how far out you are until the cull kicks in.
 *
 * Keeping these as named, repeatable scenarios means a regression shows up as
 * a number moving, not as someone saying the app feels sluggish.
 */

/** Sends a `WorldCommand` to the engine, exactly as the app's bridge does. */
export type Send = (command: Record<string, unknown>) => void;

export interface Scenario {
  readonly name: string;
  readonly description: string;
  /** Rough count of the thing being stressed, for the report. */
  readonly magnitude: string;
  run(send: Send): void;
}

/** Deterministic PRNG so a scenario lays out identically every run.
 *
 * `Math.random` would make two runs of the same scenario incomparable, which
 * defeats the purpose of tracking a number over time. */
function seeded(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    // xorshift32
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return ((state >>> 0) % 100000) / 100000;
  };
}

function spreadOver(count: number, extent: number, seed: number): [number, number][] {
  const random = seeded(seed);
  return Array.from({ length: count }, () => [
    (random() - 0.5) * extent,
    (random() - 0.5) * extent,
  ]);
}

/** Tokens: sprite count and, with vision on, per-token visibility work. */
export function tokenStorm(count: number, extent = 4000): Scenario {
  return {
    name: `tokens-${count}`,
    description: `${count} tokens spread over ${extent}x${extent} world units`,
    magnitude: `${count} tokens`,
    run(send) {
      spreadOver(count, extent, 1337).forEach(([x, y], index) => {
        send({
          type: "upsert_token",
          token: {
            id: `stress-token-${index}`,
            x,
            y,
            z: 0,
            label: `T${index}`,
            rotation: 0,
            scale: 1,
            ownerUserId: null,
            isPrimary: false,
            photoUrl: null,
          },
        });
      });
    },
  };
}

/** Lights: shader iterations, and the light-side term of shadow generation. */
export function lightStorm(count: number, extent = 4000): Scenario {
  return {
    name: `lights-${count}`,
    description: `${count} shadow-casting lights in a dark scene`,
    magnitude: `${count} lights`,
    run(send) {
      send({ type: "set_ambient_light", level: "dark" });
      spreadOver(count, extent, 4242).forEach(([x, y], index) => {
        send({
          type: "upsert_light",
          light: {
            id: `stress-light-${index}`,
            sceneId: "stress",
            x,
            y,
            radius: 300,
            intensity: 1,
            // Varied colours so a colour-blending regression is visible.
            color: index % 3 === 0 ? "#ffc880" : index % 3 === 1 ? "#80c8ff" : "#c8ff80",
            attachedTokenId: null,
            castsShadows: true,
          },
        });
      });
    },
  };
}

/** Walls: occlusion cost, and the wall-side term of shadow generation. */
export function wallMaze(count: number, extent = 4000): Scenario {
  return {
    name: `walls-${count}`,
    description: `${count} vision-blocking wall segments`,
    magnitude: `${count} walls`,
    run(send) {
      const random = seeded(99);
      for (let index = 0; index < count; index += 1) {
        const x = (random() - 0.5) * extent;
        const y = (random() - 0.5) * extent;
        const length = 120 + random() * 400;
        const horizontal = random() > 0.5;
        send({
          type: "upsert_wall",
          wall: {
            id: `stress-wall-${index}`,
            x1: x,
            y1: y,
            x2: horizontal ? x + length : x,
            y2: horizontal ? y : y + length,
            blocksVision: true,
            blocksMovement: true,
            doorState: "none",
          },
        });
      }
    },
  };
}

/**
 * The realistic worst case: a full battle.
 *
 * Deliberately combines all four costs rather than testing them in isolation,
 * because the multiplicative term (lights x walls, for shadows) only appears
 * when both are present. Isolated scenarios would each look fine.
 */
export function pitchedBattle(): Scenario {
  return {
    name: "pitched-battle",
    description: "60 tokens, 24 torches, 120 walls, dark scene — the realistic worst case",
    magnitude: "60t / 24L / 120w",
    run(send) {
      wallMaze(120, 3000).run(send);
      lightStorm(24, 3000).run(send);
      tokenStorm(60, 3000).run(send);
      send({ type: "set_ambient_light", level: "dark" });
    },
  };
}

/** Clears everything a scenario added, so scenarios can run back to back. */
export function reset(send: Send, counts: { tokens: number; lights: number; walls: number }): void {
  for (let index = 0; index < counts.tokens; index += 1) {
    send({ type: "remove_token", tokenId: `stress-token-${index}` });
  }
  for (let index = 0; index < counts.lights; index += 1) {
    send({ type: "remove_light", lightId: `stress-light-${index}` });
  }
  for (let index = 0; index < counts.walls; index += 1) {
    send({ type: "remove_wall", wallId: `stress-wall-${index}` });
  }
  send({ type: "set_ambient_light", level: "bright" });
}

/// The load axes a ramp can push until the frame budget breaks.
///
/// Fixed-load scenarios answer "does it hold at N?", which on a machine that
/// hits vsync is almost always yes — every scenario reports an identical
/// 16.7ms and the test cannot tell 5% GPU load from 90%. Ramping answers "how
/// far until it breaks?", which is the number that actually moves when
/// something regresses.
export const RAMP_AXES: Record<string, (count: number) => Scenario> = {
  tokens: (count) => tokenStorm(count),
  lights: (count) => lightStorm(count),
  walls: (count) => wallMaze(count),
};

export const SCENARIOS: Scenario[] = [
  tokenStorm(50),
  tokenStorm(200),
  tokenStorm(500),
  lightStorm(8),
  lightStorm(32),
  lightStorm(128),
  wallMaze(200),
  wallMaze(800),
  pitchedBattle(),
];
