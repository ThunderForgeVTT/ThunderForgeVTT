import { useEffect, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { useNavigate, useParams } from "react-router-dom";
import { createActor, getActor, updateActor } from "@/api/actors";
import { getWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { Loader } from "@/components/ui/loader/Loader";
import { Textarea } from "@/components/ui/textarea";
import { ActorImageryPanel } from "@/pages/world/actor/ActorImageryPanel";
import type { WorldActorRecord } from "@/types/actor";
import type { WorldRecord } from "@/types/world";

/**
 * Spec 031 (T068/T070, FR-035/FR-036): authoring an NPC.
 *
 * # Why a page and not a row of boxes under the list
 *
 * The playtest complaint was concrete: creating an NPC meant filling in two
 * cramped inputs wedged beneath a table, with no room for anything else and no
 * moment where the Game Master decided they were finished. Everything an NPC
 * might gain — a description worth reading, a portrait, a token — needs space
 * the list does not have. So creation moved here, behind an explicit Save, and
 * the compendium tab went back to being a list with a link.
 *
 * # Why creating navigates rather than staying put
 *
 * Imagery is stored against an actor id (ADR-057), so there is nothing to
 * upload against until the actor exists. Saving a new NPC therefore lands on
 * this same page in edit mode, where the imagery panel is waiting — rather
 * than holding image bytes through a creation that might be refused.
 */
export interface NpcEditorPageProps {
  mode: "create" | "edit";
}

export default function NpcEditorPage({ mode }: NpcEditorPageProps) {
  const { id: worldId = "", actorId = "" } = useParams();
  const navigate = useNavigate();
  const [world, setWorld] = useState<WorldRecord | null>(null);
  const [actor, setActor] = useState<WorldActorRecord | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [label, setLabel] = useState("");
  const [description, setDescription] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  // Reset during render rather than at the top of the effect below: this is
  // state derived from the arguments, and doing it in the effect commits one
  // render pairing the new key with the previous key's data.
  useResetOnChange(`${worldId}|${actorId}|${mode}`, () => {
    setIsLoading(true);
  });

  useEffect(() => {
    let active = true;

    Promise.all([
      getWorld(worldId),
      mode === "edit" ? getActor(worldId, actorId) : Promise.resolve(null),
    ])
      .then(([worldResult, actorResult]) => {
        if (!active) {
          return;
        }
        setWorld(worldResult);
        setActor(actorResult);
        if (actorResult) {
          setLabel(actorResult.label);
          setDescription(actorResult.description ?? "");
        }
      })
      .finally(() => {
        if (active) {
          setIsLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [worldId, actorId, mode]);

  const handleSave = async () => {
    const trimmed = label.trim();
    if (!trimmed) {
      setStatus("An NPC needs a name.");
      return;
    }
    setIsSaving(true);
    setStatus(null);
    try {
      if (mode === "create") {
        const created = await createActor({
          worldId,
          label: trimmed,
          isNpc: true,
          description: description.trim() || undefined,
        });
        navigate(`/world/${worldId}/compendium/npc/${created.id}/edit`, {
          replace: true,
        });
        return;
      }
      const updated = await updateActor({
        actorId,
        label: trimmed,
        description: description.trim(),
      });
      setActor(updated);
      setStatus("Saved.");
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to save NPC");
    } finally {
      setIsSaving(false);
    }
  };

  if (isLoading) {
    return <Loader fullScreen label="Loading NPC" />;
  }

  if (mode === "edit" && !actor) {
    return (
      <Container>
        <main className="grid min-h-[60vh] place-items-center py-16">
          <Card className="grid gap-3 p-6 text-center">
            <h1 className="text-xl font-semibold">NPC not found</h1>
            <p className="text-muted-foreground">
              This NPC doesn't exist or you don't have access to it.
            </p>
          </Card>
        </main>
      </Container>
    );
  }

  // A Viewer sees the fields read-only. The server refuses the write either
  // way (Constitution Principle III) — this only decides what is offered.
  const canEdit = mode === "create" || actor?.myPermissionLevel !== "VIEWER";

  return (
    <>
      <SEO
        title={
          mode === "create"
            ? `New NPC — ${world?.name ?? "World"}`
            : `${actor?.label ?? "NPC"} — Edit`
        }
        description="NPC authoring"
        noindex
      />
      <Container className="grid max-w-2xl gap-6 py-10">
        <Button
          variant="ghost"
          size="sm"
          icon="arrow-left"
          className="justify-self-start"
          onClick={() => navigate(`/world/${worldId}/compendium`)}
          data-testid="npc-editor-back"
        >
          Back to Compendium
        </Button>

        <Card className="grid gap-4 p-5" data-testid="npc-editor-page">
          <h1 className="text-xl font-semibold">
            {mode === "create" ? "New NPC" : `Edit ${actor?.label}`}
          </h1>

          <Field label="Name" htmlFor="npc-editor-name">
            <Input
              id="npc-editor-name"
              value={label}
              onChange={(event) => setLabel(event.target.value)}
              disabled={!canEdit || isSaving}
              placeholder="Bandit captain"
              data-testid="npc-editor-name-input"
            />
          </Field>

          <Field
            label="Description"
            htmlFor="npc-editor-description"
            hint="What the table sees or hears about them."
          >
            <Textarea
              id="npc-editor-description"
              value={description}
              onChange={(event) => setDescription(event.target.value)}
              disabled={!canEdit || isSaving}
              rows={5}
              data-testid="npc-editor-description-input"
            />
          </Field>

          {canEdit ? (
            <Button
              type="button"
              icon="skull"
              className="justify-self-start"
              disabled={isSaving || !label.trim()}
              onClick={() => void handleSave()}
              data-testid="npc-editor-save"
            >
              {mode === "create" ? "Create NPC" : "Save changes"}
            </Button>
          ) : null}

          {status ? (
            <p
              className="text-sm text-muted-foreground"
              data-testid="npc-editor-status"
            >
              {status}
            </p>
          ) : null}
        </Card>

        {mode === "edit" && actor ? (
          <ActorImageryPanel
            worldId={worldId}
            actorId={actor.id}
            canEdit={canEdit}
          />
        ) : (
          <p
            className="text-sm text-muted-foreground"
            data-testid="npc-editor-imagery-hint"
          >
            Save the NPC first, then add a portrait and a token image.
          </p>
        )}
      </Container>
    </>
  );
}
