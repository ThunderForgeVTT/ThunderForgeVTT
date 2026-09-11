import { useState } from "react";
import { useLocation } from "react-router-dom";
import { MessageSquarePlusIcon } from "lucide-react";
import { AboutLinks } from "@/components/navigation/AboutInstance";
import { FeedbackDialog } from "@/components/feedback/FeedbackDialog";
import { Button } from "@/components/ui/button";
import { useAuth } from "@/hooks/useAuth";

/**
 * The play field's "About & feedback" dock section — playtest 2026-09-10 P4.
 *
 * Both used to float over the map: About in the bottom-left corner, where the
 * Game Master's tool rail painted over it, and the app-wide Feedback button in
 * the bottom-right, under the dock rail and wholly covered whenever a dock
 * section was open. The owner's note was that they were hard to see and
 * belonged in the play field rather than on top of it — so they have a dock
 * button of their own, for every role, like Chat or Settings.
 *
 * The requirements they answer still hold, because the dock rail is always on
 * screen: spec 037 FR-001 (feedback from any screen, without navigating away)
 * and spec 039 FR-056 (the copyright contact discoverable by anyone). Feedback
 * is offered only to someone signed in, as the app-wide launcher does, since
 * the resolver refuses an anonymous submission.
 */
export function HelpPanel() {
  const { isAuthenticated } = useAuth();
  const location = useLocation();
  const [feedbackOpen, setFeedbackOpen] = useState(false);

  return (
    <div className="grid gap-6" data-testid="play-field-help">
      <section className="grid gap-2">
        <h3 className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
          Feedback
        </h3>
        {isAuthenticated ? (
          <>
            <p className="text-sm text-muted-foreground">
              Something wrong, or something you wish it did? Your session stays
              where it is.
            </p>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              data-testid="play-field-feedback"
              aria-haspopup="dialog"
              onClick={() => setFeedbackOpen(true)}
            >
              <MessageSquarePlusIcon data-icon="inline-start" />
              Send feedback
            </Button>
          </>
        ) : (
          <p className="text-sm text-muted-foreground">
            Sign in to send feedback.
          </p>
        )}
      </section>

      <AboutLinks />

      {/*
        Mounted only while open, for the reason `FeedbackLauncher` gives: the
        evidence it gathers lives exactly as long as the form does.
      */}
      {feedbackOpen ? (
        <FeedbackDialog
          open={feedbackOpen}
          onOpenChange={setFeedbackOpen}
          pathname={location.pathname}
        />
      ) : null}
    </div>
  );
}
