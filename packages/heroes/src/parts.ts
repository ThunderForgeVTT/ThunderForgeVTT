/**
 * The parts a hero is drawn from. Everything is drawn on a 256×256 canvas;
 * the head is centred at (128, 112) with radius 58 and the shoulders fill the
 * bottom, so every part lines up with every other without knowing about it.
 *
 * Only `ResolvedHero` reaches here: every colour has passed `HEX_COLOR` and
 * every choice is one of its list, so interpolating them is safe.
 */
import { shade } from "./color.ts";
import type { ResolvedHero } from "./spec.ts";

export const INK = "#2b2233";

const stroke = `stroke="${INK}" stroke-width="3" stroke-linejoin="round"`;

function body(h: ResolvedHero): string {
  return `
  <rect x="116" y="160" width="24" height="26" fill="${h.skin}" ${stroke}/>
  <path d="M40 256 Q46 196 128 186 Q210 196 216 256 Z" fill="${h.outfit}" ${stroke}/>
  <path d="M104 190 L128 222 L152 190" fill="none" stroke="${h.trim}" stroke-width="6" stroke-linecap="round"/>
  ${emblem(h)}`;
}

function emblem(h: ResolvedHero): string {
  switch (h.emblem) {
    case "sun": {
      const rays = Array.from({ length: 8 }, (_, i) => {
        const a = (i * Math.PI) / 4;
        const x1 = 128 + Math.cos(a) * 11;
        const y1 = 232 + Math.sin(a) * 11;
        const x2 = 128 + Math.cos(a) * 17;
        const y2 = 232 + Math.sin(a) * 17;
        return `<line x1="${x1.toFixed(1)}" y1="${y1.toFixed(1)}" x2="${x2.toFixed(1)}" y2="${y2.toFixed(1)}" stroke="#f4c542" stroke-width="3" stroke-linecap="round"/>`;
      }).join("");
      return `<circle cx="128" cy="232" r="8" fill="#f4c542" ${stroke}/>${rays}`;
    }
    case "cross":
      return `<path d="M122 222 h12 v8 h8 v10 h-8 v14 h-12 v-14 h-8 v-10 h8 z" fill="#f4c542" ${stroke}/>`;
    case "gear":
      return `<circle cx="128" cy="234" r="11" fill="#c9a96e" ${stroke}/><circle cx="128" cy="234" r="4" fill="${INK}"/>`;
    default:
      return "";
  }
}

function hairBack(h: ResolvedHero): string {
  switch (h.hair) {
    case "long":
      return `<path d="M64 110 Q60 196 96 206 L160 206 Q196 196 192 110 Z" fill="${h.hairColor}" ${stroke}/>`;
    case "braids":
      return `<rect x="60" y="120" width="18" height="72" rx="9" fill="${h.hairColor}" ${stroke}/>
  <rect x="178" y="120" width="18" height="72" rx="9" fill="${h.hairColor}" ${stroke}/>`;
    case "bun":
      return `<circle cx="128" cy="50" r="22" fill="${h.hairColor}" ${stroke}/>`;
    default:
      return "";
  }
}

function ears(h: ResolvedHero): string {
  if (h.ears === "pointed") {
    return `<path d="M72 112 L44 92 L74 132 Z" fill="${h.skin}" ${stroke}/>
  <path d="M184 112 L212 92 L182 132 Z" fill="${h.skin}" ${stroke}/>`;
  }
  return `<circle cx="72" cy="120" r="10" fill="${h.skin}" ${stroke}/>
  <circle cx="184" cy="120" r="10" fill="${h.skin}" ${stroke}/>`;
}

function head(h: ResolvedHero): string {
  return `<circle cx="128" cy="112" r="58" fill="${h.skin}" ${stroke}/>`;
}

function mouth(h: ResolvedHero): string {
  switch (h.mouth) {
    case "grin":
      return `<path d="M112 144 Q128 162 144 144 Z" fill="#8a3b3b" ${stroke}/>`;
    case "smirk":
      return `<path d="M116 148 Q132 154 142 142" fill="none" stroke="${INK}" stroke-width="3" stroke-linecap="round"/>`;
    default:
      return `<path d="M116 146 Q128 156 140 146" fill="none" stroke="${INK}" stroke-width="3" stroke-linecap="round"/>`;
  }
}

function face(h: ResolvedHero): string {
  const eye = (x: number) => `
  <ellipse cx="${x}" cy="120" rx="11" ry="13" fill="#ffffff" ${stroke}/>
  <ellipse cx="${x}" cy="123" rx="7" ry="9" fill="${h.eyes}"/>
  <circle cx="${x + 3}" cy="118" r="3" fill="#ffffff"/>`;
  const tusks = h.tusks
    ? `<path d="M112 150 l4 -12 l4 12 z" fill="#fffbe8" ${stroke}/><path d="M136 150 l4 -12 l4 12 z" fill="#fffbe8" ${stroke}/>`
    : "";
  return `${eye(106)}${eye(150)}
  <ellipse cx="92" cy="140" rx="9" ry="5" fill="#ff8fa3" opacity="0.45"/>
  <ellipse cx="164" cy="140" rx="9" ry="5" fill="#ff8fa3" opacity="0.45"/>
  ${mouth(h)}${tusks}`;
}

function beard(h: ResolvedHero): string {
  if (!h.beard) return "";
  return `<path d="M88 138 Q92 186 128 190 Q164 186 168 138 Q150 158 128 158 Q106 158 88 138 Z" fill="${h.beardColor}" ${stroke}/>
  <path d="M116 150 Q128 156 140 150" fill="none" stroke="${INK}" stroke-width="3" stroke-linecap="round"/>`;
}

const CURLS: readonly [number, number][] = [
  [80, 88],
  [98, 66],
  [120, 56],
  [142, 58],
  [162, 70],
  [178, 90],
];

function hairFront(h: ResolvedHero): string {
  switch (h.hair) {
    case "short":
      return `<path d="M70 108 Q72 52 128 50 Q184 52 186 108 Q170 84 128 82 Q86 84 70 108 Z" fill="${h.hairColor}" ${stroke}/>`;
    case "long":
    case "braids":
      return `<path d="M68 112 Q70 50 128 48 Q186 50 188 112 Q176 80 150 78 Q140 96 118 92 Q96 88 84 80 Q72 92 68 112 Z" fill="${h.hairColor}" ${stroke}/>`;
    case "spiky":
      return `<path d="M68 108 L72 64 L90 78 L98 46 L114 70 L128 40 L142 70 L158 46 L166 78 L184 64 L188 108 Q164 86 128 86 Q92 86 68 108 Z" fill="${h.hairColor}" ${stroke}/>`;
    case "bun":
      return `<path d="M70 108 Q72 56 128 54 Q184 56 186 108 Q166 80 128 80 Q90 80 70 108 Z" fill="${h.hairColor}" ${stroke}/>`;
    case "curly":
      return `<g fill="${h.hairColor}" ${stroke}>${CURLS.map(
        ([x, y]) => `<circle cx="${x}" cy="${y}" r="18"/>`,
      ).join("")}</g>`;
    default:
      return "";
  }
}

const LEAVES: readonly [number, number, number][] = [
  [84, 76, -30],
  [104, 62, -15],
  [128, 56, 0],
  [152, 62, 15],
  [172, 76, 30],
];

function headgear(h: ResolvedHero): string {
  const c = h.headgearColor;
  switch (h.headgear) {
    case "helm":
      return `<path d="M66 110 Q66 46 128 44 Q190 46 190 110 L176 110 Q172 72 128 70 Q84 72 80 110 Z" fill="${c}" ${stroke}/>
  <rect x="76" y="100" width="104" height="12" rx="6" fill="${shade(c)}" ${stroke}/>
  <path d="M128 44 Q140 18 162 22 Q146 30 140 46 Z" fill="${h.accent}" ${stroke}/>`;
    case "wizard":
      return `<path d="M64 88 Q128 68 192 88 Q128 104 64 88 Z" fill="${c}" ${stroke}/>
  <path d="M86 84 L140 4 L170 84 Z" fill="${c}" ${stroke}/>
  <path d="M112 50 l4 8 l9 1 l-7 6 l2 9 l-8 -5 l-8 5 l2 -9 l-7 -6 l9 -1 z" fill="#f4c542"/>
  <circle cx="148" cy="66" r="3" fill="#f4c542"/><circle cx="130" cy="28" r="2.5" fill="#f4c542"/>`;
    case "hood":
      return `<path d="M56 132 Q52 38 128 34 Q204 38 200 132 Q196 170 180 186 L176 120 Q170 70 128 68 Q86 70 80 120 L76 186 Q60 170 56 132 Z" fill="${c}" ${stroke}/>`;
    case "circlet":
      return `<path d="M72 96 Q128 76 184 96" fill="none" stroke="#f4c542" stroke-width="7" stroke-linecap="round"/>
  <path d="M128 76 l8 10 l-8 10 l-8 -10 z" fill="${h.accent}" ${stroke}/>`;
    case "horned":
      return `<path d="M68 106 Q68 50 128 48 Q188 50 188 106 Z" fill="${c}" ${stroke}/>
  <path d="M72 84 Q42 70 40 36 Q58 58 82 66 Z" fill="#f2e6c9" ${stroke}/>
  <path d="M184 84 Q214 70 216 36 Q198 58 174 66 Z" fill="#f2e6c9" ${stroke}/>
  <rect x="70" y="96" width="116" height="12" rx="6" fill="${shade(c)}" ${stroke}/>`;
    case "cap":
      return `<ellipse cx="116" cy="70" rx="58" ry="20" fill="${c}" ${stroke}/>
  <path d="M150 62 Q186 30 206 36 Q184 48 162 70 Z" fill="${h.accent}" ${stroke}/>`;
    case "leaves":
      return `<g ${stroke}>${LEAVES.map(
        ([x, y, r]) =>
          `<ellipse cx="${x}" cy="${y}" rx="9" ry="16" transform="rotate(${r} ${x} ${y})" fill="${r === 0 ? "#f4a3c0" : "#6fbf5a"}"/>`,
      ).join("")}</g>`;
    case "headband":
      return `<rect x="70" y="86" width="116" height="14" rx="7" fill="${c}" ${stroke}/>
  <path d="M186 92 Q204 96 210 112 Q196 106 184 100 Z" fill="${c}" ${stroke}/>`;
    case "goggles":
      return `<rect x="70" y="80" width="116" height="10" rx="5" fill="#6b4a2f" ${stroke}/>
  <circle cx="106" cy="84" r="15" fill="#8fd3e8" ${stroke}/><circle cx="150" cy="84" r="15" fill="#8fd3e8" ${stroke}/>
  <circle cx="102" cy="80" r="4" fill="#ffffff" opacity="0.8"/><circle cx="146" cy="80" r="4" fill="#ffffff" opacity="0.8"/>`;
    case "tiefling":
      return `<path d="M88 66 Q74 40 84 22 Q88 44 102 58 Z" fill="#4a2b3d" ${stroke}/>
  <path d="M168 66 Q182 40 172 22 Q168 44 154 58 Z" fill="#4a2b3d" ${stroke}/>`;
    default:
      return "";
  }
}

function prop(h: ResolvedHero): string {
  switch (h.prop) {
    case "sword":
      return `<g transform="rotate(20 212 170)">
    <rect x="206" y="90" width="12" height="96" rx="3" fill="#dfe6ee" ${stroke}/>
    <rect x="194" y="184" width="36" height="10" rx="4" fill="#f4c542" ${stroke}/>
    <rect x="207" y="194" width="10" height="24" rx="3" fill="#6b4a2f" ${stroke}/>
  </g>`;
    case "staff":
      return `<rect x="206" y="92" width="10" height="164" rx="4" fill="#8a5a36" ${stroke}/>
  <circle cx="211" cy="82" r="22" fill="#b98cff" opacity="0.35"/>
  <circle cx="211" cy="82" r="14" fill="#c9a7ff" ${stroke}/>
  <circle cx="206" cy="77" r="4" fill="#ffffff"/>`;
    case "dagger":
      return `<g transform="rotate(-25 214 190)">
    <path d="M210 130 L220 130 L218 186 L212 186 Z" fill="#dfe6ee" ${stroke}/>
    <rect x="202" y="184" width="26" height="8" rx="3" fill="#4a4a5a" ${stroke}/>
    <rect x="210" y="192" width="10" height="18" rx="3" fill="#2f2f3a" ${stroke}/>
  </g>`;
    case "bow":
      return `<path d="M214 60 Q250 150 214 240" fill="none" stroke="#8a5a36" stroke-width="9" stroke-linecap="round"/>
  <path d="M214 60 Q250 150 214 240" fill="none" stroke="${INK}" stroke-width="2"/>
  <line x1="214" y1="60" x2="214" y2="240" stroke="#f2e6c9" stroke-width="2"/>`;
    case "lute":
      return `<g transform="rotate(-30 206 200)">
    <rect x="200" y="104" width="12" height="60" rx="3" fill="#8a5a36" ${stroke}/>
    <ellipse cx="206" cy="190" rx="30" ry="36" fill="#d18a4a" ${stroke}/>
    <circle cx="206" cy="184" r="9" fill="${INK}"/>
  </g>`;
    case "axe":
      return `<rect x="208" y="84" width="11" height="172" rx="4" fill="#8a5a36" ${stroke}/>
  <path d="M218 92 Q252 100 250 138 Q232 128 218 132 Z" fill="#c7ccd4" ${stroke}/>
  <path d="M209 92 Q180 100 182 130 Q196 124 209 126 Z" fill="#c7ccd4" ${stroke}/>`;
    case "vine":
      return `<path d="M212 256 Q200 200 218 160 Q232 128 212 96" fill="none" stroke="#6b4a2f" stroke-width="9" stroke-linecap="round"/>
  <ellipse cx="226" cy="140" rx="8" ry="13" transform="rotate(35 226 140)" fill="#6fbf5a" ${stroke}/>
  <ellipse cx="202" cy="118" rx="8" ry="13" transform="rotate(-35 202 118)" fill="#6fbf5a" ${stroke}/>
  <circle cx="212" cy="92" r="8" fill="#f4a3c0" ${stroke}/>`;
    case "hammer":
      return `<rect x="208" y="104" width="11" height="152" rx="4" fill="#8a5a36" ${stroke}/>
  <rect x="186" y="80" width="56" height="30" rx="6" fill="#f4c542" ${stroke}/>`;
    case "fists":
      return `<circle cx="206" cy="206" r="20" fill="${h.skin}" ${stroke}/>
  <path d="M188 200 h36 M188 210 h36" stroke="#f2e6c9" stroke-width="5" stroke-linecap="round"/>`;
    case "flame":
      return `<ellipse cx="210" cy="226" rx="20" ry="12" fill="${h.skin}" ${stroke}/>
  <path d="M210 216 Q182 190 204 160 Q204 180 214 186 Q212 168 226 152 Q238 190 210 216 Z" fill="#ff9a3c" ${stroke}/>
  <path d="M210 210 Q198 196 208 180 Q212 194 218 194 Q222 202 210 210 Z" fill="#ffe066"/>`;
    case "wrench":
      return `<g transform="rotate(25 212 180)">
    <rect x="206" y="120" width="12" height="100" rx="5" fill="#9aa4ae" ${stroke}/>
    <path d="M198 96 a18 18 0 1 0 28 0 l-6 18 h-16 z" fill="#9aa4ae" ${stroke}/>
  </g>`;
    default:
      return "";
  }
}

/** The hero, drawn back to front, so each part's outline overlaps the one
 * behind it. A hood hangs behind the head; every other headgear sits on it. */
export function figure(h: ResolvedHero): string {
  const hood = h.headgear === "hood";
  return [
    hairBack(h),
    body(h),
    hood ? headgear(h) : "",
    ears(h),
    head(h),
    face(h),
    beard(h),
    hairFront(h),
    hood ? "" : headgear(h),
    prop(h),
  ].join("\n");
}
