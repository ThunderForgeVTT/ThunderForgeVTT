import { Link } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { useAuth } from "@/hooks/useAuth";
import type { SeoConfig } from "@/types/seo";

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
        <main className="grid gap-8 py-8">
          <section className="grid gap-3">
            <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              Welcome
            </p>
            <h1 className="text-3xl font-semibold">
              Welcome back to ThunderForge.
            </h1>
            <p className="max-w-2xl text-muted-foreground">
              {user?.username ?? "Welcome"}, your next step begins here.
              Choose a world to enter, create a fresh world, or gather your
              party through an invite code.
            </p>
          </section>

          <section className="grid gap-4 sm:grid-cols-3">
            <Card surface="parchment" className="grid gap-3 p-6">
              <h2 className="text-lg font-semibold">Enter a world</h2>
              <p className="text-muted-foreground">
                Resume collaborative scenecraft inside your world index.
              </p>
              <Button asChild icon="worlds">
                <Link to="/worlds">Enter a World</Link>
              </Button>
            </Card>

            <Card surface="leather" className="grid gap-3 p-6">
              <h2 className="text-lg font-semibold">Create a world</h2>
              <p className="text-muted-foreground">
                Start a fresh tabletop chapter from the dashboard.
              </p>
              <Button asChild variant="secondary" icon="quill">
                <Link to="/worlds/create">Create a World</Link>
              </Button>
            </Card>

            <Card surface="stone" className="grid gap-3 p-6">
              <h2 className="text-lg font-semibold">Join via invite code</h2>
              <p className="text-muted-foreground">
                Use the existing dashboard chrome while invite flows are
                staged.
              </p>
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
