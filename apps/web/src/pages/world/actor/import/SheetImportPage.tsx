/**
 * Spec 048: bringing a character in from a sheet. The browser reads the
 * file with the system's own reader and asks the server what it would
 * change; nothing is uploaded until the player accepts the review.
 */
import { useEffect, useState, type ChangeEvent } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { getActor } from "@/api/actors";
import { GraphQLRequestError } from "@/api/graphqlClient";
import type { SheetImportPlan } from "@/api/sheetImport";
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

type Step =
  | { kind: "pick" }
  | { kind: "reading" }
  | { kind: "review"; file: File; plan: SheetImportPlan }
  | { kind: "applying"; file: File; plan: SheetImportPlan; sent: number };

/** What a refusal says to the person, from its code where there is one. */
export function refusalText(err: unknown): string {
  if (err instanceof GraphQLRequestError) {
    if (err.codes.includes("PLAN_CHANGED")) {
      return "The character changed while you were reviewing. Read the sheet again.";
    }
    if (err.codes.includes("FORBIDDEN")) {
      return "You may not change this character.";
    }
    if (err.codes.includes("FEATURE_DISABLED")) {
      return "Bringing in sheets is switched off on this server.";
    }
    return err.errors[0] ?? err.message;
  }
  return err instanceof Error ? err.message : String(err);
}

export default function SheetImportPage() {
  const { id: worldId = "", actorId = "" } = useParams();
  const navigate = useNavigate();
  const [actor, setActor] = useState<WorldActorRecord | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [step, setStep] = useState<Step>({ kind: "pick" });
  const [problem, setProblem] = useState<string | null>(null);
  const { preview, apply } = useSheetImport(actorId);
  const viewPath = `/world/${worldId}/actor/${actorId}/view`;

  useResetOnChange(`${worldId}|${actorId}`, () => {
    setIsLoading(true);
    setStep({ kind: "pick" });
    setProblem(null);
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
        throw new Error(answer.error ?? "This is not a sheet we can read.");
      }
      const plan = await preview(answer.reading);
      setStep({ kind: "review", file, plan });
    } catch (err) {
      setProblem(refusalText(err));
      setStep({ kind: "pick" });
    }
  };

  const onAccept = async () => {
    if (step.kind !== "review") return;
    const { file, plan } = step;
    setProblem(null);
    setStep({ kind: "applying", file, plan, sent: 0 });
    try {
      await apply({
        file,
        corrections: {},
        overwritePlayState: [],
        planHash: plan.planHash,
        onProgress: (sent, total) =>
          setStep({
            kind: "applying",
            file,
            plan,
            sent: total > 0 ? Math.round((sent / total) * 100) : 0,
          }),
      });
      navigate(viewPath);
    } catch (err) {
      setProblem(refusalText(err));
      setStep({ kind: "review", file, plan });
    }
  };

  const plan =
    step.kind === "review" || step.kind === "applying" ? step.plan : null;

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
          >
            {problem}
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
              <ul className="grid" data-testid="sheet-import-fields">
                {plan.fields.map((field) => (
                  <FieldRow key={field.path} field={field} />
                ))}
              </ul>
            </Card>
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
            {plan.unmapped.length > 0 ? (
              <p
                className="text-sm text-muted-foreground"
                data-testid="sheet-import-unmapped"
              >
                {plan.unmapped.length} value
                {plan.unmapped.length === 1 ? "" : "s"} the system does not
                track will go into the notes.
              </p>
            ) : null}
            <div className="flex flex-wrap items-center gap-2">
              <Button
                onClick={() => void onAccept()}
                disabled={step.kind === "applying"}
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
