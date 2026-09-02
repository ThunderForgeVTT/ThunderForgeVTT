import {
  ACTOR_IMAGE_PORTRAIT,
  ACTOR_IMAGE_TOKEN,
  type ActorImageRecord,
} from "@/api/actors";

/**
 * Reading an actor's imagery by role.
 *
 * # Why a lookup and not two fields
 *
 * Spec 031 FR-036, ADR-057. Imagery is rows keyed by role, and the role set is
 * open so the deferred talking/not-talking/background images are additive. So
 * every caller asks "which of these is the portrait?" rather than reading a
 * field that only exists while there are exactly two of them.
 *
 * # Why an unknown role is silently dropped
 *
 * ADR-057 again: a role no code recognises is ignored rather than rendered.
 * Guessing — showing an unknown image in the portrait slot because it is the
 * only one there — would put a background plate on a character sheet the first
 * time a later role ships against an older client.
 */
export function imageForRole(
  images: readonly ActorImageRecord[] | null | undefined,
  role: string,
): ActorImageRecord | null {
  return images?.find((image) => image.role === role) ?? null;
}

/** The character's face, for a sheet or a panel — never the map. */
export function portraitOf(
  images: readonly ActorImageRecord[] | null | undefined,
): ActorImageRecord | null {
  return imageForRole(images, ACTOR_IMAGE_PORTRAIT);
}

/** What stands on the map — never the sheet. */
export function tokenImageOf(
  images: readonly ActorImageRecord[] | null | undefined,
): ActorImageRecord | null {
  return imageForRole(images, ACTOR_IMAGE_TOKEN);
}
