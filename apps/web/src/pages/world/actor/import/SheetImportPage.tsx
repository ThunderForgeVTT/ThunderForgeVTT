/**
 * Spec 048: bringing a character in from a sheet. The browser reads the
 * file with the system's own reader and asks the server what it would
 * change; nothing is uploaded until the player accepts the review.
 */
import { useEffect, useState, type ChangeEvent } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { getActor } from "@/api/actors";
import type { SheetCorrections, SheetImportPlan } from "@/api/sheetImport";
import { WorldAppearance } from "@/appearance/WorldAppearance";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { Loader } from "@/components/ui/loader/Loader";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { useSheetImport } from "@/hooks/useSheetImport";
import { mayEditActor } from "@/pages/world/actor/actorEditRight";
import { readSheet } from "@/pages/world/actor/systemSheetReaders";
import type { WorldActorRecord } from "@/types/actor";
import { ContentRow } from "./ContentRow";
import { FieldRow } from "./FieldRow";
import { CrossChecks, KeptInPlayList, UnmappedList } from "./PlanNotes";
import { refusalProblem, SheetRefused, type Problem } from "./refusal";
import { filterFields, type FieldFilter } from "./rows";

/** What the person is reviewing: the reading, their corrections, the plan. */
interface Review {
  file: File;
  reading: unknown;
  corrections: SheetCorrections;
  overwrite: string[];
  plan: SheetImportPlan;
}

type Step =
  | { kind: "pick" }
  | { kind: "reading" }
  | ({ kind: "review"; busy: boolean } & Review)
  | ({ kind: "applying"; sent: number } & Review);

const FILTERS: { id: FieldFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "uncertain", label: "Check this" },
  { id: "unread", label: "Not read" },
];

export default function SheetImportPage() {
  const { id: worldId = "", actorId = "" } = useParams();
  const navigate = useNavigate();
  const [actor, setActor] = useState<WorldActorRecord | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [step, setStep] = useState<Step>({ kind: "pick" });
  const [problem, setProblem] = useState<Problem | null>(null);
  const [filter, setFilter] = useState<FieldFilter>("all");
  const { preview, apply } = useSheetImport(actorId);
  const viewPath = `/world/${worldId}/actor/${actorId}/view`;

  useResetOnChange(`${worldId}|${actorId}`, () => {
    setIsLoading(true);
    setStep({ kind: "pick" });
    setProblem(null);
    setFilter("all");
  });

  useEffect(() => {
    let active = true;
    getActor(worldId, actorId)
      .then((found) => {
        if (active) setActor(found);
      })
      .catch(() => {
        if (active) setActor(null);
      })
      .finally(() => {
        if (active) setIsLoading(false);
      });
    return () => {
      active = false;
    };
  }, [worldId, actorId]);

  if (isLoading) return <Loader fullScreen label="Loading actor" />;
  if (!actor || !mayEditActor(actor)) {
    return (
      <Container className="grid max-w-2xl gap-4 py-10">
        <p data-testid="sheet-import-refused">
          You may not bring a sheet onto this character.
        </p>
        <Link to={viewPath}>Back to the character</Link>
      </Container>
    );
  }

  const onFile = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    setProblem(null);
    setStep({ kind: "reading" });
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const answer = await readSheet(actor.gameSystemId, bytes);
      if (!answer) {
        throw new Error("This game system cannot read a character sheet.");
      }
      if (!answer.recognised || answer.reading === undefined) {
        throw new SheetRefused(
          answer.error ?? "This is not a sheet we can read.",
          answer.code,
        );
      }
      const plan = await preview(answer.reading);
      setFilter("all");
      setStep({
        kind: "review",
        busy: false,
        file,
        reading: answer.reading,
        corrections: {},
        overwrite: [],
        plan,
      });
    } catch (err) {
      setProblem(refusalProblem(err));
      setStep({ kind: "pick" });
    }
  };

  /** A correction asks for the plan again; the server marks it CORRECTED. */
  const onCorrect = async (path: string, value: unknown) => {
    if (step.kind !== "review") return;
    const review: Review = step;
    const corrections = { ...review.corrections, [path]: value };
    setProblem(null);
    setStep({ ...review, kind: "review", busy: true });
    try {
      const plan = await preview(review.reading, corrections);
      const kept = new Set(plan.keptInPlay.map((row) => row.target));
      setStep({
        ...review,
        kind: "review",
        busy: false,
        corrections,
        overwrite: review.overwrite.filter((target) => kept.has(target)),
        plan,
      });
    } catch (err) {
      setProblem(refusalProblem(err));
      setStep({ ...review, kind: "review", busy: false });
    }
  };

  const onToggleKept = (target: string, on: boolean) => {
    if (step.kind !== "review") return;
    const rest = step.overwrite.filter((t) => t !== target);
    setStep({ ...step, overwrite: on ? [...rest, target] : rest });
  };

  const onAccept = async () => {
    if (step.kind !== "review" || step.busy) return;
    const review: Review = step;
    setProblem(null);
    setStep({ ...review, kind: "applying", sent: 0 });
    try {
      await apply({
        file: review.file,
        corrections: review.corrections,
        overwritePlayState: review.overwrite,
        planHash: review.plan.planHash,
        onProgress: (sent, total) =>
          setStep({
            ...review,
            kind: "applying",
            sent: total > 0 ? Math.round((sent / total) * 100) : 0,
          }),
      });
      navigate(viewPath);
    } catch (err) {
      setProblem(refusalProblem(err));
      setStep({ ...review, kind: "review", busy: false });
    }
  };

  const review =
    step.kind === "review" || step.kind === "applying" ? step : null;
  const plan = review?.plan ?? null;
  const locked =
    step.kind === "applying" || (step.kind === "review" && step.busy);

  return (
    <WorldAppearance worldId={worldId}>
      <SEO
        title={`Bring in a sheet · ${actor.label}`}
        description="Bring a character in from a sheet."
      />
      <Container
        className="grid max-w-2xl gap-6 py-10"
        data-testid="sheet-import-page"
      >
        <header className="grid gap-1">
          <h1 className="text-2xl font-semibold">Bring in a sheet</h1>
          <p className="text-sm text-muted-foreground">
            Onto {actor.label}. Nothing changes until you accept what the sheet
            will write.
          </p>
        </header>

        {problem ? (
          <p
            role="alert"
            className="rounded-lg border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive"
            data-testid="sheet-import-problem"
            data-code={problem.code}
          >
            {problem.text}
          </p>
        ) : null}

        {step.kind === "pick" || step.kind === "reading" ? (
          <Card className="grid gap-3 p-4">
            <Field label="Character sheet (PDF)" htmlFor="sheet-import-file">
              <Input
                id="sheet-import-file"
                type="file"
                accept="application/pdf,.pdf"
                disabled={step.kind === "reading"}
                onChange={(event) => void onFile(event)}
                data-testid="sheet-import-file"
              />
            </Field>
            {step.kind === "reading" ? (
              <p className="text-sm" data-testid="sheet-import-reading">
                Reading the sheet…
              </p>
            ) : null}
          </Card>
        ) : null}

        {plan ? (
          <>
            <Card className="grid gap-2 p-4">
              <h2 className="font-semibold">
                {plan.isReimport ? "What changes" : "What the sheet writes"}
              </h2>
              <div
                className="flex flex-wrap gap-1"
                role="group"
                aria-label="Show"
                data-testid="sheet-import-filters"
              >
                {FILTERS.map(({ id, label }) => (
                  <Button
                    key={id}
                    size="sm"
                    variant={filter === id ? "secondary" : "ghost"}
                    aria-pressed={filter === id}
                    onClick={() => setFilter(id)}
                    data-testid={`sheet-import-filter-${id}`}
                  >
                    {label}
                  </Button>
                ))}
              </div>
              <ul className="grid" data-testid="sheet-import-fields">
                {filterFields(plan.fields, filter).map((field) => (
                  <FieldRow
                    key={field.path}
                    field={field}
                    disabled={locked}
                    onCorrect={(path, value) => void onCorrect(path, value)}
                  />
                ))}
              </ul>
            </Card>
            <CrossChecks checks={plan.crossChecks} />
            <KeptInPlayList
              kept={plan.keptInPlay}
              overwrite={review?.overwrite ?? []}
              onToggle={onToggleKept}
              disabled={locked}
            />
            {plan.content.length > 0 ? (
              <Card className="grid gap-2 p-4">
                <h2 className="font-semibold">Spells, features and items</h2>
                <ul className="grid" data-testid="sheet-import-content">
                  {plan.content.map((change) => (
                    <ContentRow
                      key={`${change.kind}:${change.name}`}
                      change={change}
                    />
                  ))}
                </ul>
              </Card>
            ) : null}
            <UnmappedList unmapped={plan.unmapped} />
            <div className="flex flex-wrap items-center gap-2">
              <Button
                onClick={() => void onAccept()}
                disabled={locked}
                data-testid="sheet-import-accept"
              >
                {step.kind === "applying"
                  ? `Uploading… ${step.sent}%`
                  : "Accept"}
              </Button>
              <Button
                variant="ghost"
                onClick={() => navigate(viewPath)}
                disabled={step.kind === "applying"}
                data-testid="sheet-import-decline"
              >
                Decline
              </Button>
            </div>
          </>
        ) : null}
      </Container>
    </WorldAppearance>
  );
}
