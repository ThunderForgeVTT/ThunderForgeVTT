/**
 * Spec 032: the interface packs this deployment has.
 *
 * REST rather than GraphQL, mirroring `gameSystems.ts` and the server's own
 * `/api/systems` routes: a manifest is a static document served whole, and
 * routing it through the typed graph would gain nothing while putting a JSON
 * blob in it.
 *
 * There is no hand-kept list here. The server discovers packs by listing a
 * directory, so a pack that exists is a pack that is offered, and nothing on
 * this side has to be kept in step with what is installed.
 *
 * That was once the difference between this file and `gameSystems.ts`, which
 * carried `BUNDLED_SYSTEM_IDS` — seven system ids and their titles, written
 * out by hand. Spec 032 T085 pointed `/api/systems` at `packs/systems/` the
 * same way, and the asymmetry is gone: both halves discover, neither lists.
 */

/** Enough to choose from, without fetching every manifest. */
export interface InterfacePackSummary {
  id: string;
  title: string;
  version: string;
  description: string;
  /** Empty means the pack composes against any system. */
  targets: string[];
}

/** The values a pack sets for one mode. Every key optional. */
export type TokenMap = Record<string, string>;

export interface InterfaceManifest {
  id: string;
  type: "interface";
  title: string;
  version: string;
  description: string;
  light: TokenMap;
  dark: TokenMap;
  canvas?: Record<string, unknown>;
  targets: string[];
  layout?: unknown[];
}

async function readJson<T>(response: Response, what: string): Promise<T> {
  const body = (await response.json().catch(() => null)) as
    | (T & { error?: string })
    | null;

  if (!response.ok) {
    throw new Error(
      body?.error ?? `Failed to load ${what} (${response.status})`,
    );
  }
  if (!body) {
    throw new Error(`${what} response was not valid JSON`);
  }
  return body;
}

/** Every installed pack, in title order, with no pinned position for the base. */
export async function listInterfacePacks(): Promise<InterfacePackSummary[]> {
  const response = await fetch("/api/interface-packs", {
    credentials: "same-origin",
  });
  return readJson<InterfacePackSummary[]>(response, "interface packs");
}

/**
 * One pack's manifest.
 *
 * The server validates before serving, so a pack that has drifted out of
 * compliance arrives as an error rather than as something half-applied.
 */
export async function getInterfacePack(id: string): Promise<InterfaceManifest> {
  const response = await fetch(
    `/api/interface-packs/${encodeURIComponent(id)}/manifest.json`,
    { credentials: "same-origin" },
  );
  return readJson<InterfaceManifest>(response, `interface pack "${id}"`);
}
