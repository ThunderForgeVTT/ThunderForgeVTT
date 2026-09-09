import { SEO } from "@/components/seo/SEO";
import { MySubmissions } from "@/components/feedback/MySubmissions";
import { Container } from "@/components/ui/container/Container";
import type { SeoConfig } from "@/types/seo";

export const feedbackSettingsPageSeo: SeoConfig = {
  title: "Feedback you have sent",
  description:
    "What you have reported to this instance, and what became of it.",
  canonicalPath: "/settings/feedback",
  noindex: true,
};

/**
 * Spec 037 US5 (FR-016): the account's own view of what it has reported.
 *
 * Its own page rather than a card on the security screen, because those are
 * different questions — one is "how does this account prove it is mine", the
 * other is "what did I tell these people and what happened to it". Sharing a
 * screen would make both harder to find.
 */
export function FeedbackSettingsPage() {
  return (
    <>
      <SEO {...feedbackSettingsPageSeo} />
      <Container>
        <div className="grid gap-6 py-8">
          <header className="grid gap-1">
            <h1 className="text-2xl font-semibold">Feedback you have sent</h1>
            <p className="text-sm text-muted-foreground">
              Everything here reached this instance and is kept. Where it went
              afterwards, if anywhere, is shown alongside it.
            </p>
          </header>
          <MySubmissions />
        </div>
      </Container>
    </>
  );
}

export default FeedbackSettingsPage;
