import type { SystemManifest } from "@/types/systemManifest";

/**
 * Enough to choose a system from, without fetching every manifest.
 *
 * Mirrors `InterfacePackSummary` in `interfacePacks.ts`, because the two
 * questions are the same question.
 */
export interface GameSystemSummary {
  id: string;
  title: string;
  description: string;
  version: string;
}

/**
 * What this deployment offers, and which system a new world gets.
 *
 * The default arrives with the list rather than from a second call, because
 * it is the same question and because the client used to answer its second
 * half from a literal. `null` means the operator configured no default, which
 * is a real answer: a world created without naming a system has none.
 */
export interface InstalledSystems {
  systems: GameSystemSummary[];
  defaultId: string | null;
}

/**
 * Every system pack this deployment has, in title order.
 *
 * # What this replaces
 *
 * `BUNDLED_SYSTEM_IDS` and `BUNDLED_SYSTEM_LABELS` stood here: two hand-kept
 * literals naming all seven bundled systems and their titles. Their comment
 * was honest about why — "there is no reconciled installed system packs
 * catalog yet ... extend it as more packs are added" — and that is the
 * hardcoded list spec 032's SC-004 forbids, since adding a system is supposed
 * to touch only that system's own pack directory.
 *
 * They existed because `/api/systems` read the `game_systems` database table,
 * and that table has never been seeded with the bundled packs, so the server
 * honestly answered an empty list and the client compensated with a literal.
 * Spec 032 T085 pointed the route at `packs/systems/` instead — the same
 * directory listing `/api/interface-packs` has always used, which is why
 * nothing here ever hardcoded interface pack names. The asymmetry was the bug.
 *
 * A pack that declares itself a template is not offered; that is the pack's
 * declaration, not a decision taken here.
 */
export async function listGameSystems(): Promise<InstalledSystems> {
  const response = await fetch("/api/systems", { credentials: "same-origin" });
  const body = (await response.json().catch(() => null)) as
    | (InstalledSystems & { error?: string })
    | null;

  if (!response.ok) {
    throw new Error(
      body?.error ?? `Failed to load game systems (${response.status})`,
    );
  }
  if (!body || !Array.isArray(body.systems)) {
    throw new Error("Game systems response was not valid JSON");
  }
  return body;
}

/**
 * A system's title, for a place that has an id and needs a name.
 *
 * Falls back to the id rather than to nothing: a pack that names a target
 * this deployment does not have must still read as *something*, and the id
 * is the truest thing left to say about it.
 */
export function titleFor(systems: GameSystemSummary[], id: string): string {
  return systems.find((system) => system.id === id)?.title ?? id;
}

/**
 * `IMPLEMENTED_SYSTEM_IDS` stood here, holding only `genie`.
 *
 * Its reason was true when written: "the other packs ship a manifest ... but
 * nothing renders/edits real actor data for them yet, so pickers mark them
 * (TBD) and disable selecting them rather than silently accepting a choice
 * that doesn't actually work in play."
 *
 * Spec 032 removed that condition rather than the caution. `PackActorSheet`
 * renders any system's sheet from what its manifest declares and what its
 * interface pack lays out, so a bundled pack now works in play by having a
 * manifest — which every one of them has. Six of the seven were disabled in
 * every picker for the absence of a hand-written React container, and there
 * is no longer such a thing to be missing.
 */

/** Fetches one system pack's full manifest — including its required
 * `legal` object — straight from the server's static-pack-serving route
 * (`GET /api/systems/:id/manifest.json`, `systems.rs::get_system_manifest`).
 * This is a plain REST route, not GraphQL: manifest content lives in each
 * pack's on-disk `system.json`, not the database. */
export function getGameSystemManifest(
  systemId: string,
): Promise<SystemManifest> {
  return fetch(`/api/systems/${encodeURIComponent(systemId)}/manifest.json`, {
    credentials: "same-origin",
  }).then(async (response) => {
    const body = (await response.json().catch(() => null)) as
      | (SystemManifest & { error?: string })
      | null;

    if (!response.ok) {
      throw new Error(
        body?.error ?? `Failed to load system manifest (${response.status})`,
      );
    }
    if (!body) {
      throw new Error("System manifest response was not valid JSON");
    }
    return body;
  });
}
