import { useMemo, useState, type FormEvent } from "react";
import { Link, useNavigate } from "react-router-dom";
import { createWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Container } from "@/components/ui/container/Container";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import type { SeoConfig } from "@/types/seo";

const GAME_SYSTEM_OPTIONS = [
  { value: "systemless-sandbox", label: "Systemless Sandbox" },
  { value: "dnd5e-preview", label: "Fifth Age Preview" },
  { value: "pathfinder2e-preview", label: "Second Chronicle Preview" },
] as const;

const INTERFACE_PACK_OPTIONS = [
  { value: "guild-hall-default", label: "Guild Hall Default" },
  { value: "starlit-vault-preview", label: "Starlit Vault Preview" },
  { value: "emberkeep-tome-preview", label: "Emberkeep Tome Preview" },
] as const;

export const createWorldPageSeo: SeoConfig = {
  title: "Create world",
  description:
    "Found a new ThunderForge world with its core metadata, placeholder game system, and interface pack contract.",
  canonicalPath: "/worlds/create",
  noindex: true,
};

export default function CreateWorldPage() {
  const navigate = useNavigate();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [gameSystemId, setGameSystemId] = useState("");
  const [interfacePackId, setInterfacePackId] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  const descriptionCount = useMemo(
    () => description.trim().length,
    [description],
  );

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setIsSaving(true);
    setStatus(null);

    try {
      const world = await createWorld({
        name,
        description,
        gameSystemId: gameSystemId || null,
        interfacePackId: interfacePackId || null,
      });
      void navigate(`/world/${world.id}`);
    } catch (error) {
      setStatus(
        error instanceof Error ? error.message : "Failed to create world.",
      );
      setIsSaving(false);
    }
  };

  return (
    <>
      <SEO {...createWorldPageSeo} />
      <Container narrow>
        <main className="grid gap-8 pb-16">
          <section className="grid gap-3">
            <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              New world
            </p>
            <h1 className="text-3xl font-semibold">
              Create a new world in your ThunderForge library.
            </h1>
            <p className="text-muted-foreground">
              Give the world a name, record its first description, and bind
              placeholder contracts for its future game system and interface
              pack.
            </p>
          </section>

          <form
            className="grid gap-6 rounded-xl border border-border bg-card p-6"
            onSubmit={(event) => void handleSubmit(event)}
          >
            <div className="flex items-start justify-between gap-4">
              <div>
                <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                  Details
                </p>
                <h2 className="text-xl font-semibold">World metadata</h2>
              </div>
              <Button asChild variant="ghost" icon="arrow-left">
                <Link to="/worlds">Back to worlds</Link>
              </Button>
            </div>

            <div className="grid gap-4">
              <Field
                label="World name"
                htmlFor="world-name"
                hint="Use 3-64 characters. Spaces and simple punctuation are welcome."
              >
                <Input
                  id="world-name"
                  name="name"
                  autoComplete="off"
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  placeholder="The Ember Crown"
                  maxLength={64}
                  required
                />
              </Field>

              <Field
                label="Description"
                htmlFor="world-description"
                hint={`${descriptionCount}/600 characters`}
              >
                <Textarea
                  id="world-description"
                  name="description"
                  value={description}
                  onChange={(event) => setDescription(event.target.value)}
                  placeholder="A rain-soaked kingdom of fractured banners, hidden sigils, and long-buried oaths."
                  maxLength={600}
                  rows={5}
                />
              </Field>
            </div>

            <div className="grid gap-4 sm:grid-cols-2">
              <Field
                label="Game system"
                htmlFor="world-game-system"
                hint="Placeholder until deeper system contracts arrive."
              >
                <Select value={gameSystemId} onValueChange={setGameSystemId}>
                  <SelectTrigger id="world-game-system" className="w-full">
                    <SelectValue placeholder="Choose a placeholder game system" />
                  </SelectTrigger>
                  <SelectContent>
                    {GAME_SYSTEM_OPTIONS.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>

              <Field
                label="Interface pack"
                htmlFor="world-interface-pack"
                hint="Placeholder until Phase 3.5 ships the full selector."
              >
                <Select
                  value={interfacePackId}
                  onValueChange={setInterfacePackId}
                >
                  <SelectTrigger id="world-interface-pack" className="w-full">
                    <SelectValue placeholder="Choose a placeholder interface pack" />
                  </SelectTrigger>
                  <SelectContent>
                    {INTERFACE_PACK_OPTIONS.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
            </div>

            {status ? (
              <StatusBadge variant="danger">{status}</StatusBadge>
            ) : null}

            <div className="flex flex-wrap items-center justify-between gap-4 border-t border-border pt-4">
              <p className="text-sm text-muted-foreground">
                Ownership is bound automatically to your authenticated session
                at creation time.
              </p>
              <Button type="submit" icon="spark" disabled={isSaving}>
                {isSaving ? "Creating world..." : "Create world"}
              </Button>
            </div>
          </form>
        </main>
      </Container>
    </>
  );
}
