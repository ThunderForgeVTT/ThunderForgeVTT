import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { withCsrf } from "@/api/auth";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useAuth } from "@/hooks/useAuth";
import type { SeoConfig } from "@/types/seo";
import styles from "./JoinWorldPage.module.scss";

interface WorldInfo {
  id: string;
  name: string;
  description?: string;
}

interface JoinResponse {
  world: WorldInfo;
  alreadyMember: boolean;
}

const GRAPHQL_ENDPOINT = "/api/graphql";

async function postGraphQL<TData>(
  query: string,
  variables?: Record<string, unknown>,
): Promise<TData> {
  const response = await fetch(GRAPHQL_ENDPOINT, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({
      "Content-Type": "application/json",
    }),
    body: JSON.stringify({
      query,
      variables,
    }),
  });

  type GraphQLResponse<T> = {
    data?: T;
    errors?: Array<{ message?: string }>;
  };

  const payload = (await response.json()) as GraphQLResponse<TData>;
  if (!response.ok) {
    throw new Error(payload.errors?.[0]?.message || "GraphQL request failed");
  }

  if (payload.errors?.length) {
    throw new Error(payload.errors[0]?.message || "GraphQL request failed");
  }

  if (!payload.data) {
    throw new Error("GraphQL response did not include data");
  }

  return payload.data;
}

export const joinWorldPageSeo: SeoConfig = {
  title: "Join Campaign",
  description: "Accept an invite and join a campaign in ThunderForge VTT.",
  canonicalPath: "/join",
  noindex: true,
};

/**
 * JoinWorldPage handles joining a world via an invite code.
 *
 * Flow:
 * 1. Extract invite code from URL (/join/:code)
 * 2. Fetch world info to display preview
 * 3. Show world name, description, and join button
 * 4. On join click: call joinWorld(code) mutation
 * 5. On success: redirect to world dashboard
 * 6. On error: show error message
 */
export default function JoinWorldPage() {
  const navigate = useNavigate();
  const { code = "" } = useParams();
  const { isAuthenticated, isLoading: authLoading } = useAuth();
  const [worldState, setWorldState] = useState<{
    world: WorldInfo | null;
    alreadyMember: boolean;
    status: string | null;
    isLoading: boolean;
  }>({
    world: null,
    alreadyMember: false,
    status: null,
    isLoading: true,
  });
  const [isJoining, setIsJoining] = useState(false);

  // When not authenticated, redirect to login
  useEffect(() => {
    if (!authLoading && !isAuthenticated) {
      navigate(`/login?returnTo=${encodeURIComponent(`/join/${code}`)}`);
    }
  }, [authLoading, isAuthenticated, code, navigate]);

  // Fetch world info on mount
  useEffect(() => {
    if (!isAuthenticated || !code) return;

    let active = true;

    const fetchWorld = async () => {
      try {
        const data = await postGraphQL<{
          worldByInviteCode: WorldInfo;
          alreadyMember: boolean;
        }>(
          `
            query worldByInviteCode($code: String!) {
              worldByInviteCode(code: $code) {
                id
                name
                description
              }
              alreadyMember(code: $code)
            }
          `,
          { code },
        );

        if (active) {
          setWorldState({
            world: data.worldByInviteCode,
            alreadyMember: data.alreadyMember,
            status: null,
            isLoading: false,
          });
        }
      } catch (error) {
        if (active) {
          setWorldState({
            world: null,
            alreadyMember: false,
            status:
              error instanceof Error ? error.message : "Failed to load campaign.",
            isLoading: false,
          });
        }
      }
    };

    void fetchWorld();

    return () => {
      active = false;
    };
  }, [isAuthenticated, code]);

  const handleJoin = async () => {
    try {
      setIsJoining(true);

      await postGraphQL(
        `
          mutation joinWorld($code: String!) {
            joinWorld(code: $code) {
              id
              role
            }
          }
        `,
        { code },
      );

      // Redirect to world dashboard
      if (worldState.world) {
        navigate(`/world/${worldState.world.id}`);
      }
    } catch (error) {
      setWorldState((current) => ({
        ...current,
        status:
          error instanceof Error ? error.message : "Failed to join campaign.",
      }));
      setIsJoining(false);
    }
  };

  const isLoading = authLoading || worldState.isLoading;

  if (authLoading) {
    return <Loader fullScreen label="Checking authentication" />;
  }

  if (!isAuthenticated) {
    return null; // Will redirect in useEffect
  }

  if (isLoading) {
    return <Loader fullScreen label="Loading campaign preview" />;
  }

  return (
    <>
      <SEO
        {...joinWorldPageSeo}
        title={
          worldState.world
            ? `Join ${worldState.world.name} | ThunderForge`
            : joinWorldPageSeo.title
        }
      />
      <Container>
        <main className={styles.shell}>
          {worldState.status ? (
            <Card surface="stone" className={styles.errorState}>
              <StatusBadge variant="danger">{worldState.status}</StatusBadge>
              <h1>Could not load campaign</h1>
              <p>This invite code may be invalid, expired, or already used.</p>
              <Button onClick={() => navigate("/worlds")}>
                Return to my campaigns
              </Button>
            </Card>
          ) : !worldState.world ? (
            <Card surface="stone" className={styles.errorState}>
              <h1>Campaign not found</h1>
              <p>This invite code does not exist or has expired.</p>
              <Button onClick={() => navigate("/worlds")}>
                Return to my campaigns
              </Button>
            </Card>
          ) : worldState.alreadyMember ? (
            <Card surface="leather" className={styles.previewCard}>
              <div className={styles.header}>
                <h1>{worldState.world.name}</h1>
                <p className={styles.subtitle}>You are already a member</p>
              </div>
              <p className={styles.description}>
                {worldState.world.description ??
                  "A shared realm awaits your return."}
              </p>
              <div className={styles.actions}>
                <Button onClick={() => navigate(`/world/${worldState.world.id}`)}>
                  Enter campaign
                </Button>
                <Button
                  variant="secondary"
                  onClick={() => navigate("/worlds")}
                >
                  Back to campaigns
                </Button>
              </div>
            </Card>
          ) : (
            <Card surface="leather" className={styles.previewCard}>
              <div className={styles.header}>
                <h1>{worldState.world.name}</h1>
                <p className={styles.subtitle}>Campaign Invitation</p>
              </div>
              <p className={styles.description}>
                {worldState.world.description ??
                  "A shared realm awaits your arrival."}
              </p>
              <div className={styles.actions}>
                <Button
                  onClick={() => void handleJoin()}
                  disabled={isJoining}
                  icon="worlds"
                >
                  {isJoining ? "Joining..." : "Join Campaign"}
                </Button>
                <Button
                  variant="secondary"
                  onClick={() => navigate("/worlds")}
                  disabled={isJoining}
                >
                  Not interested
                </Button>
              </div>
            </Card>
          )}
        </main>
      </Container>
    </>
  );
}
