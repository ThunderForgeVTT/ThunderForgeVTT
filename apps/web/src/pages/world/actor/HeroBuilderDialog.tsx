import { useEffect, useId, useMemo, useState } from "react";
import {
  HeroBuilder,
  heroFiles,
  saveHeroFile,
} from "@thunderforge/hero-builder";
import { matchRace, type HeroSpec, type RaceKey } from "@thunderforge/heroes";
import {
  getActorImageSpecs,
  type ActorImageRecord,
  type ActorImageSpec,
} from "@/api/actors";
import { getGameSystemManifest } from "@/api/gameSystems";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button/Button";
import { useActorSystemData } from "@/hooks/useActorSystemData";
import {
  BUILT_ROLES,
  saveBuiltHero,
  savedImages,
  type BuiltHeroSave,
  type BuiltRole,
  type RoleOutcome,
} from "@/pages/world/actor/saveBuiltHero";
import { openingLook, type OpeningLook } from "@/pages/world/actor/storedLook";
import { raceOnSheet } from "@/utils/raceOnSheet";

/**
 * Spec 044 FR-019, FR-023, FR-025: the hero builder, over the page that
 * opened it.
 *
 * # Why a full-screen dialog and never a route
 *
 * The owner's words: a page of its own makes navigation harder. A Game
 * Master building a face for the NPC they are editing, or for one row of a
 * long list, should close the builder and be exactly where they were. So the
 * URL never changes, and an unsaved hero does not survive a refresh.
 *
 * # Why the hosts load this lazily
 *
 * The builder is every part's drawing and every control generated from the
 * catalogue. The imagery panel and the compendium list are on pages most
 * visits never open a builder from, so they `lazy()` this module (see
 * `heroBuilderLazy.ts`) and none of it loads until "Build look" is pressed
 * (FR-022).
 *
 * # Where the race comes from
 *
 * The sheet, read through the system's declared `appearance.race.source`,
 * turned into a look by `matchRace`. A system with no races, a sheet with no
 * race or a race we have no look for all open on "any". The race narrows the
 * dice and is never written back (contract B5a).
 *
 * # Where the hero comes from
 *
 * The spec stored with the actor's images, the portrait's first (FR-036);
 * read from the server each time the builder opens, so it is the look the
 * server holds and not one this page remembers. A stored spec passes
 * `validateHero` before anything draws it (FR-037); one that fails is shown
 * by field and nothing is redrawn or saved unless the user asks to start
 * again from the name (US6 scenario 4). An actor with no stored spec opens on
 * its name, as in phase (b).
 */
export interface HeroBuilderDialogProps {
  open: boolean;
  /** The world the actor is in; its stored look is read from there. */
  worldId: string;
  onOpenChange(open: boolean): void;
  /** The actor saved to, or null for a hero with no actor yet (Quick NPC's
   *  "Open in builder"), which hands the hero back through `onUse`. */
  actorId: string | null;
  actorLabel: string;
  /** Roles the actor already has an image for; saving says it replaces them. */
  existingRoles: readonly string[];
  /** Every row a save stored, as it is stored. */
  onSaved?(images: ActorImageRecord[]): void;
  /** The hero to open on. Without one, the actor's stored look, or one
   *  named after the actor. */
  initialSpec?: HeroSpec;
  /** For an actor-less hero: the look chosen. */
  onUse?(spec: HeroSpec): void;
}

const MAX_NAME = 80;
const ROLE_LABEL: Record<BuiltRole, string> = {
  portrait: "Portrait",
  token: "Token",
};

/** The race the actor's sheet names, as a look, or null; `undefined` while
 *  it is still being read. */
function useSheetRace(actorId: string | null): RaceKey | null | undefined {
  const { data, loading } = useActorSystemData(actorId ?? "");
  const [race, setRace] = useState<{
    key: string;
    race: RaceKey | null;
  } | null>(null);
  const systemId = data?.game_system_id ?? null;
  const key = `${actorId}|${systemId}|${loading}`;

  useEffect(() => {
    if (!actorId || loading || !systemId) return;
    let active = true;
    const sheet = data as unknown as Record<string, unknown>;
    getGameSystemManifest(systemId)
      .then((manifest) => matchRace(raceOnSheet(manifest, sheet)))
      .catch(() => null)
      .then((found) => {
        if (active) setRace({ key, race: found });
      });
    return () => {
      active = false;
    };
  }, [actorId, loading, systemId, data, key]);

  if (!actorId) return null;
  if (loading) return undefined;
  if (!systemId) return null;
  return race?.key === key ? race.race : undefined;
}

/** The actor's images' stored specs; `undefined` while they are read, and
 *  empty when there is no actor or nothing could be read. */
function useStoredSpecs(
  worldId: string,
  actorId: string | null,
  wanted: boolean,
): readonly ActorImageSpec[] | undefined {
  const key = `${worldId}|${actorId}`;
  const [read, setRead] = useState<{
    key: string;
    images: ActorImageSpec[];
  } | null>(null);

  useEffect(() => {
    if (!actorId || !wanted) return;
    let active = true;
    getActorImageSpecs(worldId, actorId)
      .catch(() => [])
      .then((images) => {
        if (active) setRead({ key, images });
      });
    return () => {
      active = false;
    };
  }, [worldId, actorId, wanted, key]);

  if (!actorId || !wanted) return [];
  return read?.key === key ? read.images : undefined;
}

export default function HeroBuilderDialog({
  open,
  onOpenChange,
  worldId,
  actorId,
  actorLabel,
  existingRoles,
  onSaved,
  initialSpec,
  onUse,
}: HeroBuilderDialogProps) {
  const idPrefix = `hb${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const race = useSheetRace(actorId);
  const stored = useStoredSpecs(worldId, actorId, initialSpec === undefined);
  const [fromName, setFromName] = useState(false);
  const nameSpec = useMemo<HeroSpec>(
    () => ({ name: actorLabel.slice(0, MAX_NAME) || "Hero" }),
    [actorLabel],
  );
  const opening = useMemo<OpeningLook | undefined>(() => {
    if (initialSpec !== undefined) {
      return { source: "name", valid: true, spec: initialSpec, note: null };
    }
    if (fromName) {
      return { source: "name", valid: true, spec: nameSpec, note: null };
    }
    return stored === undefined ? undefined : openingLook(stored, nameSpec);
  }, [initialSpec, fromName, nameSpec, stored]);
  // The hero as the user has changed it; until the first change, the one
  // it opened on. Null while an invalid stored spec is on screen, so nothing
  // can be saved from it.
  const [edited, setEdited] = useState<HeroSpec | null>(null);
  const spec: HeroSpec | null =
    edited ?? (opening?.valid ? opening.spec : null);
  const [confirming, setConfirming] = useState(false);
  const [saving, setSaving] = useState(false);
  const [save, setSave] = useState<BuiltHeroSave | null>(null);

  const replaces = existingRoles.some((role) =>
    (BUILT_ROLES as readonly string[]).includes(role),
  );

  const report = (outcomes: RoleOutcome[]) => {
    const images = savedImages(...outcomes);
    if (images.length > 0) onSaved?.(images);
  };

  const start = async () => {
    if (!actorId || !spec) return;
    setConfirming(false);
    setSaving(true);
    try {
      const result = await saveBuiltHero(actorId, spec);
      setSave(result);
      if (!result.paused) report([result.portrait, result.token]);
      if (
        result.portrait.status === "saved" &&
        result.token.status === "saved"
      ) {
        onOpenChange(false);
      }
    } finally {
      setSaving(false);
    }
  };

  const retry = async (role: BuiltRole) => {
    if (!save) return;
    setSaving(true);
    try {
      const outcome = await save.retry(role);
      const next = { ...save, [role]: outcome };
      next.paused =
        (next.portrait.status === "failed" && next.portrait.paused) ||
        (next.token.status === "failed" && next.token.paused);
      setSave(next);
      report([outcome]);
      if (next.portrait.status === "saved" && next.token.status === "saved") {
        onOpenChange(false);
      }
    } finally {
      setSaving(false);
    }
  };

  const exportFile = (which: "portrait" | "token" | "json") => {
    if (spec) saveHeroFile(heroFiles(spec)[which]);
  };

  const onSave = () => {
    if (replaces && !confirming) setConfirming(true);
    else void start();
  };

  const actions = (
    <div className="grid gap-3" data-testid="hero-dialog-actions">
      {confirming ? (
        <div
          role="alert"
          className="grid gap-2 rounded-md border border-border p-3"
          data-testid="hero-dialog-replace-warning"
        >
          <p>
            {actorLabel} already has art. Saving replaces both the portrait and
            the token.
          </p>
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              onClick={() => void start()}
              data-testid="hero-dialog-confirm-replace"
            >
              Replace portrait and token
            </Button>
            <Button
              type="button"
              variant="ghost"
              onClick={() => setConfirming(false)}
              data-testid="hero-dialog-cancel-replace"
            >
              Keep the current art
            </Button>
          </div>
        </div>
      ) : null}

      {save ? (
        <SaveResults save={save} saving={saving} onRetry={retry} />
      ) : null}

      <div className="flex flex-wrap gap-2">
        {actorId ? (
          <Button
            type="button"
            disabled={saving || confirming}
            onClick={onSave}
            data-testid="hero-dialog-save"
          >
            {saving ? "Saving…" : "Save to this NPC"}
          </Button>
        ) : (
          <Button
            type="button"
            onClick={() => {
              if (spec) onUse?.(spec);
              onOpenChange(false);
            }}
            data-testid="hero-dialog-use"
          >
            Use this look
          </Button>
        )}
        <Button
          type="button"
          variant="secondary"
          onClick={() => exportFile("portrait")}
          data-testid="hero-dialog-export-portrait"
        >
          Export portrait
        </Button>
        <Button
          type="button"
          variant="secondary"
          onClick={() => exportFile("token")}
          data-testid="hero-dialog-export-token"
        >
          Export token
        </Button>
        <Button
          type="button"
          variant="secondary"
          onClick={() => exportFile("json")}
          data-testid="hero-dialog-export-json"
        >
          Export spec
        </Button>
        <Button
          type="button"
          variant="ghost"
          onClick={() => onOpenChange(false)}
          data-testid="hero-dialog-close"
        >
          Close
        </Button>
      </div>
    </div>
  );

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        // Full-screen: the shared dialog is a centred card, and every one of
        // its placement classes is overridden here.
        className="top-0 left-0 flex h-dvh w-screen max-w-none translate-x-0 translate-y-0 flex-col gap-3 overflow-y-auto rounded-none p-4 sm:max-w-none"
        data-testid="hero-builder-dialog"
        data-opened-from={opening?.source}
      >
        <div className="grid gap-1 pr-10">
          <DialogTitle>Build look — {actorLabel}</DialogTitle>
          <DialogDescription>
            A portrait and a token, drawn from the parts below.
          </DialogDescription>
        </div>
        {race === undefined || opening === undefined ? (
          <p
            className="text-sm text-muted-foreground"
            data-testid="hero-dialog-loading"
          >
            Reading the sheet…
          </p>
        ) : (
          <>
            {opening.note ? (
              <p
                role="status"
                className="rounded-md border border-border p-3 text-sm"
                data-testid="hero-dialog-stored-note"
              >
                {opening.note}
              </p>
            ) : null}
            {opening.valid ? null : (
              <div
                className="grid gap-2 rounded-md border border-destructive p-3 text-sm"
                data-testid="hero-dialog-stored-invalid"
              >
                <p>
                  The saved look no longer draws: the parts named below have
                  changed since it was saved. Nothing has been redrawn, and the
                  stored portrait and token are as they were.
                </p>
                <div className="flex flex-wrap gap-2">
                  <Button
                    type="button"
                    variant="secondary"
                    onClick={() => setFromName(true)}
                    data-testid="hero-dialog-start-from-name"
                  >
                    Start again from {nameSpec.name}
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    onClick={() => onOpenChange(false)}
                    data-testid="hero-dialog-close"
                  >
                    Close
                  </Button>
                </div>
              </div>
            )}
            <HeroBuilder
              key={`${opening.source}|${fromName}`}
              // An invalid stored spec goes in as it is: the builder
              // validates it again, draws nothing and lists its problems by
              // field (B3).
              initialSpec={opening.spec as HeroSpec}
              idPrefix={idPrefix}
              initialRace={race}
              onChange={(next) => {
                setEdited(next);
                setSave(null);
              }}
              actions={actions}
            />
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}

function SaveResults({
  save,
  saving,
  onRetry,
}: {
  save: BuiltHeroSave;
  saving: boolean;
  onRetry(role: BuiltRole): void;
}) {
  if (save.paused) {
    const outcome = save.portrait;
    return (
      <p
        role="alert"
        className="text-sm text-destructive"
        data-testid="hero-dialog-paused"
      >
        Nothing was saved.{" "}
        {outcome.status === "failed" ? outcome.message : null} The hero is still
        here: save it once play resumes, or export it.
      </p>
    );
  }
  return (
    <ul className="grid gap-1 text-sm" role="status">
      {BUILT_ROLES.map((role) => {
        const outcome = save[role];
        return (
          <li
            key={role}
            className="flex flex-wrap items-center gap-2"
            data-testid={`hero-dialog-result-${role}`}
            data-status={outcome.status}
          >
            {outcome.status === "saved" ? (
              <span>{ROLE_LABEL[role]} saved.</span>
            ) : (
              <>
                <span className="text-destructive">
                  {ROLE_LABEL[role]} not saved: {outcome.message}
                </span>
                <Button
                  type="button"
                  size="sm"
                  variant="secondary"
                  disabled={saving}
                  onClick={() => onRetry(role)}
                  data-testid={`hero-dialog-retry-${role}`}
                >
                  Retry {ROLE_LABEL[role].toLowerCase()}
                </Button>
              </>
            )}
          </li>
        );
      })}
    </ul>
  );
}
