import type { Hero } from "@thunderforge/encounter";

// The same four as PARTY in src/sections/Hero.tsx, with the art
// `scripts/make-room-art.mjs` built from them. Sir Pip leads (FR-010).

export interface PartyHero extends Hero {
  slug: string;
}

export const PARTY: readonly PartyHero[] = [
  { slug: "sir-pip", name: "Sir Pip", art: "/room/sir-pip.webp" },
  { slug: "mira-starweave", name: "Mira Starweave", art: "/room/mira-starweave.webp" },
  { slug: "fenna-swiftbow", name: "Fenna Swiftbow", art: "/room/fenna-swiftbow.webp" },
  { slug: "brother-oak", name: "Brother Oak", art: "/room/brother-oak.webp" },
];
