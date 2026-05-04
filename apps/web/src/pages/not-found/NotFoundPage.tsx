import { Link } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import type { SeoConfig } from "@/types/seo";
import styles from "./NotFoundPage.module.scss";

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
      <Container narrow className={styles.shell}>
        <Card>
          <h1>That page does not exist.</h1>
          <p>
            The route you requested is missing or no longer available. Head back to
            the main ThunderForge flow to continue.
          </p>
          <Link to={destination} className={styles.linkAction}>
            Return to {setupRequired ? "setup" : "login"}
          </Link>
        </Card>
      </Container>
    </>
  );
}
