export const fantasyTheme = {
  colors: {
    gold: "#D4AF37",
    goldBright: "#F0D596",
    parchment: "#F5E6C8",
    parchmentSoft: "#FAF1DE",
    deepBrown: "#2C1A0E",
    obsidian: "#120C09",
    obsidianSoft: "#22150F",
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
    panel: "0 22px 56px rgba(7, 4, 2, 0.45)",
    inner: "inset 0 1px 0 rgba(255, 230, 163, 0.18), inset 0 -12px 28px rgba(18, 12, 9, 0.1)",
    glow: "0 0 28px rgba(255, 230, 163, 0.18)",
  },
  textures: {
    leather:
      "linear-gradient(180deg, rgba(107, 66, 38, 0.96), rgba(18, 12, 9, 0.96))",
    parchment:
      "linear-gradient(180deg, rgba(250, 241, 222, 0.98), rgba(245, 230, 200, 0.92))",
    obsidian:
      "linear-gradient(180deg, rgba(44, 26, 14, 0.92), rgba(18, 12, 9, 0.98))",
  },
  motion: {
    slow: "260ms ease",
    pulse: "3200ms ease-in-out infinite",
    shimmer: "5600ms linear infinite",
  },
  audioHooks: {
    setupAmbience: "guild-hall-candles",
  },
} as const;

export type FantasyTheme = typeof fantasyTheme;
