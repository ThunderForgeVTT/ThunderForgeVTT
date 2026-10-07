import { INKS, stroke, type Ink } from "./marker.ts";

/** A hand-drawn tick mark, the green marker's check. */
export function Check({ ink = "green" }: { ink?: Ink }) {
  return (
    <svg className="check" viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M4 13.5 Q7 15.5 9.5 19 Q13 10 21 4"
        stroke={INKS[ink]}
        className="marker"
        fill="none"
      />
    </svg>
  );
}

/** A loose marker ring around whatever it is laid over. */
export function Ring({ ink = "green", seed = 3 }: { ink?: Ink; seed?: number }) {
  const t = (seed % 7) * 0.9;
  return (
    <svg className="ring" viewBox="0 0 120 50" preserveAspectRatio="none" aria-hidden="true">
      <path
        d={`M${12 + t} 9 C 40 1, 104 2, 114 20 C 121 36, 84 48, 52 47 C 18 46, 2 36, 6 22 C 9 13, 26 7, 46 6`}
        stroke={INKS[ink]}
        className="marker"
        fill="none"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}
