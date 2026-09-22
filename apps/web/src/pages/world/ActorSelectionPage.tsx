import { Suspense, useEffect, useState } from "react";
import type { HeroSpec } from "@thunderforge/heroes";
import { useNavigate, useParams } from "react-router-dom";
import {
  claimActor,
  createAndClaimActor,
  getAvailableActors,
  getMyActorClaim,
} from "@/api/actorClaims";
import { getWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { Loader } from "@/components/ui/loader/Loader";
import { LazyHeroBuilderDialog } from "@/pages/world/actor/heroBuilderLazy";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type { WorldActorRecord } from "@/types/actor";
import type { WorldRecord } from "@/types/world";

/**
 * Spec 017 (US1/US2/US3): `/world/:id/actor-select` — the onboarding gate
 * a non-GM member lands on until they claim a character (FR-001/FR-002).
 * Never reachable by the GM/Owner role (FR-003, enforced by
 * `useActorClaimGate` never redirecting them here in the first place —
 * this page itself doesn't re-check that, since arriving here at all
 * implies the gate already cleared them for it).
 */
export default function ActorSelectionPage() {
  const { id: worldId = "" } = useParams();
  const navigate = useNavigate();
  const [world, setWorld] = useState<WorldRecord | null>(null);
  const [available, setAvailable] = useState<WorldActorRecord[] | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [status, setStatus] = useState<string | null>(null);
  const [isClaiming, setIsClaiming] = useState<string | null>(null);
  const [newName, setNewName] = useState("");
  const [newDescription, setNewDescription] = useState("");
  const [isCreating, setIsCreating] = useState(false);
  /** Spec 044 FR-034: a look built before the character exists. */
  const [look, setLook] = useState<HeroSpec | null>(null);
  const [building, setBuilding] = useState(false);
  /** The character was created but its art was not stored. */
  const [lacksArt, setLacksArt] = useState<{
    actorId: string;
    message: string;
  } | null>(null);

  const loadWorldAndActors = () => {
    setIsLoading(true);
    return Promise.all([getWorld(worldId), getAvailableActors(worldId)])
      .then(([worldResult, actorsResult]) => {
        setWorld(worldResult);
        setAvailable(actorsResult);
      })
      .catch((err) => {
        setStatus(
          err instanceof Error ? err.message : "Failed to load Actor Selection",
        );
      })
      .finally(() => {
        setIsLoading(false);
      });
  };

  useEffect(() => {
    let active = true;

    // A player who already has a claim (e.g. revisiting a stale link, or
    // the gate redirected them here before their claim synced) is sent
    // straight back to the world rather than shown the picker again
    // (FR-002).
    getMyActorClaim(worldId)
      .then((claim) => {
        if (!active) {
          return;
        }
        if (claim) {
          navigate(`/world/${worldId}`, { replace: true });
          return;
        }
        void loadWorldAndActors();
      })
      .catch(() => {
        if (active) {
          void loadWorldAndActors();
        }
      });

    return () => {
      active = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [worldId]);

  const handleClaim = async (actorId: string) => {
    setIsClaiming(actorId);
    setStatus(null);
    try {
      await claimActor(worldId, actorId);
      navigate(`/world/${worldId}`, { replace: true });
    } catch (err) {
      setStatus(
        err instanceof Error ? err.message : "Failed to claim this character",
      );
      // The claim may have been lost to a race — refresh the list so a
      // just-claimed character disappears rather than being offered again.
      void loadWorldAndActors();
    } finally {
      setIsClaiming(null);
    }
  };

  const handleCreate = async () => {
    if (!newName.trim()) {
      setStatus("Enter a name for your character.");
      return;
    }
    setIsCreating(true);
    setStatus(null);
    try {
      const claim = await createAndClaimActor(
        worldId,
        newName.trim(),
        newDescription.trim() || undefined,
      );
      // Spec 044 FR-034: the character first, then its look — the same order
      // and the same reasoning as Quick NPC. A failed upload keeps the
      // character, which is what the player asked for, and says where the
      // art can be added; it never deletes the character to hide the failure.
      if (look) {
        const { saveBuiltHero } =
          await import("@/pages/world/actor/saveBuiltHero");
        const saved = await saveBuiltHero(claim.actorId, {
          ...look,
          name: newName.trim(),
        });
        const failed = [saved.portrait, saved.token].find(
          (outcome) => outcome.status === "failed",
        );
        if (failed?.status === "failed") {
          setLacksArt({ actorId: claim.actorId, message: failed.message });
          return;
        }
      }
      navigate(`/world/${worldId}`, { replace: true });
    } catch (err) {
      setStatus(
        err instanceof Error ? err.message : "Failed to create your character",
      );
    } finally {
      setIsCreating(false);
    }
  };

  if (isLoading) {
    return <Loader fullScreen label="Loading Actor Selection" />;
  }

  if (!world) {
    return (
      <Container>
        <main className="grid min-h-[60vh] place-items-center py-16">
          <Card className="grid gap-3 p-6 text-center">
            <h1 className="text-xl font-semibold">World not found</h1>
            <p className="text-muted-foreground">
              This world doesn't exist or you don't have access to it.
            </p>
          </Card>
        </main>
      </Container>
    );
  }

  const hasAvailable = (available ?? []).length > 0;
  const canCreateOwn = world.allowPlayerCreatedActors;

  return (
    <>
      <SEO
        title={`${world.name} — Choose your character`}
        description="Actor selection"
        noindex
      />
      <Container className="grid max-w-2xl gap-6 py-10">
        <div>
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            {world.name}
          </p>
          <h1 className="text-2xl font-semibold">Choose your character</h1>
          <p className="mt-1 text-muted-foreground">
            Pick a character your GM has prepared,{" "}
            {canCreateOwn
              ? "or create your own."
              : "or wait for your GM to prepare one."}
          </p>
        </div>

        {status ? <StatusBadge variant="danger">{status}</StatusBadge> : null}

        {hasAvailable ? (
          <Card className="grid gap-3 p-6" data-testid="available-actors-list">
            <h2 className="text-sm font-semibold tracking-wide text-muted-foreground uppercase">
              Available characters
            </h2>
            <ul className="grid gap-2">
              {(available ?? []).map((actor) => (
                <li
                  key={actor.id}
                  className="flex items-center justify-between gap-3 rounded-md border border-border p-3"
                  data-testid="available-actor-row"
                >
                  <div>
                    <p className="font-medium">{actor.label}</p>
                    {actor.description ? (
                      <p className="text-sm text-muted-foreground">
                        {actor.description}
                      </p>
                    ) : null}
                  </div>
                  <Button
                    onClick={() => void handleClaim(actor.id)}
                    disabled={isClaiming !== null}
                  >
                    {isClaiming === actor.id ? "Claiming..." : "Select"}
                  </Button>
                </li>
              ))}
            </ul>
          </Card>
        ) : null}

        {canCreateOwn ? (
          <Card className="grid gap-4 p-6" data-testid="create-own-actor-form">
            <h2 className="text-sm font-semibold tracking-wide text-muted-foreground uppercase">
              Create your own character
            </h2>
            <Field label="Name" htmlFor="new-character-name">
              <Input
                id="new-character-name"
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                placeholder="Your character's name"
              />
            </Field>
            <Field
              label="Description (optional)"
              htmlFor="new-character-description"
            >
              <Input
                id="new-character-description"
                value={newDescription}
                onChange={(e) => setNewDescription(e.target.value)}
                placeholder="A short description…"
              />
            </Field>
            {/* Spec 044 FR-034: optional; a character with no look is
                still a character. */}
            <div className="flex flex-wrap items-center gap-3">
              <Button
                type="button"
                variant="secondary"
                icon="quill"
                onClick={() => setBuilding(true)}
                disabled={isCreating || lacksArt !== null}
                data-testid="create-own-build-look"
              >
                {look ? "Change look" : "Build look"}
              </Button>
              <span
                className="text-sm text-muted-foreground"
                data-testid="create-own-look-state"
              >
                {look
                  ? "A portrait and a token will be stored with your character."
                  : "Optional. You can add art later from your character's page."}
              </span>
            </div>
            {lacksArt ? (
              <div
                role="alert"
                className="grid gap-2 text-sm"
                data-testid="create-own-lacks-art"
              >
                <p>
                  Your character was created, but without its art:{" "}
                  {lacksArt.message}
                </p>
                <p>
                  Open your character&apos;s page and use “Build look” to add a
                  portrait and a token.
                </p>
                <div className="flex flex-wrap gap-2">
                  <Button
                    type="button"
                    onClick={() =>
                      navigate(
                        `/world/${worldId}/actor/${lacksArt.actorId}/view`,
                        { replace: true },
                      )
                    }
                    data-testid="create-own-open-character"
                  >
                    Open my character
                  </Button>
                  <Button
                    type="button"
                    variant="secondary"
                    onClick={() =>
                      navigate(`/world/${worldId}`, { replace: true })
                    }
                  >
                    Continue to the world
                  </Button>
                </div>
              </div>
            ) : null}
            <Button
              className="justify-self-start"
              onClick={() => void handleCreate()}
              disabled={isCreating || lacksArt !== null}
              data-testid="create-own-submit"
            >
              {isCreating ? "Creating..." : "Create and play as this character"}
            </Button>
          </Card>
        ) : null}

        {building ? (
          <Suspense fallback={null}>
            <LazyHeroBuilderDialog
              open
              worldId={worldId}
              onOpenChange={setBuilding}
              actorId={null}
              actorLabel={newName.trim() || "Your character"}
              existingRoles={[]}
              initialSpec={
                look
                  ? { ...look, name: newName.trim() || look.name }
                  : undefined
              }
              onUse={(chosen) => {
                setLook(chosen);
                if (!newName.trim() && chosen.name.trim()) {
                  setNewName(chosen.name);
                }
              }}
            />
          </Suspense>
        ) : null}

        {!hasAvailable && !canCreateOwn ? (
          <Card
            className="grid gap-2 p-6 text-center"
            data-testid="waiting-for-gm"
          >
            <h2 className="text-lg font-semibold">No characters ready yet</h2>
            <p className="text-muted-foreground">
              Ask your GM to mark a character as available for you to claim, or
              to turn on player-created characters. You're already a member of
              this world — you'll be able to jump in as soon as one of those
              happens.
            </p>
          </Card>
        ) : null}
      </Container>
    </>
  );
}
