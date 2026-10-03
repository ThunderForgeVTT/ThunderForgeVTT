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

export interface AccessConsequencesProps {
  /** The policy as the operator has it set right now, if they have chosen. */
  policy?: string;
  /** Whether they have said this instance publishes beyond a world. */
  publishes: boolean;
}

export function AccessConsequences({
  policy,
  publishes,
}: AccessConsequencesProps) {
  return (
    <section className="grid gap-3" data-testid="setup-access-consequences">
      <ul className="grid gap-2">
        {CONSEQUENCES.map((option) => (
          <li
            key={option.value}
            data-testid={`setup-access-consequence-${option.value}`}
            data-chosen={policy === option.value ? "true" : "false"}
            className={
              policy === option.value
                ? "grid gap-1 rounded-lg border border-ring bg-primary/5 p-3 text-sm"
                : "grid gap-1 rounded-lg border border-border p-3 text-sm"
            }
          >
            <span className="flex items-center gap-2 font-semibold">
              {option.label}
              {option.value === RECOMMENDED ? (
                <StatusBadge variant="info">Recommended</StatusBadge>
              ) : null}
            </span>
            <span className="text-muted-foreground">{option.detail}</span>
          </li>
        ))}
      </ul>

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
    </section>
  );
}
