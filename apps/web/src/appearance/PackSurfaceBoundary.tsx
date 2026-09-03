/**
 * Contains a failure inside one pack-contributed surface (FR-016, SC-009).
 *
 * # Why this exists
 *
 * Nothing contained one. `PackActorSheet` handles a *fetch* rejection and says
 * the sheet could not be loaded, which is a different failure entirely: a
 * component that throws while rendering unmounts the tree above it, and React
 * with no boundary anywhere unmounts the whole application. The participant
 * gets a blank page, in the middle of a session, naming nothing.
 *
 * `apps/web` had no error boundary at all — no `componentDidCatch`, no
 * `getDerivedStateFromError` — so this is new machinery rather than an
 * existing one to reuse.
 *
 * # Why here and not at the app root
 *
 * A root boundary satisfies "the application does not crash" and fails the
 * half that matters: SC-009 requires the rest of the session to stay usable,
 * and a root boundary replaces the session with an apology. The surface is the
 * unit of containment because the surface is the unit a pack contributes.
 *
 * # Why it names the pack
 *
 * SC-009 measures two things separately — the session stays usable, and the
 * message identifies the responsible pack — because a contained failure that
 * says "something went wrong" leaves a Game Master no idea which of their
 * packs to suspect, and the containment is worth much less for it.
 *
 * `MissingPackNotice` is the tonal precedent: say it plainly, name the pack,
 * block nothing. A surface that failed is not a reason to bar the door.
 */
import { Component, type ErrorInfo, type ReactNode } from "react";

interface PackSurfaceFailedProps {
  packId: string;
  surface: string;
}

/**
 * What a participant reads when a pack's surface fails.
 *
 * A plain function rather than markup inside the boundary's `render`, so it
 * can be tested for the thing SC-009 actually measures — that the message
 * names the responsible pack — without a DOM. React's server renderer does
 * not run `getDerivedStateFromError`, so a test cannot reach the fallback by
 * throwing at it; the containment half is proved end-to-end in a browser
 * instead, which is where "the rest of the session stays usable" is a real
 * claim rather than a simulated one.
 */
export function PackSurfaceFailed({
  packId,
  surface,
}: PackSurfaceFailedProps): ReactNode {
  return (
    <div
      role="alert"
      data-slot="pack-surface-failed"
      data-pack={packId}
      className="rounded-lg border border-border bg-muted/40 p-4 text-sm text-muted-foreground"
    >
      <p>
        The <strong>{packId}</strong> pack could not draw this {surface}.
      </p>
      <p className="mt-1">
        Nothing else in this session is affected, and none of this
        character&rsquo;s data has changed.
      </p>
    </div>
  );
}

interface PackSurfaceBoundaryProps {
  /**
   * The pack actually rendering, not the one the world names.
   *
   * A world bound to a pack that is not installed has already fallen back to
   * the base pack, and the base pack is what threw. Naming the absent one
   * would send someone to look at a pack that never ran.
   */
  packId: string;
  /** What the surface is, so the message can say what is missing from view. */
  surface: string;
  children: ReactNode;
}

interface PackSurfaceBoundaryState {
  failed: boolean;
}

export class PackSurfaceBoundary extends Component<
  PackSurfaceBoundaryProps,
  PackSurfaceBoundaryState
> {
  state: PackSurfaceBoundaryState = { failed: false };

  static getDerivedStateFromError(): PackSurfaceBoundaryState {
    return { failed: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    // The message a person reads names the pack and stops there; the console
    // carries the stack, because whoever is fixing the pack needs the part
    // that would be noise on screen.
    console.error(
      `[pack:${this.props.packId}] ${this.props.surface} failed to render`,
      error,
      info.componentStack,
    );
  }

  /**
   * A different pack, or a different surface, is a different question.
   *
   * Without this a boundary that has failed once stays failed for whatever is
   * mounted into it next — navigate to another character and the sheet that
   * would have rendered fine shows the previous one's error instead.
   */
  componentDidUpdate(previous: PackSurfaceBoundaryProps): void {
    if (
      this.state.failed &&
      (previous.packId !== this.props.packId ||
        previous.surface !== this.props.surface)
    ) {
      this.setState({ failed: false });
    }
  }

  render(): ReactNode {
    if (!this.state.failed) {
      return this.props.children;
    }

    return (
      <PackSurfaceFailed
        packId={this.props.packId}
        surface={this.props.surface}
      />
    );
  }
}
