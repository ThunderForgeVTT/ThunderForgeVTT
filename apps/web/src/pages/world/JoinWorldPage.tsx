import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { postGraphQL } from "@/api/graphqlClient";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useAuth } from "@/hooks/useAuth";
import type { SeoConfig } from "@/types/seo";

interface WorldInfo {
  id: string;
  name: string;
  description?: string;
}

interface JoinResponse {
  world: WorldInfo;
  alreadyMember: boolean;
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
              error instanceof Error
                ? error.message
                : "Failed to load campaign.",
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
          mutation joinWorld($input: JoinWorldInput!) {
            joinWorld(input: $input) {
              id
              role
            }
          }
        `,
        { input: { inviteCode: code } },
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

  const world = worldState.world;

  return (
    <>
      <SEO
        {...joinWorldPageSeo}
        title={
          world ? `Join ${world.name} | ThunderForge` : joinWorldPageSeo.title
        }
      />
      <Container>
        <main className="grid min-h-[60vh] place-items-center py-16">
          {worldState.status ? (
            <Card
              surface="stone"
              className="grid w-full max-w-lg gap-4 p-6 text-center"
            >
              <StatusBadge variant="danger">{worldState.status}</StatusBadge>
              <h1 className="text-2xl font-semibold">
                This link is no longer available
              </h1>
              {/* Spec 027 (FR-011 / SC-005): one message for every dead link.
                  Listing possible causes — invalid, expired, used up, revoked —
                  tells the holder of a killed link which one applied, which is
                  exactly what the server refuses to disclose. */}
              <p className="text-muted-foreground">
                Ask your GM for a new invite link.
              </p>
              <Button onClick={() => navigate("/worlds")}>
                Return to my campaigns
              </Button>
            </Card>
          ) : !world ? (
            <Card
              surface="stone"
              className="grid w-full max-w-lg gap-4 p-6 text-center"
            >
              <h1 className="text-2xl font-semibold">
                This link is no longer available
              </h1>
              <p className="text-muted-foreground">
                Ask your GM for a new invite link.
              </p>
              <Button onClick={() => navigate("/worlds")}>
                Return to my campaigns
              </Button>
            </Card>
          ) : worldState.alreadyMember ? (
            <Card surface="leather" className="grid w-full max-w-lg gap-4 p-6">
              <div>
                <h1 className="text-2xl font-semibold">{world.name}</h1>
                <p className="text-muted-foreground">
                  You are already a member
                </p>
              </div>
              <p className="text-muted-foreground">
                {world.description ?? "A shared realm awaits your return."}
              </p>
              <div className="flex flex-wrap gap-3">
                <Button onClick={() => navigate(`/world/${world.id}`)}>
                  Enter campaign
                </Button>
                <Button variant="secondary" onClick={() => navigate("/worlds")}>
                  Back to campaigns
                </Button>
              </div>
            </Card>
          ) : (
            <Card surface="leather" className="grid w-full max-w-lg gap-4 p-6">
              <div>
                <h1 className="text-2xl font-semibold">{world.name}</h1>
                <p className="text-muted-foreground">Campaign Invitation</p>
              </div>
              <p className="text-muted-foreground">
                {world.description ?? "A shared realm awaits your arrival."}
              </p>
              <div className="flex flex-wrap gap-3">
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
