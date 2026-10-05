import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";

/**
 * What each access policy actually admits, and what saying "this publishes"
 * will add to the rest of the wizard.
 *
 * # Why the consequences are spelled out here
 *
 * Spec 035's admin panel learned this the hard way and says so in its own
 * header: an operator who believes "closed" merely hides the registration
 * form is the person the feature exists to protect. The wizard is the *first*
 * place that belief forms, so the three sentences belong here too — not as a
 * hint under a select, but beside it, all three visible at once, because the
 * choice is a comparison.
 *
 * # Why the sentences are the control
 *
 * They used to sit beside a dropdown, which asked the operator to read three
 * cards and then find the same three words again in a select underneath. The
 * card is now the thing that is pressed: the consequence and the choice are
 * one object, and the step reads as two questions in order — who may join,
 * then whether anything leaves the table — rather than as a form.
 *
 * The wording is deliberately the same three sentences the admin panel shows
 * (`AccessPanel.tsx`'s `POLICY_COPY`). Not imported from it: that record is
 * keyed by the GraphQL enum (`INVITE_ONLY`) and this one by the registry
 * value (`invite_only`), and a conversion layer between two three-element
 * tables would be more machinery than the duplication costs. If they ever
 * disagree, the admin panel is the one people read second.
 *
 * # Why invite-only is marked
 *
 * Because it is the shipped default (the access migration seeds it, and the
 * registry declares it), and because it is the shape a private table actually
 * wants: nobody arrives uninvited, and a link admits its holder either by
 * creating an account or by signing in with a provider configured two steps
 * from here. Marked as a recommendation rather than enforced — an operator
 * standing up a public instance is not doing it wrong.
 */

const CONSEQUENCES: { value: string; label: string; detail: string }[] = [
  {
    value: "invite_only",
    label: "Invite only",
    detail:
      "Only someone holding a valid invitation may create an account. Every other route is refused, providers included.",
  },
  {
    value: "open",
    label: "Open",
    detail: "Anyone may create an account, by password or by any provider.",
  },
  {
    value: "closed",
    label: "Closed",
    detail:
      "Nobody may create an account by any means — a valid invitation included. Existing users are unaffected.",
  },
];

const RECOMMENDED = "invite_only";

const PUBLISHING: { value: boolean; label: string; detail: string }[] = [
  {
    value: false,
    label: "No, everything stays at the table",
    detail:
      "Maps, characters and collections are seen only by the people in the world they belong to. You are not asked for a copyright contact, a jurisdiction or public terms.",
  },
  {
    value: true,
    label: "Yes, people share beyond their own world",
    detail:
      "Members can share collections, characters and items outside the world they were made in. A share link works for whoever holds it. Fine on an invite-only instance, where everyone uploading is someone you invited.",
  },
];

export interface AccessConsequencesProps {
  /** The policy as the operator has it set right now, if they have chosen. */
  policy?: string;
  /** Whether they have said this instance publishes beyond a world. */
  publishes: boolean;
  /** Absent when the environment fixed the policy: shown, not offered. */
  onPolicy?: (policy: string) => void;
  /** Absent when the environment fixed the answer. */
  onPublishes?: (publishes: boolean) => void;
}

const CARD =
  "grid gap-1 rounded-lg border p-3 text-left text-sm transition-colors outline-none focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed";
const CHOSEN = "border-ring bg-primary/10";
const UNCHOSEN = "border-border enabled:hover:border-ring/60";

export function AccessConsequences({
  policy,
  publishes,
  onPolicy,
  onPublishes,
}: AccessConsequencesProps) {
  return (
    <section className="grid gap-6" data-testid="setup-access-consequences">
      <div className="grid gap-3">
        <h3 className="font-semibold">
          1. Who should be able to get an account here?
        </h3>
        <div
          role="radiogroup"
          aria-label="Who may create an account"
          className="grid gap-2"
        >
          {CONSEQUENCES.map((option) => {
            const chosen = policy === option.value;
            return (
              <button
                key={option.value}
                type="button"
                role="radio"
                aria-checked={chosen}
                disabled={!onPolicy}
                data-testid={`setup-access-consequence-${option.value}`}
                data-chosen={chosen ? "true" : "false"}
                className={`${CARD} ${chosen ? CHOSEN : UNCHOSEN}`}
                onClick={() => onPolicy?.(option.value)}
              >
                <span className="flex items-center gap-2 font-semibold">
                  {option.label}
                  {option.value === RECOMMENDED ? (
                    <StatusBadge variant="info">Recommended</StatusBadge>
                  ) : null}
                  {chosen ? (
                    <StatusBadge variant="success">Chosen</StatusBadge>
                  ) : null}
                </span>
                <span className="text-muted-foreground">{option.detail}</span>
              </button>
            );
          })}
        </div>
        {!onPolicy ? (
          <p className="text-sm text-muted-foreground">
            This deployment&rsquo;s environment fixes the answer. It cannot be
            changed here.
          </p>
        ) : null}
      </div>

      <div className="grid gap-3">
        <h3 className="font-semibold">
          2. Will anything here be shared with people outside the world it was
          made in?
        </h3>
        <div
          role="radiogroup"
          aria-label="Sharing beyond a world"
          className="grid gap-2"
        >
          {PUBLISHING.map((option) => {
            const chosen = publishes === option.value;
            return (
              <button
                key={String(option.value)}
                type="button"
                role="radio"
                aria-checked={chosen}
                disabled={!onPublishes}
                // The "yes" card carries the setting's own testid: it is the
                // control that turns the setting on, as the checkbox was.
                data-testid={
                  option.value
                    ? "setup-setting-instance.publishes_beyond_world"
                    : "setup-publishes-no"
                }
                data-chosen={chosen ? "true" : "false"}
                className={`${CARD} ${chosen ? CHOSEN : UNCHOSEN}`}
                onClick={() => onPublishes?.(option.value)}
              >
                <span className="flex items-center gap-2 font-semibold">
                  {option.label}
                  {!option.value ? (
                    <StatusBadge variant="info">Recommended</StatusBadge>
                  ) : null}
                  {chosen ? (
                    <StatusBadge variant="success">Chosen</StatusBadge>
                  ) : null}
                </span>
                <span className="text-muted-foreground">{option.detail}</span>
              </button>
            );
          })}
        </div>
      </div>

      {publishes ? (
        /*
         * FR-032 again, and the one place in the wizard where an answer
         * changes what comes after it. Said before this step is saved, because
         * "three more screens appeared and I don't know why" is the experience
         * this whole fork exists to avoid.
         */
        <p
          data-testid="setup-publishing-preview"
          className="rounded-lg border border-border bg-muted/40 p-3 text-sm text-muted-foreground"
        >
          Saying this instance publishes beyond a world adds two steps: a
          copyright-notice contact — a name, an email address and a postal
          address, served publicly so that somebody is reachable about other
          people&rsquo;s uploads — and your terms and privacy text, which people
          who are not at your table need to read before they join. Neither has
          to be finished now; what is left blank is simply not served.
        </p>
      ) : null}

      {publishes && policy !== "open" ? (
        <p
          data-testid="setup-invite-publishing-note"
          className="rounded-lg border border-border bg-muted/40 p-3 text-sm text-muted-foreground"
        >
          While sign-ups are not open, this means the people you invited can
          share what they make with each other and hand out links to it. The
          copyright-notice contact stays optional for you. Opening sign-ups to
          everyone later is what changes that: anyone could then upload and
          share from here, and you become the one answerable for copyright
          (DMCA) notices about it.
        </p>
      ) : null}

      {publishes && policy === "open" ? (
        <p
          data-testid="setup-open-publishing-warning"
          className="rounded-lg border border-amber-500/40 bg-amber-500/5 p-3 text-sm"
        >
          Open and sharing together is the combination that carries risk: anyone
          may sign up, and what they upload — copyrighted or Creative
          Commons-licensed material included — can be shared onward from an
          instance you run. That leaves you answerable for copyright (DMCA)
          notices about it. Fill in the copyright-notice contact on the next
          step, or keep this instance invite-only.
        </p>
      ) : null}
    </section>
  );
}
