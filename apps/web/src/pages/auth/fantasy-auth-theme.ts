export const fantasyAuthTheme = {
  colors: {
    gold: "#D4AF37",
    goldBright: "#F0D596",
    parchment: "#F5E6C8",
    parchmentSoft: "#FAF1DE",
    deepBrown: "#2C1A0E",
    obsidian: "#120C09",
    obsidianSoft: "#24160F",
    violet: "#7A5C9E",
    candlelight: "#FFE6A3",
    success: "#76A06A",
    danger: "#C7664D",
  },
  radii: {
    shell: "32px",
    panel: "26px",
    field: "18px",
    pill: "999px",
  },
  shadows: {
    panel: "0 24px 64px rgba(7, 4, 2, 0.42)",
    panelInner:
      "inset 0 1px 0 rgba(255, 230, 163, 0.18), inset 0 -14px 26px rgba(18, 12, 9, 0.12)",
    glow: "0 0 28px rgba(255, 230, 163, 0.18)",
  },
  motion: {
    base: "220ms ease",
    slow: "320ms ease",
    runePulse: "2200ms ease-in-out infinite",
    unfurl: "420ms cubic-bezier(0.22, 1, 0.36, 1)",
    shimmer: "4600ms linear infinite",
  },
  textures: {
    parchment:
      "linear-gradient(180deg, rgba(250, 241, 222, 0.98), rgba(245, 230, 200, 0.94))",
    parchmentOverlay:
      "radial-gradient(circle at top left, rgba(255, 230, 163, 0.24), transparent 28%)",
    leather:
      "linear-gradient(180deg, rgba(107, 66, 38, 0.96), rgba(18, 12, 9, 0.98))",
  },
} as const;

export type FantasyAuthTheme = typeof fantasyAuthTheme;
