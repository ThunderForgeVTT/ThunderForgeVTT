import { useCallback, useEffect, useState } from "react";
import { uploadCanvasImage, type CanvasImageAsset } from "@/api/assets";

export interface AssetPasteToolProps {
  worldId: string;
  sceneId: string;
  /** Whether this scene's canvas is currently the focused/active one — only listens for paste while true (US3's "focused scene canvas"), matching WallTool/MapImportTool's mount-gated GM-only convention. */
  active: boolean;
  onPasted?: (asset: CanvasImageAsset) => void;
}

type PasteStatus = "idle" | "uploading" | "error";

/**
 * AssetPasteTool: listens for a clipboard paste while the scene canvas
 * is focused/active and uploads any pasted image via `uploadCanvasImage`
 * (FR-011). Renders no visible chrome in the idle case — only a small
 * status/error indicator while uploading or after a failure (FR-013's
 * "clear error"), matching MapImportTool's status-surfacing pattern.
 *
 * Non-image clipboard content (text, files without image data) is
 * silently ignored — no upload is attempted (spec.md Edge Cases).
 *
 * GM-only: the caller (WorldPage) is responsible for only rendering this
 * component for the scene owner/authorized GM, mirroring
 * WallTool/ShapeTool's existing gating convention.
 */
export function AssetPasteTool({ worldId, sceneId, active, onPasted }: AssetPasteToolProps) {
  const [status, setStatus] = useState<PasteStatus>("idle");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const handlePaste = useCallback(
    (event: ClipboardEvent) => {
      const items = event.clipboardData?.items;
      if (!items) {
        return;
      }

      const imageItem = Array.from(items).find((item) => item.type.startsWith("image/"));
      if (!imageItem) {
        // Not an image paste (e.g. pasted text) — ignore entirely, no upload attempted.
        return;
      }

      const file = imageItem.getAsFile();
      if (!file) {
        return;
      }

      event.preventDefault();
      setStatus("uploading");
      setErrorMessage(null);

      uploadCanvasImage(worldId, sceneId, "PASTED", file)
        .then((asset) => {
          setStatus("idle");
          onPasted?.(asset);
        })
        .catch((error: unknown) => {
          setStatus("error");
          setErrorMessage(error instanceof Error ? error.message : "Failed to paste image");
        });
    },
    [worldId, sceneId, onPasted],
  );

  useEffect(() => {
    if (!active) {
      return;
    }
    document.addEventListener("paste", handlePaste);
    return () => document.removeEventListener("paste", handlePaste);
  }, [active, handlePaste]);

  if (status === "idle") {
    return null;
  }

  return (
    <div
      role="status"
      aria-live="polite"
      className="pointer-events-none fixed bottom-4 left-1/2 z-50 -translate-x-1/2 rounded-md bg-background/90 px-3 py-1.5 text-sm shadow-md"
    >
      {status === "uploading" && <span className="text-muted-foreground">Pasting image…</span>}
      {status === "error" && <span className="text-destructive">{errorMessage}</span>}
    </div>
  );
}
