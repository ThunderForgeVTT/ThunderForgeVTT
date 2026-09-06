/**
 * What an actor's system says about it, in the sets a sheet lays out.
 *
 * Spec 032's read. `actorSystemData` beside this returns the *stored* JSON
 * slots, which is what an editor writes back; this returns what the system
 * publishes — stored values resolved against the manifest, plus whatever the
 * pack derives — and it is the only one of the two a sheet should render.
 *
 * The sets come from the server because the server is the side that reads the
 * manifest: a value's set is a fact about which block declared it. The flat
 * `actorDeclaredValues` field is still there for callers wanting one list; a
 * renderer wants six, and reconstructing them here would be guessing.
 */
import { postGraphQL } from "@/api/graphqlClient";
import type { SheetDeclarations, SheetValue } from "@/sheet-layout/types";

const VALUE_FIELDS = `
  id
  label
  abbreviation
  value
  fraction { current max }
  track { filled of }
  state { current options }
  group
  groupLabel
  headline
  origin
`;

const ACTOR_SHEET_QUERY = `
  query ActorSheet($actorId: UUID!) {
    actorSheet(actorId: $actorId) {
      attributes { ${VALUE_FIELDS} }
      resources { ${VALUE_FIELDS} }
      skills { ${VALUE_FIELDS} }
      movement { ${VALUE_FIELDS} }
      derived { ${VALUE_FIELDS} }
      other { ${VALUE_FIELDS} }
      all { ${VALUE_FIELDS} }
    }
  }
`;

/** The wire shape, which is `SheetDeclarations` with `other` already worked out. */
interface ActorSheetResponse {
  attributes: SheetValue[];
  resources: SheetValue[];
  skills: SheetValue[];
  movement: SheetValue[];
  derived: SheetValue[];
  other: SheetValue[];
  all: SheetValue[];
}

/**
 * Every value this actor's system publishes.
 *
 * `other` arrives from the server and is dropped on the way through:
 * `declarationsFrom` computes it, and a complement supplied by two different
 * places is two answers that can disagree. The server's copy exists for
 * consumers that are not this renderer.
 */
export async function fetchActorSheet(
  actorId: string,
): Promise<Partial<SheetDeclarations>> {
  const data = await postGraphQL<{ actorSheet: ActorSheetResponse }>(
    ACTOR_SHEET_QUERY,
    { actorId },
  );
  const sheet = data.actorSheet;

  return {
    attributes: sheet.attributes,
    resources: sheet.resources,
    skills: sheet.skills,
    movement: sheet.movement,
    derived: sheet.derived,
    all: sheet.all,
  };
}
