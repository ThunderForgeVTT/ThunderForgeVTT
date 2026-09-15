import type {
  AttackFlag,
  AttackPreviewRecord,
  AttackRecord,
  OfferRecord,
} from "@/types/attack";

/**
 * How an attack reads to the table, as words.
 *
 * Pure, so what a seat is told can be tested without rendering, and so the
 * log and the attacker's own confirmation say the same thing. Everything in
 * it comes from the server's answer for this viewer: an attacker the server
 * redacted is already "Unknown", and nothing here reaches for a name anywhere
 * else.
 */

const FLAG_TEXT: Record<AttackFlag, string> = {
  OUT_OF_REACH: "out of reach",
  LONG_RANGE: "long range",
  BEYOND_RANGE: "beyond range",
  NO_LINE_OF_SIGHT: "no line of sight",
  NO_REACH_DECLARED: "no reach declared",
  OVERSPENT: "overspent",
  LEGENDARY_ON_OWN_TURN: "legendary action on its own turn",
};

export function flagText(flag: AttackFlag): string {
  return FLAG_TEXT[flag];
}

/** "20 ft", or "20" when the system names no unit. */
function measured(value: number, unit: string): string {
  const rounded = Math.round(value * 100) / 100;
  return unit ? `${rounded} ${unit}` : `${rounded}`;
}

/**
 * What the attacker is told before rolling, one sentence per flag (FR-033):
 * "Out of reach: 20 ft, reach 5 ft". A warning, never a refusal — the attack
 * can still be made, and the table will be shown the same flag afterwards.
 */
export function warningTexts(preview: AttackPreviewRecord): string[] {
  const { distance, unit } = preview;
  const far = distance === null ? null : measured(distance, unit);
  return preview.flags.map((flag) => {
    switch (flag) {
      case "OUT_OF_REACH":
        return far !== null && preview.reach !== null
          ? `Out of reach: ${far}, reach ${measured(preview.reach, unit)}`
          : "Out of reach";
      case "LONG_RANGE":
        return far !== null && preview.rangeNormal !== null
          ? `Long range: ${far}, normal range ${measured(preview.rangeNormal, unit)}`
          : "Long range";
      case "BEYOND_RANGE": {
        const limit = preview.rangeLong ?? preview.rangeNormal;
        return far !== null && limit !== null
          ? `Beyond range: ${far}, range ${measured(limit, unit)}`
          : "Beyond range";
      }
      case "NO_LINE_OF_SIGHT":
        return "No line of sight: a wall is in the way";
      case "NO_REACH_DECLARED":
        return "This attack declares no reach or range";
      default: {
        const text = flagText(flag);
        return text.charAt(0).toUpperCase() + text.slice(1);
      }
    }
  });
}

/** "Aria → Goblin", "Unknown → Aria", "Aria → nothing". */
export function partiesText(attack: AttackRecord): string {
  return `${attack.attacker.label} → ${attack.target?.label ?? "nothing"}`;
}

/** "23 vs 13: hit", "4 vs 13: miss", "17: no defence", "12". */
export function outcomeText(attack: AttackRecord): string {
  const total = attack.toHit.resultValue;
  switch (attack.outcome) {
    case "HIT":
      return attack.defence === null
        ? `${total}: hit`
        : `${total} vs ${attack.defence}: hit`;
    case "MISS":
      return attack.defence === null
        ? `${total}: miss`
        : `${total} vs ${attack.defence}: miss`;
    case "NO_DEFENCE":
      return `${total}: no defence to beat`;
    case "NO_TARGET":
      return `${total}, at nothing`;
  }
}

/** What became of a hit's damage, for the table. */
export function offerText(offer: OfferRecord): string {
  const change = offer.kind === "HEALING" ? "healing" : "damage";
  switch (offer.status) {
    case "PENDING":
      return `${offer.amount} ${change} offered`;
    case "APPLIED":
      return `${offer.amount} ${change} applied`;
    case "TAKEN":
      return `${offer.amount} ${change} taken${
        offer.resolvedBy ? ` by ${offer.resolvedBy}` : ""
      }`;
    case "DECLINED":
      return `${offer.amount} ${change} declined${
        offer.resolvedBy ? ` by ${offer.resolvedBy}` : ""
      }`;
  }
}

/** The whole line, for a screen reader or a summary. */
export function attackSummary(attack: AttackRecord): string {
  const parts = [partiesText(attack)];
  if (attack.abilityName) parts.push(attack.abilityName);
  parts.push(outcomeText(attack));
  if (attack.offer) {
    parts.push(offerText(attack.offer));
  } else if (attack.damage) {
    parts.push(`${attack.damage.resultValue} damage`);
  }
  if (attack.flags.length > 0) {
    parts.push(attack.flags.map(flagText).join(", "));
  }
  return parts.join(" · ");
}
