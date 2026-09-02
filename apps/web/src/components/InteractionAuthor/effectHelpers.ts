import type {
  ConfigField,
  EffectDeclaration,
  SubjectKind,
} from "@/api/interactives";

/**
 * Turning the effect registry into one-click helpers (spec 031 FR-028).
 *
 * # Why this is a module and not three lines inside the panel
 *
 * `apps/web`'s vitest environment is `node`, so a component is only testable
 * through `renderToStaticMarkup`. The rules worth asserting here are not about
 * markup at all — which helpers a build offers, what a namespace is, and
 * whether a chosen effect is configured enough to save — so they live in
 * plain functions the tests can call directly, and the component is left as
 * the thin thing that draws them.
 *
 * # Why nothing below names an effect
 *
 * Not one identifier in this file is written down. ADR-054's whole claim is
 * that the authorable vocabulary is the union of what is compiled in, and a
 * helper row assembled from a hand-written list would quietly re-introduce
 * the drift the seam exists to prevent — offering a button for an effect no
 * subsystem in this build performs, which at the table is a click that does
 * nothing and reports nothing.
 *
 * The one thing keyed by name is the *icon*, and it is keyed by namespace
 * rather than by id, with a fallback — the same call, for the same reason,
 * that `interaction_marker.rs` colours badges by namespace: a subsystem
 * contributing a second effect should not need a new decision made about it.
 */

/**
 * The part of an effect id before the dot — `lore`, `item`, `door`.
 *
 * Namespacing is what makes collision detection a prefix concern rather than
 * a coordination problem (ADR-054 §5), so it is also the honest thing to
 * group presentation by.
 */
export function effectNamespace(effectId: string): string {
  const dot = effectId.indexOf(".");
  return dot === -1 ? effectId : effectId.slice(0, dot);
}

/** One button in the helper row. */
export interface EffectHelper {
  id: string;
  label: string;
  description: string;
  namespace: string;
}

/**
 * The helpers to offer for a subject, in registry order.
 *
 * Registry order rather than alphabetical: the order effects are contributed
 * in is the order the engine assembled them, and re-sorting would mean this
 * file having an opinion about which subsystem matters most — an opinion it
 * cannot hold without knowing what the subsystems are.
 */
export function helpersFor(
  registry: EffectDeclaration[],
  subjectKind: SubjectKind,
): EffectHelper[] {
  return registry
    .filter((declaration) => declaration.subjectKinds.includes(subjectKind))
    .map((declaration) => ({
      id: declaration.id,
      label: declaration.label,
      description: declaration.description,
      namespace: effectNamespace(declaration.id),
    }));
}

/**
 * The required fields a draft has not filled in yet.
 *
 * Read from the declaration, never from a rule written here. The server is
 * still the authority and still refuses an incomplete effect — this only
 * moves the refusal to where the Game Master can act on it, because the
 * refusal they get today is a flat "that could not be saved" that names
 * neither the field nor the reason.
 *
 * A lore marker is the case that made this worth doing: `lore.open` is
 * useless without an entry, and an empty picker looks exactly like a picker
 * whose default is fine.
 */
export function missingRequiredFields(
  declaration: EffectDeclaration | null,
  config: Record<string, unknown>,
): ConfigField[] {
  if (!declaration) {
    return [];
  }
  return declaration.config.filter(
    (field) => field.required && !isFilledIn(config[field.key]),
  );
}

/**
 * Whether a value counts as supplied.
 *
 * `false` counts. A boolean field that is required is asking to be *decided*,
 * and treating "no" as unanswered would make one of its two legitimate
 * answers unsavable — the kind of rule that is discovered at a table by a
 * Game Master who cannot work out why the button is dead.
 */
function isFilledIn(value: unknown): boolean {
  if (value === null || value === undefined) return false;
  if (typeof value === "string") return value.trim().length > 0;
  if (Array.isArray(value)) return value.length > 0;
  return true;
}
