import { useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Grid } from "@/components/ui/grid/Grid";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type { SeoConfig } from "@/types/seo";
import styles from "./CounterPage.module.scss";

export const counterPageSeo: SeoConfig = {
  title: "Dashboard preview",
  description:
    "Preview reusable dashboard surfaces, setup completion state, and code-split navigation targets inside ThunderForge VTT.",
  keywords: ["ThunderForge dashboard", "React dashboard", "virtual tabletop admin"],
  canonicalPath: "/counter",
  prefetchHrefs: ["/world/demo-world"],
};

export default function CounterPage() {
  const [value, setValue] = useState(0);
  const [searchParams] = useSearchParams();
  const bootstrapComplete = searchParams.get("bootstrap") === "complete";
  const insights = useMemo(
    () => [
      {
        title: "Typed routes",
        body: "Each page is lazy-loaded so route-level code stays split and easier to scale.",
      },
      {
        title: "SEO metadata",
        body: "Every page owns its own SEO config while the shared component updates head tags on navigation.",
      },
      {
        title: "Reusable styling",
        body: "UI primitives share SCSS modules, design tokens, and responsive layout helpers.",
      },
    ],
    [],
  );

  return (
    <>
      <SEO {...counterPageSeo} />
      <Container>
        <main className={styles.shell}>
          <section className={styles.hero}>
            <h1>Reusable dashboard surfaces for the web app.</h1>
            <p>
              This page doubles as a lightweight sandbox for app shell components,
              route code-splitting, and the bootstrap completion handoff.
            </p>
          </section>

          {bootstrapComplete ? (
            <StatusBadge variant="success">
              The initial administrator was created successfully.
            </StatusBadge>
          ) : null}

          <Grid columns="two">
            <Card className={styles.counterPanel}>
              <div className={styles.counterRow}>
                <div>
                  <h2>Component demo counter</h2>
                  <p>Keep a small interactive example to validate shared primitives.</p>
                </div>
                <span className={styles.counterValue}>{value}</span>
              </div>
              <div className={styles.counterRow}>
                <Button variant="secondary" onClick={() => setValue(0)}>
                  Reset
                </Button>
                <Button variant="primary" onClick={() => setValue((current) => current + 1)}>
                  Increment
                </Button>
              </div>
            </Card>

            <Card className={styles.stack}>
              <h2>Next experience</h2>
              <p>
                Use the world route to load the collaborative canvas and Bevy engine
                shell inside the new layout system.
              </p>
              <Link to="/world/demo-world" className={styles.worldLink}>
                Open demo world
              </Link>
            </Card>
          </Grid>

          <Grid columns="three">
            {insights.map((insight) => (
              <Card key={insight.title} className={styles.metaList}>
                <h2>{insight.title}</h2>
                <p>{insight.body}</p>
              </Card>
            ))}
          </Grid>
        </main>
      </Container>
    </>
  );
}
