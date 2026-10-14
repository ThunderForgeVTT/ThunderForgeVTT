import type { CSSProperties } from "react";
import type { Glitter } from "./useRoom.ts";

// Spec 091: the chest's opening on the board. A warm glow over the chest's
// square and a burst of gold sparks out of its interior, for the moment the
// art changes to the open chest; then it is gone and the open chest stands
// alone. Drawn by the page over the engine's board (`EngineFrame`'s
// overlay), in board pixels, so the engine draws nothing it need not. Pure
// decoration: hidden from assistive technology, and never under reduced
// motion (`useRoom` does not ask for it then).

const SPARKS = 14;

/** Each spark's way out, spread round the chest, with a little stagger. */
const sparks = Array.from({ length: SPARKS }, (_, i) => ({
  angle: -90 + ((i - (SPARKS - 1) / 2) * 150) / (SPARKS - 1) + (i % 2 ? 6 : -6),
  reach: 0.9 + (i % 3) * 0.35,
  delay: (i % 4) * 70,
  size: i % 3 === 0 ? 1 : 0.65,
}));

export default function ChestGlitter({ x, y, size, n }: Glitter) {
  return (
    <div
      key={n}
      className="room-glitter"
      aria-hidden="true"
      style={{ left: x, top: y, width: size, height: size }}
    >
      <div className="room-glitter-glow" />
      {sparks.map((s, i) => (
        <i
          key={i}
          className="room-spark"
          style={
            {
              "--spark-angle": `${s.angle}deg`,
              "--spark-reach": `${s.reach * size}px`,
              "--spark-delay": `${s.delay}ms`,
              "--spark-size": `${s.size * Math.max(4, size * 0.12)}px`,
            } as CSSProperties
          }
        />
      ))}
    </div>
  );
}
