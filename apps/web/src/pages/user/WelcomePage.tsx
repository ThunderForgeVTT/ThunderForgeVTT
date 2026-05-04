import { Link } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { useAuth } from "@/hooks/useAuth";
import type { SeoConfig } from "@/types/seo";
import styles from "./WelcomePage.module.scss";

export const welcomePageSeo: SeoConfig = {
  title: "Welcome",
  description:
    "Return to ThunderForge and choose your next action: enter a world, create one, or join by invite code.",
  canonicalPath: "/welcome",
  noindex: true,
};

export default function WelcomePage() {
  const { user } = useAuth();

  return (
    <>
      <SEO {...welcomePageSeo} />
      <Container>
        <main className={styles.shell}>
          <section className={styles.hero}>
            <p className={styles.eyebrow}>Realm foyer</p>
            <h1>Welcome back to ThunderForge.</h1>
            <p>
              {user?.username ?? "Steward"}, your next ritual begins here.
              Choose a world to enter, create a fresh realm, or gather your
              party through an invite code.
            </p>
          </section>

          <section className={styles.grid}>
            <Card surface="parchment" className={styles.card}>
              <h2>Enter a world</h2>
              <p>Resume collaborative scenecraft inside the current guild atlas.</p>
              <Button asChild icon="worlds">
                <Link to="/world/demo-world">Enter a World</Link>
              </Button>
            </Card>

            <Card surface="leather" className={styles.card}>
              <h2>Create a world</h2>
              <p>Start a fresh tabletop chapter from the dashboard preview shell.</p>
              <Button asChild variant="secondary" icon="quill">
                <Link to="/counter">Create a World</Link>
              </Button>
            </Card>

            <Card surface="stone" className={styles.card}>
              <h2>Join via invite code</h2>
              <p>Use the existing dashboard chrome while invite rituals are staged.</p>
              <Button asChild variant="ghost" icon="spark">
                <Link to="/counter">Join via Invite Code</Link>
              </Button>
            </Card>
          </section>
        </main>
      </Container>
    </>
  );
}
