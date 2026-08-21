import { Link } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import type { SeoConfig } from "@/types/seo";

interface NotFoundPageProps {
  setupRequired: boolean;
}

export const notFoundPageSeo: SeoConfig = {
  title: "Page not found",
  description:
    "The requested ThunderForge VTT page could not be found. Return to the main setup or login experience.",
  canonicalPath: "/404",
  noindex: true,
};

export default function NotFoundPage({ setupRequired }: NotFoundPageProps) {
  const destination = setupRequired ? "/setup" : "/login";

  return (
    <>
      <SEO {...notFoundPageSeo} />
      <Container narrow className="grid min-h-screen place-items-center">
        <Card surface="parchment" className="grid max-w-md gap-4 p-8 text-center">
          <h1 className="text-2xl font-semibold">
            That page does not exist.
          </h1>
          <p className="text-muted-foreground">
            The route you requested is missing or no longer available. Head
            back to the main ThunderForge flow to continue.
          </p>
          <Button asChild icon="arrow-left" className="justify-self-center">
            <Link to={destination}>
              Return to {setupRequired ? "setup" : "login"}
            </Link>
          </Button>
        </Card>
      </Container>
    </>
  );
}
