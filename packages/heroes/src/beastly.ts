/**
 * The parts a hero does not have: wings, a tail, a muzzle, a hide.
 *
 * Same canvas, same skeleton and same ink as `parts.ts` — this file is a
 * seam, not a second style. It lives apart only because the humanoid parts
 * were already a long file and a dragon's anatomy is not something a reader
 * looking for the hair styles needs to scroll past.
 *
 * Everything here is drawn to keep within 110px of (128, 128): the token
 * clips to a disc of that radius, so a wingspan drawn to the full width of
 * the portrait would have its tips sliced off on the map, which is the one
 * place the drawing has to read at a glance.
 */
import { INK, shade } from "./color.ts";
import type { ResolvedHero } from "./spec.ts";

const stroke = `stroke="${INK}" stroke-width="3" stroke-linejoin="round"`;
const TOOTH = "#fffbe8";

/** The right-hand copy of a part, mirrored to the left. Wings and antlers
 * are symmetrical, so they are drawn once and reflected about x=128 rather
 * than written out twice and drifting apart when one is tuned. */
function mirrored(right: string): string {
  return `${right}<g transform="translate(256 0) scale(-1 1)">${right}</g>`;
}

/** Wings grow from the shoulder, not the ear.
 *
 * Anchoring them at (152, 186) — where the torso's shoulder is — and sweeping
 * up and out is what makes them read as wings; drawn level with the head they
 * come out as fins, which is what the first attempt did. */
export function wings(h: ResolvedHero): string {
  const membrane = h.wingColor;
  switch (h.wings) {
    case "bat":
      return mirrored(`<g>
    <path d="M152 186 Q200 136 236 118 Q208 142 208 168 Q186 166 182 192 Q162 184 152 186 Z" fill="${membrane}" ${stroke}/>
    <path d="M152 186 L236 118 M152 186 L208 168 M152 186 L182 192" stroke="${shade(membrane, 0.35)}" stroke-width="3" stroke-linecap="round" fill="none"/>
  </g>`);
    // Three feathers rather than one shape, and all three kept outside the
    // head's circle: a wing whose inner half is hidden behind the face shows
    // up as two sticks poking out of the ears.
    case "feathered":
      return mirrored(`<g fill="${membrane}" ${stroke}>
    <path d="M156 194 Q196 128 234 110 Q220 156 186 192 Z"/>
    <path d="M154 200 Q188 150 222 130 Q212 168 178 200 Z"/>
    <path d="M154 206 Q182 172 208 154 Q202 182 172 206 Z"/>
  </g>`);
    default:
      return "";
  }
}

/** The tail comes out to the left because the right-hand strip is the prop's;
 * a troll holding a club must not hold it through its own tail. */
export function tail(h: ResolvedHero): string {
  if (h.tail === "none") return "";
  const curl = `M98 238 Q44 240 30 200 Q20 170 44 150 Q34 180 54 200 Q76 218 102 214 Z`;
  switch (h.tail) {
    case "spiked":
      return `<path d="${curl}" fill="${h.skin}" ${stroke}/>
  <g fill="${h.hideColor}" ${stroke}>
    <path d="M30 198 l-16 6 l14 10 z"/>
    <path d="M26 172 l-18 -2 l12 14 z"/>
    <path d="M40 150 l-14 -12 l4 18 z"/>
  </g>`;
    case "tuft":
      return `<path d="M100 236 Q52 238 38 204 Q30 180 50 164 Q44 190 62 204 Q82 216 104 214 Z" fill="${h.skin}" ${stroke}/>
  <path d="M50 164 q-16 -22 2 -34 q2 14 16 16 q-10 6 -18 18 z" fill="${h.hideColor}" ${stroke}/>`;
    default:
      return `<path d="${curl}" fill="${h.skin}" ${stroke}/>
  <path d="M86 226 Q56 224 42 202" fill="none" stroke="${shade(h.skin, 0.3)}" stroke-width="3" stroke-linecap="round"/>`;
  }
}

/** Teeth along a jaw line, pointing down from `y` or up when `up`. Written
 * as a loop because a maw's whole read is the repetition. */
function teeth(y: number, up: boolean, xs: readonly number[]): string {
  return xs
    .map(
      (x) =>
        `<path d="M${x - 5} ${y} L${x} ${y + (up ? -11 : 11)} L${x + 5} ${y} Z" fill="${TOOTH}" stroke="${INK}" stroke-width="2" stroke-linejoin="round"/>`,
    )
    .join("");
}

/** A muzzle replaces the lower half of the face, so it draws its own mouth
 * and `figure` leaves the flat one off. */
export function muzzle(h: ResolvedHero): string {
  const nostrils = `<ellipse cx="116" cy="138" rx="4" ry="3" fill="${INK}"/><ellipse cx="140" cy="138" rx="4" ry="3" fill="${INK}"/>`;
  switch (h.muzzle) {
    case "snout":
      return `<ellipse cx="128" cy="150" rx="33" ry="23" fill="${h.skin}" ${stroke}/>
  ${nostrils}
  <path d="M110 158 Q128 170 146 158" fill="none" stroke="${INK}" stroke-width="3" stroke-linecap="round"/>`;
    case "maw":
      return `<path d="M92 130 Q128 118 164 130 L172 162 Q128 174 84 162 Z" fill="${h.skin}" ${stroke}/>
  <path d="M84 162 Q128 174 172 162 Q168 196 128 200 Q88 196 84 162 Z" fill="#6b2b2b" ${stroke}/>
  ${teeth(166, false, [98, 114, 130, 146, 160])}
  ${teeth(192, true, [106, 124, 142])}
  ${nostrils}`;
    case "beak":
      return `<path d="M102 130 L128 188 L154 130 Z" fill="#e8b04a" ${stroke}/>
  <path d="M108 146 L148 146" stroke="${INK}" stroke-width="3" stroke-linecap="round"/>`;
    default:
      return "";
  }
}

const WARTS: readonly [number, number, number][] = [
  [96, 96, 6],
  [160, 92, 5],
  [86, 126, 5],
  [172, 128, 6],
  [128, 82, 5],
];

/** The hide on the head. Drawn over the skin and under the face, so a
 * scaled brow never hides an eye. */
export function hideFace(h: ResolvedHero): string {
  const c = h.hideColor;
  switch (h.hide) {
    // Brow and crown only. The eyes sit at y=120 and a row of scales across
    // them reads as a pair of grey eyebrows on somebody else's face.
    case "scales":
      return `<g fill="none" stroke="${c}" stroke-width="4" stroke-linecap="round">
    <path d="M100 78 q14 -12 28 0 q14 -12 28 0"/>
    <path d="M88 96 q14 -12 28 0 q14 -12 28 0 q14 -12 28 0"/>
  </g>`;
    case "fur":
      return `<path d="M72 104 l10 -22 l8 14 l12 -24 l10 18 l12 -26 l12 24 l10 -20 l10 24 l12 -16 l8 20 q-52 -20 -104 8 z" fill="${c}" ${stroke}/>`;
    // A skull's cheek hollows, kept clear of the eyes and small enough to
    // read as sunken bone rather than as a smudge on the face.
    case "bone":
      return `<g fill="${c}">
    <ellipse cx="98" cy="142" rx="9" ry="13"/>
    <ellipse cx="158" cy="142" rx="9" ry="13"/>
    <path d="M128 128 l9 16 h-18 z"/>
  </g>`;
    case "warts":
      return `<g fill="${c}" stroke="${INK}" stroke-width="2">${WARTS.map(
        ([x, y, r]) => `<circle cx="${x}" cy="${y}" r="${r}"/>`,
      ).join("")}</g>`;
    default:
      return "";
  }
}

/** The same hide on the chest. A creature scaled only from the neck up reads
 * as a costume, which is why this is never left out when `hideFace` is used. */
export function hideBody(h: ResolvedHero): string {
  const c = h.hideColor;
  switch (h.hide) {
    case "scales":
      return `<g fill="none" stroke="${c}" stroke-width="4" stroke-linecap="round">
    <path d="M84 214 q14 -12 28 0 q14 -12 28 0 q14 -12 28 0"/>
    <path d="M76 236 q14 -12 28 0 q14 -12 28 0 q14 -12 28 0 q14 -12 28 0"/>
  </g>`;
    case "fur":
      return `<path d="M90 192 l10 18 l10 -12 l8 20 l10 -14 l10 18 l10 -20 l8 14 l10 -20 q-38 44 -76 -4 z" fill="${c}" ${stroke}/>`;
    case "bone":
      return `<g fill="none" stroke="${c}" stroke-width="5" stroke-linecap="round">
    <path d="M96 206 q32 16 64 0"/>
    <path d="M90 226 q38 18 76 0"/>
    <path d="M88 246 q40 18 80 0"/>
  </g>`;
    case "warts":
      return `<g fill="${c}" stroke="${INK}" stroke-width="2">
    <circle cx="98" cy="216" r="6"/><circle cx="152" cy="208" r="5"/>
    <circle cx="128" cy="240" r="7"/><circle cx="84" cy="244" r="5"/>
  </g>`;
    default:
      return "";
  }
}
