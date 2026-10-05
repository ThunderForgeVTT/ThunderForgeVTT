import { useState, type FormEvent } from "react";
import { Link } from "react-router-dom";
import { createActor, setActorAvailability } from "@/api/actors";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Input } from "@/components/ui/input";
import type { WorldActorRecord } from "@/types/actor";

type NewCharacterCardProps = {
  worldId: string;
  /** Called once the character exists, so the page's pickers can list it. */
  onCreated: (actor: WorldActorRecord) => void;
};

/**
 * A character for a player to claim, made where the players are.
 *
 * There was no way to make one. Every screen that created an actor created
 * an NPC; a Game Master preparing a table made a creature in the compendium,
 * opened its edit page to say it was a player character, saved, then opened
 * its view page to offer it. Three screens for one sentence — and in a game
 * whose whole character is a name and "Do Anything 1", that was all of
 * preparing one.
 *
 * It takes the world's system (the server gives a new actor its world's) and
 * is offered for claiming at once, because that is what it was made for. A
 * character that was made and could not be offered is still made: it is
 * named with a link, so the offer can be finished on its own page.
 */
export function NewCharacterCard({
  worldId,
  onCreated,
}: NewCharacterCardProps) {
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [made, setMade] = useState<WorldActorRecord | null>(null);

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault();
    const label = name.trim();
    if (!label || busy) {
      return;
    }
    setBusy(true);
    setError(null);
    setMade(null);
    try {
      const created = await createActor({ worldId, label, isNpc: false });
      let actor = created;
      try {
        actor = await setActorAvailability(created.id, true);
      } catch (err) {
        setError(
          `Made, but not yet offered: ${
            err instanceof Error ? err.message : "the offer was refused"
          }`,
        );
      }
      setMade(actor);
      setName("");
      onCreated(actor);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to create the character",
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card className="grid gap-3 p-4" data-testid="new-character-card">
      <div className="grid gap-1">
        <h2 className="text-base font-semibold">A character to claim</h2>
        <p className="text-sm text-muted-foreground">
          Name a new player character. A player joining the world can pick it,
          or you can give it to somebody below.
        </p>
      </div>
      <form
        className="flex flex-wrap items-center gap-2"
        onSubmit={(event) => void handleSubmit(event)}
      >
        <Input
          className="min-w-48 flex-1"
          placeholder="Character name"
          value={name}
          onChange={(event) => setName(event.target.value)}
          aria-label="New character's name"
          data-testid="new-character-name"
          maxLength={120}
        />
        <Button
          type="submit"
          disabled={busy || name.trim() === ""}
          data-testid="new-character-submit"
        >
          {busy ? "Creating…" : "Create character"}
        </Button>
      </form>
      {error ? (
        <p
          className="text-sm text-destructive"
          data-testid="new-character-error"
        >
          {error}
        </p>
      ) : null}
      {made ? (
        <p className="text-sm" role="status" data-testid="new-character-made">
          <Link
            className="underline"
            to={`/world/${worldId}/actor/${made.id}/view`}
            data-testid="new-character-link"
          >
            {made.label}
          </Link>{" "}
          {made.availableForClaim ? "is ready to be claimed." : "was created."}
        </p>
      ) : null}
    </Card>
  );
}
