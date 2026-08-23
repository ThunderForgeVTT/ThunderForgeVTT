import type { SystemManifest } from "@/contexts/GameSystemContext";

/**
 * Spec 016 (T008): the set of system packs actually bundled with this
 * deployment today. There is no reconciled "installed system packs"
 * catalog yet — the existing `gameSystems` GraphQL query and `/api/systems`
 * REST list both read from a `game_systems` database table that nothing
 * currently seeds with the bundled packs under `packs/systems/` (a
 * pre-existing gap, out of this spec's scope to fully reconcile — building
 * the five remaining system packs is explicitly out of scope per spec
 * Assumptions). This constant is the honest, minimal picker source for
 * what's actually selectable right now; extend it (or replace it with a
 * real catalog query) once more packs exist.
 */
export const BUNDLED_SYSTEM_IDS: readonly string[] = ["dnd5e"];

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
