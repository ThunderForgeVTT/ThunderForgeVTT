import { useEffect, useState, type FormEvent } from "react";
import { Link, Navigate, useNavigate } from "react-router-dom";
import { getMyWorlds } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { Loader } from "@/components/ui/loader/Loader";
import { useAuth } from "@/hooks/useAuth";
import type { SeoConfig } from "@/types/seo";
import type { WorldRecord } from "@/types/world";

export const welcomePageSeo: SeoConfig = {
  title: "Welcome",
  description:
    "Return to ThunderForge and choose your next action: enter a world, create one, or join by invite code.",
  canonicalPath: "/welcome",
  noindex: true,
};

/** T001 (Foundational): fetches the user's world list once on mount — both
 * the zero-world redirect (T006, US1) and the hub's per-world shortcut
 * cards (T021, US3) read from this same fetch, per research.md §2. */
function useMyWorlds() {
  const [worlds, setWorlds] = useState<WorldRecord[] | null>(null);
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    let active = true;
    getMyWorlds()
      .then((result) => {
        if (active) {
          setWorlds(result);
        }
      })
      .catch((err) => {
        if (active) {
          setError(err instanceof Error ? err : new Error(String(err)));
        }
      });
    return () => {
      active = false;
    };
  }, []);

  return { worlds, error };
}

export default function WelcomePage() {
  const { user } = useAuth();
  const navigate = useNavigate();
  const { worlds, error } = useMyWorlds();
  const [inviteCode, setInviteCode] = useState("");

  const handleJoinByCode = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const trimmed = inviteCode.trim();
    if (trimmed) {
      navigate(`/join/${encodeURIComponent(trimmed)}`);
    }
  };

  // T006 (US1)/T010 (US3): a user with zero worlds never sees hub content —
  // straight to the create-world form, no extra click (FR-001). A user
  // whose worlds all failed to load is treated the same as zero for safety
  // rather than risking a stuck loading state.
  if (worlds !== null && worlds.length === 0) {
    return <Navigate to="/worlds/create" replace />;
  }

  if (worlds === null && !error) {
    return <Loader fullScreen label="Loading your worlds" />;
  }

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
              Jump back into one of your worlds, create a fresh one, or
              gather your party through an invite code.
            </p>
          </section>

          {/* T021 (US3): direct, one-click shortcuts into the user's actual
           * worlds — FR-009 — always shown once we know worlds.length > 0
           * (the zero case already redirected away above), regardless of
           * count (FR-001a: never auto-enter, even for exactly one). */}
          <section className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {(worlds ?? []).map((world) => (
              <Card key={world.id} surface="parchment" className="grid gap-3 p-6">
                <h2 className="text-lg font-semibold">{world.name}</h2>
                <p className="text-muted-foreground">
                  {world.description ?? "Jump back into this world."}
                </p>
                <Button asChild icon="worlds">
                  <Link to={`/world/${world.id}/staging`}>Enter {world.name}</Link>
                </Button>
              </Card>
            ))}
          </section>

          <section className="grid gap-4 sm:grid-cols-3">
            <Card surface="parchment" className="grid gap-3 p-6">
              <h2 className="text-lg font-semibold">Browse all worlds</h2>
              <p className="text-muted-foreground">
                See the full list, including ones owned by others you've
                joined.
              </p>
              <Button asChild variant="secondary" icon="worlds">
                <Link to="/worlds">Browse Worlds</Link>
              </Button>
            </Card>

            <Card surface="leather" className="grid gap-3 p-6">
              <h2 className="text-lg font-semibold">Create a world</h2>
              <p className="text-muted-foreground">
                Start a fresh tabletop chapter.
              </p>
              <Button asChild variant="secondary" icon="quill">
                <Link to="/worlds/create">Create a World</Link>
              </Button>
            </Card>

            {/* T016 (US2): real invite-code entry — FR-007 — replaces the
             * dead CTA that used to link to /counter. */}
            <Card surface="stone" className="grid gap-3 p-6">
              <h2 className="text-lg font-semibold">Join via invite code</h2>
              <form onSubmit={handleJoinByCode} className="grid gap-3">
                <Field label="Invite code" htmlFor="welcome-invite-code">
                  <Input
                    id="welcome-invite-code"
                    value={inviteCode}
                    onChange={(event) => setInviteCode(event.target.value)}
                    placeholder="Enter your code"
                  />
                </Field>
                <Button type="submit" variant="ghost" icon="spark">
                  Join via Invite Code
                </Button>
              </form>
            </Card>
          </section>
        </main>
      </Container>
    </>
  );
}
