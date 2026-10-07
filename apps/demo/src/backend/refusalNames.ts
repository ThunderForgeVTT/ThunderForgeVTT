/**
 * What a visitor is told was refused: the part of ThunderForge they reached
 * for, in words, never the GraphQL field that asked for it.
 *
 * An exact name wins; otherwise the first family whose pattern matches the
 * field. Anything left over is "That feature", which is vaguer than a field
 * name but never jargon.
 */

const EXACT: Record<string, string> = {
  generateInviteCode: "Invite links",
  revokeInviteCode: "Invite links",
  worldInviteCodes: "Invite links",
  redeemInviteCode: "Invite links",
  deleteWorld: "Deleting a world",
  createWorld: "Creating another world",
  logout: "Signing out",
  systemStatus: "The system status page",
  exportMyData: "Exporting your account",
  deleteMyAccount: "Deleting your account",
  uploadLoreImage: "Uploading images to lore",
  worldEventsSince: "Catching up on missed changes",
};

const FAMILIES: Array<[RegExp, string]> = [
  [/invite/i, "Invite links"],
  [/combat|encounter|initiative|attack|damage/i, "Combat"],
  [
    /loreRepository|loreSync|loreIncoming|lorePending/i,
    "Syncing lore with a repository",
  ],
  [/github|repository/i, "Connected repositories"],
  [/lore/i, "Lore"],
  [/compendium|book|collection|library/i, "The compendium library"],
  [/moderation|report|appeal|case/i, "Moderation"],
  [
    /account|password|email|profile|avatar|session|login|signup|twoFactor|totp|passkey/i,
    "Your account",
  ],
  [/upload|asset|image|portrait/i, "Uploads"],
  [/status|health|metric/i, "The system status page"],
  [/admin|operator|instance/i, "Instance administration"],
  [/notification|subscription|webhook/i, "Notifications"],
  [/chat|message|whisper/i, "Chat"],
  [/condition|disclosure|tokenStatus/i, "Token conditions"],
  [/scene/i, "Scenes"],
  [/actor|character|claim/i, "Characters"],
  [/member|role|permission/i, "World members"],
  [/world/i, "World settings"],
];

/** A GraphQL field or subscription name, as opposed to words already. */
const FIELD_NAME = /^[a-z][A-Za-z0-9]*$/;

/**
 * The visitor-facing name of the area a refused field belongs to. What is
 * already written for a reader ("Importing a book") passes through as it is.
 */
export function refusalArea(field: string): string {
  if (!FIELD_NAME.test(field)) return field;
  if (field in EXACT) return EXACT[field];
  for (const [pattern, name] of FAMILIES) {
    if (pattern.test(field)) return name;
  }
  return "That feature";
}
