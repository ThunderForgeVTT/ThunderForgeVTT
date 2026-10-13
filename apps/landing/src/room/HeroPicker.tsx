import { PARTY, type PartyHero } from "./party.ts";

// Spec 091 FR-010: the hero, picked in the page. A new pick starts over.
// Names only: the art is the table's, fetched when the table is asked for
// (T056), and the hero mat above already shows the four.

interface Props {
  hero: PartyHero;
  onPick: (hero: PartyHero) => void;
}

export default function HeroPicker({ hero, onPick }: Props) {
  return (
    <fieldset className="room-heroes">
      <legend>Your hero</legend>
      <p className="room-hint">Picking another starts the room over.</p>
      <div className="room-heroes-list">
        {PARTY.map((h) => (
          <label key={h.slug} className="room-hero">
            <input
              type="radio"
              name="room-hero"
              value={h.slug}
              checked={h.slug === hero.slug}
              onChange={() => onPick(h)}
            />
            <span>{h.name}</span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}
