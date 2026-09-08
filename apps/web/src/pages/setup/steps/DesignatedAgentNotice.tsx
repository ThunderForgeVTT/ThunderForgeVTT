/**
 * Spec 039 FR-055, reached from spec 040 T073.
 *
 * Registering a designated agent — with the US Copyright Office, or whatever
 * the operator's own jurisdiction requires — is a thing a **person** does with
 * a government, on their own account, for a fee. ThunderForge does not do it,
 * cannot do it, and must not leave an operator with the impression that
 * filling in a contact form here has done it. Publishing an address on a page
 * is not registration, and an operator who believes otherwise loses the safe
 * harbour they think they have.
 *
 * So this is stated on the step that collects the notice contact, in the
 * operator's own words rather than in a footnote, and it is deliberately not
 * dismissible.
 *
 * It renders from the *settings* on the step (any `notice.` key), never from a
 * step index or a step name — the wizard has no hard-coded steps to hang it
 * on, and a step named "notices" is not guaranteed to exist. See
 * `SettingsStep`'s `mentionsNoticeContact`.
 */
export function DesignatedAgentNotice() {
  return (
    <aside
      data-testid="setup-designated-agent-notice"
      className="grid gap-2 rounded-lg border border-amber-500/40 bg-amber-500/5 p-4 text-sm"
    >
      <h3 className="font-semibold">
        Registering a designated agent is your job, not this software&rsquo;s
      </h3>
      <p className="text-muted-foreground">
        Where your jurisdiction requires a designated agent for copyright
        notices — in the United States, a registration with the Copyright Office
        under 17 U.S.C. § 512(c)(2), which carries a fee and must be renewed —
        that registration is the operator&rsquo;s own legal obligation.
        ThunderForge does not perform it, does not file anything on your behalf,
        and cannot tell you whether you need it.
      </p>
      <p className="text-muted-foreground">
        What the address you enter here does is publish a contact on this
        instance&rsquo;s pages so a notice can reach you. That is not the same
        thing as being registered, and it does not by itself give you a safe
        harbour. If you are unsure whether your jurisdiction requires a
        registration, ask somebody qualified to tell you.
      </p>
    </aside>
  );
}
