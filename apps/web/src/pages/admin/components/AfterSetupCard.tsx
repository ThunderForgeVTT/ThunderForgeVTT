import { useEffect, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import {
  getInstanceAccessSettings,
  type InstanceAccessSettings,
} from "@/api/instanceAccess";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";

/**
 * What to do next, shown once, to the operator who has just finished setup.
 *
 * Setup ends by landing here, and until this card existed it ended there too:
 * an instance with one account on it and nothing saying how a second person
 * gets in. The answer is one section away and depends on what the operator
 * chose on the Access step, so this says it in those terms and links to where
 * it is done.
 *
 * # Why it keys off a mark setup leaves, and not only the address
 *
 * `?bootstrap=complete` is what both setup landings navigate with, and it is
 * not enough on its own. The moment setup stops being required the setup
 * route redirects to the public home by itself, and that redirect can win
 * the race with the landing's own navigation — the operator arrives on
 * `/admin` with no parameter, and the card never showed. So the landings also
 * leave a mark in this tab's session storage (`markSetupJustFinished`) before
 * they report completion, and either one shows the card.
 *
 * Dismissing the card removes both, so a reload or a bookmark does not bring
 * it back. Session storage rather than anything longer-lived: the notice is
 * only relevant to the tab that just finished setup.
 *
 * # Why a failed policy read still shows the card
 *
 * The wording adapts to the policy when the policy is known. When it is not,
 * the card falls back to the sentence that is true under every policy rather
 * than disappearing: the link is the point, and it is the same link.
 */

const NEXT_STEP: Record<InstanceAccessSettings["policy"], string> = {
  OPEN: "This instance is open: anybody who can reach it can create an account. Send your players its address, or issue invitations if you would rather hand each of them a link of their own.",
  INVITE_ONLY:
    "This instance is invite-only: nobody can create an account without an invitation. Issue one for each player and send them the link.",
  CLOSED:
    "This instance is closed: nobody can create an account, with or without an invitation. Change the access policy before your players can join, then issue their invitations.",
};

const WHATEVER_THE_POLICY =
  "You are the only person with an account here. Access is where you decide who may create one, and where you issue the invitations that let your players in.";

const JUST_FINISHED_KEY = "thunderforge-setup-just-finished";

/** Called by a setup landing before it reports setup complete. */
export function markSetupJustFinished(): void {
  try {
    sessionStorage.setItem(JUST_FINISHED_KEY, "1");
  } catch {
    // No storage: the address parameter is then the only signal, as before.
  }
}

function markedJustFinished(): boolean {
  try {
    return sessionStorage.getItem(JUST_FINISHED_KEY) === "1";
  } catch {
    return false;
  }
}

export function AfterSetupCard() {
  const [searchParams, setSearchParams] = useSearchParams();
  const [dismissed, setDismissed] = useState(false);
  const justFinished =
    !dismissed &&
    (searchParams.get("bootstrap") === "complete" || markedJustFinished());
  const [policy, setPolicy] = useState<InstanceAccessSettings["policy"] | null>(
    null,
  );

  useEffect(() => {
    if (!justFinished) {
      return;
    }
    let active = true;
    void getInstanceAccessSettings()
      .then((settings) => {
        if (active) {
          setPolicy(settings.policy);
        }
      })
      .catch(() => {
        // The fallback sentence is already on screen.
      });
    return () => {
      active = false;
    };
  }, [justFinished]);

  if (!justFinished) {
    return null;
  }

  const dismiss = () => {
    try {
      sessionStorage.removeItem(JUST_FINISHED_KEY);
    } catch {
      // Nothing was stored, so there is nothing to forget.
    }
    setDismissed(true);
    const next = new URLSearchParams(searchParams);
    next.delete("bootstrap");
    setSearchParams(next, { replace: true });
  };

  return (
    <Card
      surface="parchment"
      className="grid gap-3 p-6"
      data-testid="after-setup-card"
      data-policy={policy ?? "unknown"}
    >
      <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
        Setup is finished
      </p>
      <h2 className="text-lg font-semibold">Next: invite your players</h2>
      <p className="max-w-[70ch] text-sm text-muted-foreground">
        {policy ? NEXT_STEP[policy] : WHATEVER_THE_POLICY}
      </p>
      <p className="max-w-[70ch] text-sm text-muted-foreground">
        If this instance sends mail, Mail has a tester that proves it before
        somebody is waiting on a message that never arrives.
      </p>
      <div className="flex flex-wrap gap-3">
        <Button asChild icon="actors">
          <Link to="/admin/access" data-testid="after-setup-invite-link">
            {policy === "CLOSED" ? "Open Access" : "Invite players"}
          </Link>
        </Button>
        <Button asChild variant="secondary">
          <Link to="/admin/mail" data-testid="after-setup-mail-link">
            Test mail
          </Link>
        </Button>
        <Button
          type="button"
          variant="ghost"
          onClick={dismiss}
          data-testid="after-setup-dismiss"
        >
          Dismiss
        </Button>
      </div>
    </Card>
  );
}
