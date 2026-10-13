import { Link } from "react-router-dom";
import type { PauseTrigger } from "@/api/playPause";
import {
  entityLabel,
  formatMoment,
  TRIGGER_KIND_LABEL,
} from "./playPauseFormat";

/**
 * What prompted a pause or a request: kind, what it landed on, when, and the
 * moderation case a takedown belongs to. Shared by *Requests* and *Record*;
 * `testId` keeps each section's hooks its own.
 */
export function TriggerList({
  triggers,
  label,
  testId,
}: {
  triggers: PauseTrigger[];
  label: string;
  testId: string;
}) {
  if (triggers.length === 0) return null;
  return (
    <ul className="grid gap-1 text-sm" aria-label={label}>
      {triggers.map((trigger, index) => (
        <li
          key={`${trigger.moderationActionId ?? "trigger"}-${index}`}
          className="flex flex-wrap items-center gap-x-2 gap-y-0.5"
          data-testid={`${testId}-trigger`}
          data-kind={trigger.kind}
          data-entity-id={trigger.entityId ?? undefined}
        >
          {/* Spaces written out: flex gaps separate the parts on screen, not
              in the text a screen reader or a copy reads. */}
          <span className="font-medium">
            {TRIGGER_KIND_LABEL[trigger.kind]}
          </span>{" "}
          {entityLabel(trigger.entityType) ? (
            <>
              <span className="text-muted-foreground">
                on {entityLabel(trigger.entityType)}
              </span>{" "}
            </>
          ) : null}
          <span className="text-xs text-muted-foreground">
            recorded{" "}
            <time dateTime={trigger.recordedAt}>
              {formatMoment(trigger.recordedAt)}
            </time>
          </span>{" "}
          {trigger.caseId ? (
            <>
              <Link
                to={`/admin/moderation?case=${trigger.caseId}`}
                className="text-xs underline underline-offset-2"
                data-testid={`${testId}-case-link`}
              >
                Moderation case <code>{trigger.caseId.slice(0, 8)}</code>
              </Link>{" "}
            </>
          ) : null}
          {trigger.note ? (
            <span className="basis-full whitespace-pre-line text-muted-foreground">
              {trigger.note}
            </span>
          ) : null}
        </li>
      ))}
    </ul>
  );
}

/** A world's name as the record holds it, marked when the world is gone. */
export function WorldName({ name, exists }: { name: string; exists: boolean }) {
  return (
    <span className="font-medium" data-world-exists={exists}>
      {name}
      {exists ? "" : " (deleted)"}
    </span>
  );
}
