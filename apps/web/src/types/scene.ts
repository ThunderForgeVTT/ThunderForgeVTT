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
