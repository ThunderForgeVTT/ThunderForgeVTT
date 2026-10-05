import { AlertTriangle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { EngineStopReason } from "@/engine/bevy/engineStopped";

const WHY: Record<EngineStopReason, string> = {
  crashed: "The map hit an error and stopped drawing.",
  "context-lost": "Your browser took the map's graphics away.",
};

/**
 * The board died after it had started (spec 070).
 *
 * Not `EngineLoader`'s error: that one is a download that failed and can be
 * tried again in place. A running engine that stops cannot be restarted in
 * the page it stopped in, so the only honest action is a reload — and the
 * thing a person at a table most needs to hear is that reloading costs them
 * nothing, because the table was never kept here.
 */
export function EngineStopped({
  reason,
  className,
}: {
  reason: EngineStopReason;
  className?: string;
}) {
  return (
    <div
      role="alert"
      data-testid="engine-stopped"
      data-reason={reason}
      className={cn(
        "flex w-full flex-col items-center justify-center gap-3 p-6 text-center",
        className,
      )}
    >
      <AlertTriangle className="size-8 text-destructive" aria-hidden="true" />
      <p className="text-sm font-medium">The board stopped</p>
      <p className="max-w-md text-xs text-muted-foreground">
        {WHY[reason]} Nothing is lost — the table is saved on the server. Reload
        to bring the board back.
      </p>
      <Button
        variant="secondary"
        size="sm"
        onClick={() => window.location.reload()}
        data-testid="engine-stopped-reload"
      >
        Reload
      </Button>
    </div>
  );
}
