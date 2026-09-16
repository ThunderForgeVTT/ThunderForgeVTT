import { cn } from "@/lib/utils";

/**
 * Owner, 2026-09-15, twice:
 *
 * - "When we're actually loading in the play field, it'd be really nice to
 *   have kind of a cute loading symbol."
 * - "When I select a world and it's loading a specific world image, it doesn't
 *   actually show me that that's being loaded. It sort of just freezes and
 *   waits for an interaction point."
 *
 * # Why a list of steps and not a spinner
 *
 * Bringing a table up is four waits in a trench coat: the route's own chunk,
 * the engine arriving and starting, the scene's contents being fetched, and
 * the map image — which is far and away the largest of them and, until this,
 * the only one with no signal at all. A spinner says "something is happening"
 * for all four indistinguishably, which is exactly the report: it freezes and
 * waits.
 *
 * Naming the steps turns one long unexplained wait into four short explained
 * ones, and — the part that matters most — it *moves*. A step going from
 * waiting to done is progress a person can see even when the thing they are
 * waiting for has no byte count to report.
 *
 * # Why this is not a second loading indicator
 *
 * Spec 031 FR-041, and `engine-loading.spec.ts` asserts it: at most one
 * loading affordance on screen at any moment. This component is the shape all
 * of them share — the play route's suspense fallback, the engine loader and
 * the scene loader each render it with a different step current, so the
 * hand-offs between them are invisible rather than reading as something
 * having failed and restarted.
 *
 * # Movement is optional, the information is not
 *
 * Every animated affordance here is duplicated in text, and every animation is
 * dropped under `prefers-reduced-motion`. Somebody who asked for less movement
 * gets the same four steps, the same current one, and the same byte count —
 * just still.
 */

/** Which of the four waits is happening now. */
export type BoardStep = "world" | "engine" | "scene" | "map";

const STEPS: { id: BoardStep; label: string }[] = [
  { id: "world", label: "Fetching the world" },
  { id: "engine", label: "Waking the engine" },
  { id: "scene", label: "Drawing the scene" },
  { id: "map", label: "Loading the map image" },
];

export interface BoardLoadingProps {
  /** The step now underway. Everything before it is done, everything after
   *  it is still to come. */
  current: BoardStep;
  /**
   * A line under the current step saying what specifically is happening —
   * the engine's byte count, or which parts of the scene are still coming.
   * Optional: a step with nothing to add shows its name alone.
   */
  detail?: string;
  /** Rendered under the steps — the engine's progress bar, in practice. */
  children?: React.ReactNode;
  className?: string;
}

export function BoardLoading({
  current,
  detail,
  children,
  className,
}: BoardLoadingProps) {
  const currentIndex = STEPS.findIndex((step) => step.id === current);

  return (
    <div
      className={cn(
        "flex min-h-64 w-full flex-col items-center justify-center gap-3 p-6",
        className,
      )}
      data-testid="board-loading"
      data-step={current}
      // Announced, not only drawn: a wait nobody is told about is the freeze
      // the owner reported. `status` is polite, so it is read after whatever
      // the person was already hearing. Nothing in here takes focus, so
      // nothing loses it when the panel lifts.
      role="status"
    >
      {/*
        The cute symbol the owner asked for: a die coming to rest. It is
        decorative — everything it means is written beside it — so it is
        hidden from assistive technology and stands still for anyone who
        asked for less movement.
      */}
      <span
        aria-hidden="true"
        className="text-2xl motion-safe:animate-bounce"
        data-testid="board-loading-mark"
      >
        🎲
      </span>

      <p className="text-sm font-medium">Bringing the table up…</p>

      <ol className="grid gap-1" data-testid="board-loading-steps">
        {STEPS.map((step, index) => {
          const state =
            index < currentIndex
              ? "done"
              : index === currentIndex
                ? "current"
                : "waiting";
          return (
            <li
              key={step.id}
              data-testid={`board-loading-step-${step.id}`}
              data-state={state}
              className={cn(
                "flex items-center gap-2 text-xs",
                state === "current"
                  ? "font-medium text-foreground"
                  : "text-muted-foreground",
              )}
            >
              {/*
                A marker per state, in characters rather than colour alone:
                done, underway and waiting have to be distinguishable without
                seeing a hue (WCAG 1.4.1), and they have to be distinguishable
                without seeing movement either.
              */}
              <span aria-hidden="true" className="w-3 text-center">
                {state === "done" ? "✓" : state === "current" ? "›" : "·"}
              </span>
              <span>{step.label}</span>
              {/* The state in words, for anyone who is being read this rather
                  than looking at it. */}
              <span className="sr-only">
                {state === "done"
                  ? " — done"
                  : state === "current"
                    ? " — underway"
                    : " — waiting"}
              </span>
            </li>
          );
        })}
      </ol>

      {detail ? (
        <p
          className="max-w-xs text-center text-xs text-muted-foreground"
          data-testid="board-loading-detail"
        >
          {detail}
        </p>
      ) : null}

      {children}
    </div>
  );
}
