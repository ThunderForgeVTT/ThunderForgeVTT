import { AlertTriangle, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { EngineLoadProgress } from "@/engine/bevy";

export interface EngineLoaderProps {
  progress: EngineLoadProgress | null;
  error: Error | null;
  onRetry?: () => void;
  className?: string;
}

function formatMegabytes(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * Spec 028 User Story 6: the engine is a large program that must arrive and
 * start before anything can be drawn, and on a first visit that wait is long
 * enough to read as a broken page.
 *
 * Three states, and the distinctions between them are the point:
 *
 * - **Downloading with a known total** — a real percentage, from real bytes.
 * - **Downloading with no total** — the server sent no `Content-Length`
 *   (chunked). FR-030 forbids inventing a percentage here, so this shows
 *   activity and a byte count instead of a lie.
 * - **Starting** — instantiation, which is perceptible at this bundle size.
 *   Shown as its own phase (FR-031) rather than leaving a progress bar
 *   parked at 100%, which reads as a hang.
 */
export function EngineLoader({
  progress,
  error,
  onRetry,
  className,
}: EngineLoaderProps) {
  if (error) {
    return (
      <div
        role="alert"
        data-testid="engine-load-error"
        className={cn(
          "flex min-h-64 w-full flex-col items-center justify-center gap-3 p-6 text-center",
          className,
        )}
      >
        <AlertTriangle className="size-8 text-destructive" aria-hidden="true" />
        <p className="text-sm font-medium">Failed to load game engine</p>
        {/* The message, not just a generic failure: a 404 and a network
            outage need different actions from the person reading this. */}
        <p className="max-w-md text-xs text-muted-foreground">
          {error.message}
        </p>
        {onRetry && (
          <Button
            variant="secondary"
            size="sm"
            onClick={onRetry}
            data-testid="engine-load-retry"
          >
            Try again
          </Button>
        )}
      </div>
    );
  }

  const stage = progress?.stage ?? "downloading";
  const total = progress?.total ?? null;
  const loaded = progress?.loaded ?? 0;
  const determinate = stage === "downloading" && total !== null && total > 0;
  // Capped at 99 while downloading: reaching 100 before the canvas is
  // interactive is what makes a loader feel stuck (SC-010).
  const percent =
    determinate && total
      ? Math.min(99, Math.floor((loaded / total) * 100))
      : null;

  return (
    <div
      data-testid="engine-loader"
      data-stage={stage}
      className={cn(
        "flex min-h-64 w-full flex-col items-center justify-center gap-3 p-6",
        className,
      )}
    >
      <Loader2
        className="size-6 animate-spin text-muted-foreground"
        aria-hidden="true"
      />

      <p
        className="text-sm text-muted-foreground"
        data-testid="engine-loader-label"
      >
        {stage === "starting"
          ? "Starting the engine…"
          : "Downloading the game engine…"}
      </p>

      {stage === "downloading" && (
        <div className="w-full max-w-xs">
          <div
            className="h-1.5 w-full overflow-hidden rounded-full bg-muted"
            role="progressbar"
            aria-label="Engine download progress"
            // Omitted entirely when indeterminate, which is how assistive
            // technology is told "working, duration unknown" rather than
            // being handed a fabricated number.
            aria-valuenow={percent ?? undefined}
            aria-valuemin={percent === null ? undefined : 0}
            aria-valuemax={percent === null ? undefined : 100}
            data-testid="engine-loader-progress"
            data-determinate={determinate ? "true" : "false"}
          >
            <div
              className={cn(
                "h-full bg-primary transition-[width] duration-200",
                !determinate && "w-1/3 animate-pulse",
              )}
              style={determinate ? { width: `${percent}%` } : undefined}
            />
          </div>
          <p className="mt-1 text-center text-xs text-muted-foreground tabular-nums">
            {determinate && total
              ? `${percent}% · ${formatMegabytes(loaded)} of ${formatMegabytes(total)}`
              : loaded > 0
                ? formatMegabytes(loaded)
                : "Connecting…"}
          </p>
        </div>
      )}
    </div>
  );
}
