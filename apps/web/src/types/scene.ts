export type SceneRecord = {
  sceneId: string;
  worldId: string;
  name: string;
  description: string | null;
  type: string;
  gridSize: number;
  gridType: string;
  width: number;
  height: number;
  backgroundImagePath: string | null;
  /**
   * Bug fix: the fetchable URL for this scene's background image,
   * regardless of which storage mechanism produced it (RustFS via
   * `background_asset_id`, preferred, or the legacy
   * `backgroundImagePath` static-file route) — `null` when no background
   * has been set. Use this to load the background, not
   * `backgroundImagePath` directly (which dd2vtt/map import never sets).
   */
  backgroundUrl: string | null;

  /**
   * Why this scene's grid does not match the background under it, or absent
   * when they agree. Server-computed — see `map_import::alignment`.
   */

  backgroundGridMismatch?: string | null;
  ownerId: string;
  createdAt: string;
  updatedAt: string;
  /** Spec 022: GM-authored Markdown source for the scene's player-facing summary. */
  summaryMarkdown: string | null;
  /** Spec 022: sanitized HTML rendered from `summaryMarkdown` — render this, not the raw markdown. */
  summaryRenderedHtml: string | null;
  /** Spec 022 (FR-003/FR-008/FR-009): player-facing visibility, hidden by default on creation. */
  hidden: boolean;
  /** Spec 022 (FR-011/FR-012): computed URL for the scene's reduced-size preview image, null until generated. */
  previewUrl: string | null;
};
