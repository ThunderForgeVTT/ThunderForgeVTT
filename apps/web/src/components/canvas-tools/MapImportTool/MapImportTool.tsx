import { useCallback, useRef, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Panel } from "@/components/ui/panel/Panel";
import { Loader } from "@/components/ui/loader/Loader";
import { withCsrf } from "@/api/auth";

export interface MapImportResult {
  wallsCreated: number;
  doorsCreated: number;
  lightsCreated: number;
  backgroundImageSet: boolean;
  skippedDegeneratePolygons: number;
}

export interface MapImportToolProps {
  sceneId: string;
  onImportComplete?: (result: MapImportResult) => void;
}

type ImportStatus = "idle" | "uploading" | "error";

interface ErrorPayload {
  error?: string;
}

/**
 * MapImportTool: canvas toolbar button that opens a file picker for a
 * Universal VTT (`.dd2vtt`) file and POSTs it to the REST map-import
 * endpoint, per specs/001-bevy-canvas-authoring T028.
 *
 * This is a plain `fetch` (not GraphQL) because the endpoint is a
 * `multipart/form-data` upload — see contracts/graphql.md's "Map Import
 * (REST, not GraphQL)" section. Auth/credentials mirror the GraphQL
 * client in `api/walls.ts`: same-origin cookies plus the CSRF header
 * helper (`withCsrf`) already used by every other mutating request in
 * this app (see `api/auth.ts`).
 *
 * GM-only: the caller (WorldPage) is responsible for only rendering this
 * component for the scene owner (FR-009), mirroring WallTool's gating.
 */
export function MapImportTool({
  sceneId,
  onImportComplete,
}: MapImportToolProps) {
  const [status, setStatus] = useState<ImportStatus>("idle");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [lastResult, setLastResult] = useState<MapImportResult | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const openFilePicker = useCallback(() => {
    fileInputRef.current?.click();
  }, []);

  const handleFileChange = useCallback(
    async (event: React.ChangeEvent<HTMLInputElement>) => {
      const file = event.target.files?.[0];
      // Reset the input so choosing the same file again re-triggers onChange.
      event.target.value = "";

      if (!file) {
        return;
      }

      setStatus("uploading");
      setErrorMessage(null);

      try {
        const formData = new FormData();
        formData.append("file", file);

        const response = await fetch(`/api/scenes/${sceneId}/import/uvtt`, {
          method: "POST",
          credentials: "same-origin",
          headers: withCsrf(),
          body: formData,
        });

        if (!response.ok) {
          let message = `Import failed with status ${response.status}`;

          if (response.status === 403) {
            message =
              "You don't have permission to import a map into this scene.";
          } else if (response.status === 413) {
            message = "That file is too large to upload.";
          }

          try {
            const payload = (await response.json()) as ErrorPayload;
            if (payload?.error) {
              message = payload.error;
            }
          } catch {
            // Response body wasn't JSON; fall back to the status-based message.
          }

          setErrorMessage(message);
          setStatus("error");
          return;
        }

        const result = (await response.json()) as MapImportResult;
        setLastResult(result);
        setStatus("idle");
        onImportComplete?.(result);
      } catch {
        setErrorMessage(
          "Failed to upload the map file. Check your connection and try again.",
        );
        setStatus("error");
      }
    },
    [sceneId, onImportComplete],
  );

  return (
    <div className="grid gap-3" data-testid="map-import-tool">
      <input
        ref={fileInputRef}
        type="file"
        accept=".dd2vtt"
        className="hidden"
        onChange={(event) => void handleFileChange(event)}
      />

      <Button
        type="button"
        variant="secondary"
        icon="map"
        onClick={openFilePicker}
        disabled={status === "uploading"}
      >
        Import map
      </Button>

      {status === "uploading" ? (
        <Panel variant="stone" className="grid gap-2">
          <Loader label="Uploading map..." />
        </Panel>
      ) : null}

      {status === "error" && errorMessage ? (
        <Panel variant="stone" className="grid gap-2" role="alert">
          <p className="text-xs font-semibold tracking-widest text-destructive uppercase">
            Import failed
          </p>
          <p className="text-sm text-destructive">{errorMessage}</p>
        </Panel>
      ) : null}

      {status === "idle" && lastResult ? (
        <Panel
          variant="stone"
          className="grid gap-1"
          data-testid="map-import-success"
        >
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Map imported
          </p>
          <p className="text-sm text-muted-foreground">
            {lastResult.wallsCreated} walls, {lastResult.doorsCreated} doors,{" "}
            {lastResult.lightsCreated} lights
            {lastResult.skippedDegeneratePolygons > 0
              ? ` (${lastResult.skippedDegeneratePolygons} skipped)`
              : ""}
          </p>
        </Panel>
      ) : null}
    </div>
  );
}
