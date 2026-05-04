import { useMemo, useState, type FormEvent } from "react";
import { Link, useNavigate } from "react-router-dom";
import { createWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Container } from "@/components/ui/container/Container";
import { Field } from "@/components/ui/field/Field";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type { SeoConfig } from "@/types/seo";
import styles from "./CreateWorldPage.module.scss";

const GAME_SYSTEM_OPTIONS = [
  { value: "", label: "Choose a placeholder game system" },
  { value: "systemless-sandbox", label: "Systemless Sandbox" },
  { value: "dnd5e-preview", label: "Fifth Age Preview" },
  { value: "pathfinder2e-preview", label: "Second Chronicle Preview" },
] as const;

const INTERFACE_PACK_OPTIONS = [
  { value: "", label: "Choose a placeholder interface pack" },
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
        error instanceof Error ? error.message : "Failed to found world.",
      );
      setIsSaving(false);
    }
  };

  return (
    <>
      <SEO {...createWorldPageSeo} />
      <Container narrow>
        <main className={styles.shell}>
          <section className={styles.hero}>
            <p className={styles.eyebrow}>Realm charter</p>
            <h1>Found a new world inside the ThunderForge atlas.</h1>
            <p>
              Give the realm a name, record its first lore, and bind placeholder
              contracts for its future game system and interface pack.
            </p>
          </section>

          <form
            className={styles.formPanel}
            onSubmit={(event) => void handleSubmit(event)}
          >
            <div className={styles.formHeader}>
              <div>
                <p className={styles.sectionKicker}>Parchment seal</p>
                <h2>World metadata</h2>
              </div>
              <Button asChild variant="ghost" icon="arrow-left">
                <Link to="/worlds">Back to archive</Link>
              </Button>
            </div>

            <div className={styles.fieldStack}>
              <Field
                label="World name"
                htmlFor="world-name"
                hint="Use 3-64 characters. Spaces and simple punctuation are welcome."
              >
                <input
                  id="world-name"
                  className={styles.input}
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
                <textarea
                  id="world-description"
                  className={styles.textarea}
                  name="description"
                  value={description}
                  onChange={(event) => setDescription(event.target.value)}
                  placeholder="A rain-soaked kingdom of fractured banners, hidden sigils, and long-buried oaths."
                  maxLength={600}
                  rows={5}
                />
              </Field>
            </div>

            <div className={styles.selectGrid}>
              <Field
                label="Game system"
                htmlFor="world-game-system"
                hint="Placeholder until deeper system contracts arrive."
              >
                <select
                  id="world-game-system"
                  className={styles.select}
                  value={gameSystemId}
                  onChange={(event) => setGameSystemId(event.target.value)}
                >
                  {GAME_SYSTEM_OPTIONS.map((option) => (
                    <option key={option.value || "blank"} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </Field>

              <Field
                label="Interface pack"
                htmlFor="world-interface-pack"
                hint="Placeholder until Phase 3.5 ships the full selector."
              >
                <select
                  id="world-interface-pack"
                  className={styles.select}
                  value={interfacePackId}
                  onChange={(event) => setInterfacePackId(event.target.value)}
                >
                  {INTERFACE_PACK_OPTIONS.map((option) => (
                    <option key={option.value || "blank"} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </Field>
            </div>

            {status ? (
              <StatusBadge variant="danger">{status}</StatusBadge>
            ) : null}

            <div className={styles.footer}>
              <p>
                Ownership is bound automatically to your authenticated session
                at creation time.
              </p>
              <Button
                type="submit"
                icon="spark"
                className={styles.submitButton}
                disabled={isSaving}
              >
                {isSaving ? "Binding the realm..." : "Create world"}
              </Button>
            </div>
          </form>
        </main>
      </Container>
    </>
  );
}
