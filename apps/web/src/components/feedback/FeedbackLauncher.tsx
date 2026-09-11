/**
 * The control that is on every screen (spec 037, FR-001).
 *
 * # Why it is mounted above the router and not inside a layout
 *
 * FR-001 says "from any screen … without navigating away from it". This app
 * has no single layout that every screen passes through: `MainLayout` wraps
 * most routes, but `/world/:id/play` (the whole play session), `/join/:code`
 * and `/status` are rendered outside it, and those are exactly the screens a
 * person is most likely to be on when something goes wrong. Putting the
 * launcher in a layout would put it on most screens, which is a different
 * requirement.
 *
 * So it is mounted from `main.tsx`, as a sibling of `<App />` inside
 * `BrowserRouter` and `AuthProvider` — the first app-root overlay host this
 * codebase has. Inside the router because it needs `useLocation()` for the
 * screen it is reporting from; a sibling rather than a wrapper because it must
 * survive whatever `App` decides to render, including its "could not load the
 * instance" state.
 *
 * # Signed in only
 *
 * spec.md's Assumptions put anonymous submission out of scope, and
 * `contracts/feedback.md` rule 2 has the resolver refuse without a session. A
 * control that opened a form and then failed at the end would waste what
 * somebody wrote, so it is not offered to a signed-out visitor at all.
 */

import { useEffect, useState } from "react";
import { useLocation } from "react-router-dom";
import { MessageSquarePlusIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useAuth } from "@/hooks/useAuth";
import { setSubmitterEmail } from "@/services/feedbackRedaction";
import { FeedbackDialog } from "./FeedbackDialog";

export function FeedbackLauncher() {
  const { isAuthenticated, user } = useAuth();
  const location = useLocation();
  const [open, setOpen] = useState(false);

  /*
    FR-013: the submitter's own address must not reach the destination, and a
    log line is one of the two routes it could take. The redactor learns the
    address here — this is the one component that is both mounted for the whole
    session and aware of who is signed in — and forgets it on sign-out.

    Deliberately before the early return: a rule that only got installed on the
    screens that render a button would be no rule at all.
  */
  useEffect(() => {
    setSubmitterEmail(user?.email ?? null);
  }, [user?.email]);

  if (!isAuthenticated) {
    return null;
  }

  // The play field carries its own, in the dock's "About & feedback" section
  // (playtest 2026-09-10 P4): floating here it sat under the dock rail and
  // vanished behind any open dock section. FR-001 still holds there, since
  // the dock rail is always on screen.
  if (/^\/world\/[^/]+\/play\/?$/.test(location.pathname)) {
    return null;
  }

  return (
    <>
      {/*
        Fixed, and below the dialog layer rather than in it: the launcher is
        chrome that sits over the page, and z-50 keeps it under the z-9999 the
        dialog overlay uses, so opening the form never leaves the button
        floating on top of its own dialog.
      */}
      <div className="fixed right-4 bottom-4 z-50 print:hidden">
        <Button
          type="button"
          size="sm"
          variant="secondary"
          data-testid="feedback-launcher"
          aria-haspopup="dialog"
          onClick={() => setOpen(true)}
        >
          <MessageSquarePlusIcon data-icon="inline-start" />
          Feedback
        </Button>
      </div>
      {/*
        Mounted only while it is open, so the evidence it gathers — the log
        snapshot, the screenshot bytes — lives exactly as long as the form
        does and cannot be inherited by the next one. The draft is what
        survives, and it survives in `sessionStorage` (FR-005), not here.
      */}
      {open ? (
        <FeedbackDialog
          open={open}
          onOpenChange={setOpen}
          pathname={location.pathname}
        />
      ) : null}
    </>
  );
}
