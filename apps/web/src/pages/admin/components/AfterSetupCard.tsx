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
 * # Why it keys off the address rather than off storage
 *
 * `?bootstrap=complete` is what both setup landings navigate with. Dismissing
 * the card removes the parameter, so a reload or a bookmark of what is left
 * does not bring it back — and nothing is written to the browser or the
 * server to remember a notice that is only ever relevant for a minute.
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

export function AfterSetupCard() {
  const [searchParams, setSearchParams] = useSearchParams();
  const justFinished = searchParams.get("bootstrap") === "complete";
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
