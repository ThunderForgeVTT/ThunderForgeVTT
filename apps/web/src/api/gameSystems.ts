import type { SystemManifest } from "@/contexts/GameSystemContext";

/**
 * Spec 016 (T008), extended for the six research-digest-backed packs: the
 * set of system packs actually bundled with this deployment today. There is
 * no reconciled "installed system packs" catalog yet — the existing
 * `gameSystems` GraphQL query and `/api/systems` REST list both read from a
 * `game_systems` database table that nothing currently seeds with the
 * bundled packs under `packs/systems/` (a pre-existing gap). This constant
 * is the honest, minimal picker source for what's actually selectable right
 * now; extend it (or replace it with a real catalog query) as more packs
 * are added.
 */
export const BUNDLED_SYSTEM_IDS: readonly string[] = [
  "dnd5e",
  "genie",
  "pathfinder2e",
  "cypher_system",
  "fate_core",
  "blades_in_the_dark",
  "year_zero_engine",
];

/** Display titles for `BUNDLED_SYSTEM_IDS`, mirrored from each pack's
 * `system.json` `title` field so pickers don't have to show raw ids. */
export const BUNDLED_SYSTEM_LABELS: Readonly<Record<string, string>> = {
  dnd5e: "5E System Core",
  genie: "Genie",
  pathfinder2e: "Pathfinder Second Edition",
  cypher_system: "Cypher System",
  fate_core: "Fate Core",
  blades_in_the_dark: "Blades in the Dark",
  year_zero_engine: "Year Zero Engine",
};

/** Fetches one system pack's full manifest — including its required
 * `legal` object — straight from the server's static-pack-serving route
 * (`GET /api/systems/:id/manifest.json`, `systems.rs::get_system_manifest`).
 * This is a plain REST route, not GraphQL: manifest content lives in each
 * pack's on-disk `system.json`, not the database. */
export function getGameSystemManifest(systemId: string): Promise<SystemManifest> {
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
