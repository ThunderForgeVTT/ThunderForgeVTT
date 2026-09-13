import { useEffect, useMemo, useState, type FormEvent } from "react";
import { Link, useNavigate } from "react-router-dom";
import {
  listGameSystems,
  titleFor,
  type GameSystemSummary,
} from "@/api/gameSystems";
import { createWorld } from "@/api/world";
import { myLibrary, type LibraryBook } from "@/pages/library/library";
import { switchOn } from "@/pages/world/compendium/worldBooks";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Container } from "@/components/ui/container/Container";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Textarea } from "@/components/ui/textarea";
import type { SeoConfig } from "@/types/seo";

export const createWorldPageSeo: SeoConfig = {
  title: "Create world",
  description: "Found a new ThunderForge world and jump straight into it.",
  canonicalPath: "/worlds/create",
  noindex: true,
};

export default function CreateWorldPage() {
  const navigate = useNavigate();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  /**
   * What this deployment offers, and what it preselects.
   *
   * Both come from `/api/systems`. This form opened preselecting one bundled
   * system by name, over a hand-kept list of all seven — one
   * system named in shared web code, and one list to keep in step with what
   * is installed. The server reads `packs/systems/` and the realm's
   * configured default, so both answers now come from where they are true
   * (spec 032 T088, T090).
   */
  const [systems, setSystems] = useState<GameSystemSummary[]>([]);
  const [gameSystemId, setGameSystemId] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  /**
   * Books from the Game Master's shelf to switch on as the world is made
   * (spec 050 FR-033).
   *
   * Not a seeding step. The world is created empty and each ticked book is
   * then switched on through `switchOn` — the very call the world's Books tab
   * makes a fortnight later — so "created with books" and "created, then
   * books ticked" are one state reached one way, not two paths to keep
   * agreeing.
   */
  const [shelf, setShelf] = useState<LibraryBook[]>([]);
  const [ticked, setTicked] = useState<string[]>([]);

  useEffect(() => {
    let live = true;

    listGameSystems()
      .then((installed) => {
        if (!live) return;
        setSystems(installed.systems);
        // Only preselect; never overwrite a choice the GM has already made.
        if (installed.defaultId) {
          setGameSystemId((current) => current || installed.defaultId!);
        }
      })
      .catch(() => {
        // A picker with nothing in it is honest about the failure, and the
        // form still creates a world — the server applies the realm default
        // when no system is named, so this failing costs the GM the choice
        // rather than the world.
        if (live) setSystems([]);
      });

    return () => {
      live = false;
    };
  }, []);

  useEffect(() => {
    let live = true;
    myLibrary()
      .then((books) => {
        if (live) setShelf(books);
      })
      // An unreadable shelf offers nothing, and a world is still made: the
      // books can be switched on afterwards from the world, by the same call.
      .catch(() => {
        if (live) setShelf([]);
      });
    return () => {
      live = false;
    };
  }, []);

  // FR-041: only books read as the chosen system are offered. A tick for a
  // book the chosen system no longer matches is not dropped from state but is
  // left out of what is sent, so changing the system back restores it and a
  // request the server would refuse is never made.
  const matchingBooks = useMemo(
    () => shelf.filter((book) => book.systemId === gameSystemId),
    [shelf, gameSystemId],
  );
  const tickedMatching = useMemo(
    () => ticked.filter((id) => matchingBooks.some((book) => book.id === id)),
    [ticked, matchingBooks],
  );

  const descriptionCount = useMemo(
    () => description.trim().length,
    [description],
  );

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setIsSaving(true);
    setStatus(null);

    try {
      // Unset means unset. `prepare_world_input` falls back to the realm's
      // configured default, and a realm with no default makes a world with no
      // system — a state the product handles, unlike a guess made here.
      const world = await createWorld({
        name,
        description,
        gameSystemId: gameSystemId || null,
      });

      // One at a time, and after the world exists, through the same mutation
      // as switching a book on later. A book that is refused does not undo
      // the world: the Game Master lands on the book list instead, where what
      // is actually on is the truth rather than this form's hope.
      let everyBookOn = true;
      for (const compendiumId of tickedMatching) {
        try {
          await switchOn(world.id, compendiumId);
        } catch {
          everyBookOn = false;
        }
      }
      if (!everyBookOn) {
        void navigate(`/world/${world.id}/compendium?tab=books`);
        return;
      }
      // Spec 010: straight to staging (not the canvas, and not the
      // dashboard) — the world now always has a default scene already
      // rendered (FR-004, FR-006), via create_world's atomic transaction
      // (T005), and staging is the new first stop before "Play".
      void navigate(`/world/${world.id}/staging`);
    } catch (error) {
      // FR-011: input stays exactly as the user left it — this catch
      // never clears `name`/`description`, only surfaces the error.
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
              Give the world a name and, if you like, a first description —
              you'll land straight in it once it's created.
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

              <Field
                label="Game system"
                htmlFor="world-system"
                hint="You can change this later from the world's system settings."
              >
                <Select
                  value={gameSystemId || undefined}
                  /*
                   * An empty value is discarded rather than stored.
                   *
                   * No `SelectItem` here has an empty value, so "" can never
                   * be a system a person chose. Radix emits one anyway while
                   * the options are still arriving, and taking it at face
                   * value silently un-picked the preselected system one
                   * render after it was set — the picker read "Select a
                   * system" with the realm default already chosen underneath.
                   */
                  onValueChange={(value) => {
                    if (value) setGameSystemId(value);
                  }}
                  disabled={systems.length === 0}
                >
                  <SelectTrigger id="world-system" aria-label="Game system">
                    {/*
                     * The label is rendered here rather than left to
                     * `SelectValue` to resolve.
                     *
                     * Radix reads the trigger's text from the `SelectItem`
                     * matching the value — and those live inside
                     * `SelectContent`, which is not mounted while the
                     * dropdown is closed. That worked while the options were
                     * a module-level literal available on the first render;
                     * once they arrive from `/api/systems` a moment later,
                     * the trigger kept showing "Select a system" with a
                     * system genuinely selected underneath. Caught by an
                     * e2e, invisible to every unit test.
                     */}
                    <SelectValue placeholder="Select a system">
                      {gameSystemId ? titleFor(systems, gameSystemId) : null}
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    {systems.map((system) => (
                      <SelectItem key={system.id} value={system.id}>
                        {system.title}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
              {matchingBooks.length > 0 && (
                <Field
                  label="Books from your library"
                  htmlFor="world-books"
                  hint="Switched on for this world, not copied into it. You can change this later from the world's Books tab."
                >
                  <ul
                    id="world-books"
                    className="grid gap-2"
                    data-testid="create-world-books"
                  >
                    {matchingBooks.map((book) => (
                      <li key={book.id}>
                        <label className="flex items-center gap-2 text-sm">
                          <input
                            type="checkbox"
                            data-testid={`create-world-book-${book.id}`}
                            checked={ticked.includes(book.id)}
                            onChange={(event) => {
                              const on = event.target.checked;
                              setTicked((current) =>
                                on
                                  ? [...current, book.id]
                                  : current.filter((id) => id !== book.id),
                              );
                            }}
                          />
                          {book.bookTitle}
                        </label>
                      </li>
                    ))}
                  </ul>
                </Field>
              )}
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
