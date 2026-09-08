import { Button } from "@/components/ui/button/Button";
import { Dialog } from "@/components/ui/dialog/Dialog";
import { IN_SESSION_LINK_GROUPS } from "@/components/navigation/siteLinks";

/**
 * The footer's content, for a screen that cannot carry a footer.
 *
 * # Why the play field is the exception
 *
 * It is a canvas that fills the viewport, with fixed overlays over it. A
 * footer there would either be pinned across the map or live at the bottom of
 * a container that never scrolls — in both cases sitting on top of the thing
 * the screen exists to show. So the links move behind one small control
 * instead.
 *
 * They do not simply disappear: spec 039 FR-056 says the contact for
 * copyright notices must be discoverable by anyone who needs to file one, and
 * a person at a table is as entitled to find it as anyone else.
 *
 * # Why these links open in a new tab, and only these
 *
 * Everywhere else the footer navigates normally. Here the visitor is *in a
 * session* — a live world, an engine that has loaded, possibly other people
 * waiting on them. Following a link to the terms of service in place would
 * drop them out of the game to read a legal page, and the way back is a full
 * reload of the engine. `target="_blank"` is the difference between reading
 * the page and losing your seat.
 */
export function AboutInstance() {
  return (
    <Dialog
      title="About this instance"
      description="Who runs it, and the documents that govern using it."
      trigger={
        <Button
          type="button"
          variant="ghost"
          size="sm"
          icon="rune"
          data-testid="play-field-about"
          // Low contrast and out of the way until wanted: this is the least
          // important control on the screen and should look like it.
          className="pointer-events-auto fixed bottom-3 left-3 z-[900] bg-background/70 text-muted-foreground opacity-70 backdrop-blur transition-opacity hover:opacity-100"
        >
          About
        </Button>
      }
    >
      <div className="grid gap-4" data-testid="play-field-about-links">
        {IN_SESSION_LINK_GROUPS.map((group) => (
          <nav
            key={group.heading}
            className="grid gap-2"
            aria-label={group.heading}
          >
            <h3 className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              {group.heading}
            </h3>
            <ul className="grid gap-1.5">
              {group.links.map((link) => (
                <li key={link.to}>
                  <a
                    href={link.to}
                    target="_blank"
                    rel="noreferrer"
                    className="text-sm text-muted-foreground transition-colors hover:text-foreground hover:underline"
                  >
                    {link.label}
                  </a>
                </li>
              ))}
            </ul>
          </nav>
        ))}
        <p className="text-xs text-muted-foreground">
          These open in a new tab so your session stays where it is.
        </p>
      </div>
    </Dialog>
  );
}
