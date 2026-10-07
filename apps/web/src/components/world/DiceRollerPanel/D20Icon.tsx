/**
 * A twenty-sided die, face on: the hexagon outline of an icosahedron with its
 * front triangle. Red, for the dice roller's Roll button.
 */
export function D20Icon({ size = 22 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
    >
      <polygon
        points="12,1.5 21.5,7 21.5,17 12,22.5 2.5,17 2.5,7"
        fill="#dc2626"
        stroke="#7f1d1d"
        strokeWidth="1.2"
        strokeLinejoin="round"
      />
      <g fill="none" stroke="#fecaca" strokeWidth="1" strokeLinejoin="round">
        <polygon points="12,6.5 17.5,16 6.5,16" />
        <path d="M12 1.5 12 6.5M21.5 7 17.5 16M2.5 7 6.5 16M12 22.5 17.5 16M12 22.5 6.5 16M21.5 17 17.5 16M2.5 17 6.5 16M21.5 7 12 6.5M2.5 7 12 6.5" />
      </g>
    </svg>
  );
}
