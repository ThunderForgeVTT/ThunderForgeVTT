import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { getAllWorlds, getMyWorlds } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useAuth } from "@/hooks/useAuth";
import type { SeoConfig } from "@/types/seo";
import type { WorldRecord } from "@/types/world";
import { WorldCard } from "./components/WorldCard";
import styles from "./WorldListPage.module.scss";

export const worldListPageSeo: SeoConfig = {
  title: "World archive",
  description:
    "Browse your ThunderForge worlds, charter new realms, and inspect the guild archive through a fantasy-themed dashboard.",
  canonicalPath: "/worlds",
  noindex: true,
};

export default function WorldListPage() {
  const { isAdmin } = useAuth();
  const [includeAll, setIncludeAll] = useState(false);
  const scopeKey = isAdmin && includeAll ? "all" : "mine";
  const [archiveState, setArchiveState] = useState<{
    requestedScope: string;
    worlds: WorldRecord[];
    status: string | null;
    isLoading: boolean;
  }>({
    requestedScope: scopeKey,
    worlds: [],
    status: null,
    isLoading: true,
  });

  useEffect(() => {
    let active = true;
    const request = isAdmin && includeAll ? getAllWorlds() : getMyWorlds();
    void request
      .then((response) => {
        if (active) {
          setArchiveState({
            requestedScope: scopeKey,
            worlds: response,
            status: null,
            isLoading: false,
          });
        }
      })
      .catch((error) => {
        if (active) {
          setArchiveState({
            requestedScope: scopeKey,
            worlds: [],
            status:
              error instanceof Error
                ? error.message
                : "Failed to load world archive.",
            isLoading: false,
          });
        }
      });

    return () => {
      active = false;
    };
  }, [includeAll, isAdmin, scopeKey]);

  const isLoading =
    archiveState.requestedScope !== scopeKey || archiveState.isLoading;
  const worlds =
    archiveState.requestedScope === scopeKey ? archiveState.worlds : [];
  const status =
    archiveState.requestedScope === scopeKey ? archiveState.status : null;

  return (
    <>
      <SEO {...worldListPageSeo} />
      <Container>
        <main className={styles.shell}>
          <section className={styles.hero}>
            <div>
              <p className={styles.eyebrow}>World archive</p>
              <h1>Chart every realm in your guild ledger.</h1>
              <p>
                Create a new world, revisit an existing one, or inspect the
                wider archive when administrator privileges permit it.
              </p>
            </div>
            <div className={styles.heroActions}>
              <Button asChild icon="quill">
                <Link to="/worlds/create">Create world</Link>
              </Button>
              {isAdmin ? (
                <label className={styles.toggle}>
                  <input
                    type="checkbox"
                    checked={includeAll}
                    onChange={(event) => setIncludeAll(event.target.checked)}
                  />
                  <span>Show every world in the archive</span>
                </label>
              ) : null}
            </div>
          </section>

          {status ? <StatusBadge variant="danger">{status}</StatusBadge> : null}

          {isLoading ? (
            <Loader label="Opening world archive" />
          ) : worlds.length > 0 ? (
            <section className={styles.grid}>
              {worlds.map((world) => (
                <WorldCard
                  key={world.id}
                  world={world}
                  showOwner={Boolean(includeAll && isAdmin)}
                />
              ))}
            </section>
          ) : (
            <Card surface="leather" className={styles.emptyState}>
              <h2>No worlds are chartered yet.</h2>
              <p>
                Begin with a fresh realm and ThunderForge will carry its scenes,
                actors, events, and interface hooks through later phases.
              </p>
              <Button asChild variant="secondary" icon="worlds">
                <Link to="/worlds/create">Found the first world</Link>
              </Button>
            </Card>
          )}
        </main>
      </Container>
    </>
  );
}
