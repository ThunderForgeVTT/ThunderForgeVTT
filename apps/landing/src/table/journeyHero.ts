import { PARTY, type PartyHero } from "../room/party.ts";

// The hero the visitor walks the page with, picked or built in the journey
// card. The room plays the same hero, so the pick is kept here, outside
// either component, and told to both.

/** One of the party, or a hero built here (`data`: PNG base64). */
export interface Walker extends PartyHero {
  data?: string;
}

const CHANGED = "thunderforge:journey-hero";

let current: Walker = PARTY[Math.floor(Math.random() * PARTY.length)]!;

export const journeyHero = () => current;

export function setJourneyHero(hero: Walker) {
  current = hero;
  window.dispatchEvent(new Event(CHANGED));
}

export function watchJourneyHero(fn: (hero: Walker) => void): () => void {
  const tell = () => fn(current);
  window.addEventListener(CHANGED, tell);
  return () => window.removeEventListener(CHANGED, tell);
}
