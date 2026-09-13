import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { ImportReview, type ApprovedImport } from "@/components/import";
import {
  findBookByHash,
  ImportAbandoned,
  submitImport,
  type Compendium,
  type ImportProgress,
} from "@/api/compendium";
import {
  getGameSystemManifest,
  listGameSystems,
  type GameSystemSummary,
} from "@/api/gameSystems";
import type { ContentPatterns } from "@/engine/sdk/ContentPatterns";
import { hashOf } from "@/services/bookImport";
import { clientVersion } from "@/services/feedbackContext";

/**
 * The route into the importer, and the only one (spec 049 FR-028, T032).
 *
 * The review component has existed since the previous phase with nothing
 * wired to it, deliberately: a book is read into an **account's** library, so
 * a world panel would have been the wrong door. This is the right one — the
 * person's own shelf, reached only by that person, with no account named
 * anywhere in the request because the account is whoever is signed in.
 *
 * # What happens before the book is opened
 *
 * The file is hashed and the shelf is asked whether it already holds it
 * (FR-047), **before** the read and long before any upload. A hash is not
 * content, and a Game Master who already owns this book should find out in a
 * moment rather than after a two-minute read. The check is against their
 * library rather than a world, which is the duplication this whole arc
 * exists to stop: eight worlds, one Monster Manual.
 *
 * # Why a system must be chosen first
 *
 * The readers are declaration-driven: what a book yields is whatever the
 * chosen system's pack declares to look for. A system that declares nothing
 * is refused here by name rather than run with another system's vocabulary,
 * which is how a Pathfinder book gets imported as badly-parsed D&D.
 */
export interface ImportBookProps {
  /** Called once a book has been applied, so the shelf can re-read itself. */
  onImported: (compendium: Compendium) => void;
}

/**
 * Where this is up to.
 *
 * A union rather than a handful of booleans, so that "a duplicate is being
 * decided about" and "a book is being reviewed" cannot both be true — the
 * states that would let a file be read while the person is still being asked
 * whether they meant to replace one are not expressible.
 */
type Stage =
  | { phase: "idle" }
  | { phase: "checking"; file: File }
  | { phase: "duplicate"; file: File; held: Compendium }
  | {
      phase: "reviewing";
      file: File;
      patterns: ContentPatterns;
      replaces?: string;
    }
  | { phase: "sending"; progress: ImportProgress | null }
  | { phase: "failed"; reason: string };

export function ImportBook({ onImported }: ImportBookProps) {
  const [systems, setSystems] = useState<GameSystemSummary[]>([]);
  const [systemId, setSystemId] = useState<string>("");
  const [stage, setStage] = useState<Stage>({ phase: "idle" });
  const chooser = useRef<HTMLInputElement>(null);
  const abandon = useRef<AbortController | null>(null);

  useEffect(() => {
    let live = true;
    listGameSystems()
      .then((installed) => {
        if (!live) return;
        setSystems(installed.systems);
        setSystemId(
          (chosen) =>
            chosen || (installed.defaultId ?? installed.systems[0]?.id) || "",
        );
      })
      .catch(() => {
        if (live) setSystems([]);
      });
    return () => {
      live = false;
    };
  }, []);

  /** The patterns the chosen system declares, or a refusal naming the system. */
  const patternsFor = useCallback(
    async (id: string): Promise<ContentPatterns> => {
      const manifest = await getGameSystemManifest(id);
      const declared = manifest.contentPatterns;
      const patterns = Array.isArray(declared) ? declared : [];
      if (patterns.length === 0) {
        throw new Error(
          `${manifest.title} does not say how to find its content in a book, so a book cannot be read into it.`,
        );
      }
      return { patterns } as ContentPatterns;
    },
    [],
  );

  const begin = useCallback(
    async (file: File, replaces?: string) => {
      try {
        setStage({
          phase: "reviewing",
          file,
          patterns: await patternsFor(systemId),
          replaces,
        });
      } catch (cause: unknown) {
        setStage({
          phase: "failed",
          reason:
            cause instanceof Error
              ? cause.message
              : "That system could not be read.",
        });
      }
    },
    [patternsFor, systemId],
  );

  const chosen = useCallback(
    async (file: File) => {
      setStage({ phase: "checking", file });
      try {
        // The hash goes up; the book does not. This is the one thing allowed
        // to cross the wire before a person has submitted anything.
        const held = await findBookByHash(
          await hashOf(new Uint8Array(await file.arrayBuffer())),
        );
        if (held) {
          setStage({ phase: "duplicate", file, held });
          return;
        }
      } catch {
        // A failed check is not a reason to refuse the import: the worst it
        // costs is a duplicate the server's own unique constraint refuses.
      }
      await begin(file);
    },
    [begin],
  );

  const send = useCallback(
    async (approved: ApprovedImport, replaces?: string) => {
      const controller = new AbortController();
      abandon.current = controller;
      setStage({ phase: "sending", progress: null });
      try {
        const compendium = await submitImport(
          {
            title: approved.title,
            sha256: approved.sha256,
            pages: approved.pages,
            silentPages: approved.silentPages,
            systemId,
            // The readers ship inside this bundle, so the build that read the
            // book is the build that is running.
            parserVersion: clientVersion(),
            entries: approved.entries,
            replacesCompendiumId: replaces,
          },
          (progress) => setStage({ phase: "sending", progress }),
          controller.signal,
        );
        setStage({ phase: "idle" });
        onImported(compendium);
      } catch (cause: unknown) {
        // Abandoning is not a failure, and leaves nothing behind (FR-034).
        if (cause instanceof ImportAbandoned) {
          setStage({ phase: "idle" });
          return;
        }
        setStage({
          phase: "failed",
          reason:
            cause instanceof Error
              ? cause.message
              : "The import did not happen.",
        });
      } finally {
        abandon.current = null;
      }
    },
    [onImported, systemId],
  );

  return (
    <section className="grid gap-3" data-testid="import-book">
      <div className="flex flex-wrap items-end gap-3">
        <label className="grid gap-1 text-sm">
          <span className="font-medium">Read it as</span>
          <select
            className="h-9 rounded-md border border-input bg-background px-2 text-sm"
            data-testid="import-system"
            value={systemId}
            onChange={(event) => setSystemId(event.target.value)}
          >
            {/* A value with no option behind it is what React calls an
                uncontrolled select, so the empty choice is a real option
                until the installed systems arrive. */}
            {systems.length === 0 && <option value="">Reading systems</option>}
            {systems.map((system) => (
              <option key={system.id} value={system.id}>
                {system.title}
              </option>
            ))}
          </select>
        </label>

        <input
          ref={chooser}
          type="file"
          accept="application/pdf,.pdf"
          className="hidden"
          data-testid="import-file"
          onChange={(event) => {
            const file = event.target.files?.[0];
            // Cleared so that choosing the same file twice is two events; a
            // Game Master who cancelled a duplicate warning and changed their
            // mind would otherwise have to pick a different file first.
            event.target.value = "";
            if (file) void chosen(file);
          }}
        />
        <Button
          type="button"
          disabled={
            systemId === "" ||
            stage.phase === "checking" ||
            stage.phase === "sending"
          }
          data-testid="import-book-button"
          onClick={() => chooser.current?.click()}
        >
          Read a book in
        </Button>
        <p className="text-sm text-muted-foreground">
          The file is read on this machine. Only what you approve is sent.
        </p>
      </div>

      {stage.phase === "checking" && (
        <p
          className="text-sm text-muted-foreground"
          data-testid="import-checking"
        >
          Checking whether you already have {stage.file.name}.
        </p>
      )}

      {stage.phase === "duplicate" && (
        <div
          className="grid gap-2 rounded-md border p-3"
          data-testid="import-duplicate"
        >
          <p className="text-sm">
            You already have this file on your shelf as{" "}
            <strong>{stage.held.bookTitle}</strong>. Reading it again would
            replace what is there.
          </p>
          <div className="flex gap-2">
            <Button
              type="button"
              variant="secondary"
              data-testid="import-replace"
              onClick={() => void begin(stage.file, stage.held.id)}
            >
              Replace it
            </Button>
            <Button
              type="button"
              variant="ghost"
              data-testid="import-keep"
              onClick={() => setStage({ phase: "idle" })}
            >
              Keep what I have
            </Button>
          </div>
        </div>
      )}

      {stage.phase === "reviewing" && (
        <ImportReview
          file={stage.file}
          patterns={stage.patterns}
          onSubmit={(approved) => void send(approved, stage.replaces)}
          onClose={() => setStage({ phase: "idle" })}
        />
      )}

      {stage.phase === "sending" && (
        <div className="grid gap-2" data-testid="import-sending">
          <p className="text-sm">
            {stage.progress?.phase === "applying"
              ? `Sent — applying ${stage.progress.what}.`
              : stage.progress
                ? `Sending ${stage.progress.what} — ${Math.round(
                    (stage.progress.sentBytes /
                      Math.max(stage.progress.totalBytes, 1)) *
                      100,
                  )}%.`
                : "Starting the upload."}
          </p>
          <div>
            <Button
              type="button"
              variant="ghost"
              data-testid="import-abandon"
              onClick={() => abandon.current?.abort()}
            >
              Abandon
            </Button>
          </div>
        </div>
      )}

      {stage.phase === "failed" && (
        <StatusBadge variant="danger" data-testid="import-failed">
          {stage.reason}
        </StatusBadge>
      )}
    </section>
  );
}
