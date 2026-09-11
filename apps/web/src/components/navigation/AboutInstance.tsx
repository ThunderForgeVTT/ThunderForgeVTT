import { IN_SESSION_LINK_GROUPS } from "@/components/navigation/siteLinks";

/**
 * The footer's content, for a screen that cannot carry a footer.
 *
 * # Why the play field is the exception
 *
 * It is a canvas that fills the viewport, with fixed overlays over it. A
 * footer there would either be pinned across the map or live at the bottom of
 * a container that never scrolls — in both cases sitting on top of the thing
 * the screen exists to show. So the links live in the play field's dock, in
 * the "About & feedback" section (playtest 2026-09-10 P4), rather than in a
 * corner where the tool rail and the dock painted over them.
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
export function AboutLinks() {
  return (
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
  );
}
