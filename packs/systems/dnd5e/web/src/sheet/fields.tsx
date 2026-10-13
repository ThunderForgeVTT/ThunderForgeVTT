import { useState, type ReactNode } from "react";
import { Button } from "@thunderforge/host";
import {
  fieldClass,
  hintClass,
  sectionHeadingClass,
  textareaClass,
} from "../components/styles";

/*
 * The controls the 5e sheet's regions are built from: drafts that commit on
 * blur, Enter or Save, and labelled rows that turn to text without `canEdit`.
 */

/**
 * A checkbox that shows the click at once and settles on the server's answer.
 *
 * Its `checked` prop is the stored value. A plain controlled checkbox would
 * snap back to that value on the very next render and only move once the
 * refetch landed, which reads as a dead control. The draft flips immediately;
 * the `key` on the stored value resets the draft when the server replies, so
 * a rejected write puts the box back where it was.
 */
export function OptimisticCheckbox(props: {
  id: string;
  testId: string;
  checked: boolean;
  disabled: boolean;
  ariaLabel?: string;
  onToggle: () => Promise<void> | void;
}) {
  return <OptimisticCheckboxDraft key={String(props.checked)} {...props} />;
}

function OptimisticCheckboxDraft({
  id,
  testId,
  checked,
  disabled,
  ariaLabel,
  onToggle,
}: {
  id: string;
  testId: string;
  checked: boolean;
  disabled: boolean;
  ariaLabel?: string;
  onToggle: () => Promise<void> | void;
}) {
  const [draft, setDraft] = useState(checked);
  return (
    <input
      id={id}
      type="checkbox"
      data-testid={testId}
      className="h-6 w-6"
      checked={draft}
      disabled={disabled}
      aria-label={ariaLabel}
      onChange={() => {
        setDraft(!draft);
        void onToggle();
      }}
    />
  );
}

export function Fact({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="grid gap-0.5">
      <div className={sectionHeadingClass}>{label}</div>
      <div className="font-medium break-words">{children}</div>
    </div>
  );
}

/** A labelled row: the control when editable, the value as text otherwise. */
export function Field({
  id,
  label,
  canEdit,
  display,
  children,
}: {
  id: string;
  label: string;
  canEdit: boolean;
  display: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="grid gap-1">
      <div>
        <label
          htmlFor={canEdit ? id : undefined}
          className={sectionHeadingClass}
        >
          {label}
        </label>
      </div>
      <div className="grid">
        {canEdit ? (
          children
        ) : (
          <span className="font-medium break-words">{display}</span>
        )}
      </div>
    </div>
  );
}

/**
 * A number that commits on blur or Enter rather than on every keystroke.
 *
 * Typing "18" passes through "1"; typing a new value passes through "". A
 * write per keystroke would send both to a validator that rejects them, and
 * would race itself besides. Keyed by the stored value in the parent, so a
 * save from elsewhere replaces an untouched draft.
 */
export function NumberInput({
  id,
  testId,
  className,
  value,
  min,
  max,
  step,
  disabled,
  ariaLabel,
  onCommit,
}: {
  id: string;
  testId?: string;
  className: string;
  value: number;
  min?: number;
  max?: number;
  step?: number;
  disabled?: boolean;
  ariaLabel?: string;
  onCommit: (value: number) => Promise<void> | void;
}) {
  return (
    <NumberDraft key={value} initial={value}>
      {(draft, setDraft) => {
        const commit = () => {
          const parsed = Number(draft);
          if (draft.trim() === "" || !Number.isFinite(parsed)) {
            setDraft(String(value));
            return;
          }
          const rounded = Math.round(parsed);
          if (rounded !== value) void onCommit(rounded);
          else setDraft(String(value));
        };
        return (
          <input
            id={id}
            type="number"
            inputMode="numeric"
            className={className}
            data-testid={testId}
            value={draft}
            min={min}
            max={max}
            step={step}
            disabled={disabled}
            aria-label={ariaLabel}
            onChange={(event) => setDraft(event.target.value)}
            onBlur={commit}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                commit();
              }
            }}
          />
        );
      }}
    </NumberDraft>
  );
}

function NumberDraft({
  initial,
  children,
}: {
  initial: number;
  children: (draft: string, setDraft: (next: string) => void) => ReactNode;
}) {
  const [draft, setDraft] = useState(String(initial));
  return <>{children(draft, setDraft)}</>;
}

export function NumberField({
  id,
  label,
  canEdit,
  value,
  min,
  max,
  step,
  hint,
  onCommit,
}: {
  id: string;
  label: string;
  canEdit: boolean;
  value: number;
  min?: number;
  max?: number;
  step?: number;
  hint?: string;
  onCommit: (value: number) => Promise<void> | void;
}) {
  return (
    <div className="grid gap-1">
      <div>
        <label
          htmlFor={canEdit ? id : undefined}
          className={sectionHeadingClass}
        >
          {label}
        </label>
      </div>
      <div className="grid gap-0.5">
        {canEdit ? (
          <NumberInput
            id={id}
            testId={`${id}-input`}
            className={`${fieldClass} w-24 tabular-nums`}
            value={value}
            min={min}
            max={max}
            step={step}
            onCommit={onCommit}
          />
        ) : (
          <span className="text-lg font-semibold tabular-nums" data-testid={id}>
            {value}
          </span>
        )}
        {hint ? <span className={hintClass}>{hint}</span> : null}
      </div>
    </div>
  );
}

/** Short free text, committed on blur or Enter. */
export function TextField({
  id,
  label,
  canEdit,
  value,
  onCommit,
}: {
  id: string;
  label: string;
  canEdit: boolean;
  value: string;
  onCommit: (value: string) => Promise<void> | void;
}) {
  return (
    <div className="grid gap-1">
      <div>
        <label
          htmlFor={canEdit ? id : undefined}
          className={sectionHeadingClass}
        >
          {label}
        </label>
      </div>
      <div className="grid">
        {canEdit ? (
          <TextDraft key={value} initial={value}>
            {(draft, setDraft) => {
              const commit = () => {
                if (draft.trim() !== value) void onCommit(draft.trim());
              };
              return (
                <input
                  id={id}
                  type="text"
                  className={fieldClass}
                  data-testid={`${id}-input`}
                  value={draft}
                  onChange={(event) => setDraft(event.target.value)}
                  onBlur={commit}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault();
                      commit();
                    }
                  }}
                />
              );
            }}
          </TextDraft>
        ) : (
          <span className="font-medium break-words" data-testid={id}>
            {value || "—"}
          </span>
        )}
      </div>
    </div>
  );
}

function TextDraft({
  initial,
  children,
}: {
  initial: string;
  children: (draft: string, setDraft: (next: string) => void) => ReactNode;
}) {
  const [draft, setDraft] = useState(initial);
  return <>{children(draft, setDraft)}</>;
}

/**
 * A list of strings edited as lines. A draft and a Save rather than a write
 * per keystroke, for the same reason as the notes. Keyed by the stored list
 * in the parent.
 */
export function ListField({
  id,
  label,
  canEdit,
  items,
  rows = 3,
  isSaving,
  onSave,
}: {
  id: string;
  label: string;
  canEdit: boolean;
  items: string[];
  rows?: number;
  isSaving: boolean;
  onSave: (items: string[]) => Promise<void> | void;
}) {
  const stored = items.join("\n");
  return (
    <ListDraft key={stored} initial={stored}>
      {(draft, setDraft) => (
        <div className="grid gap-1" data-testid={id}>
          <label
            htmlFor={canEdit ? `${id}-input` : undefined}
            className={sectionHeadingClass}
          >
            {label}
          </label>
          {canEdit ? (
            <>
              <textarea
                id={`${id}-input`}
                rows={rows}
                className={textareaClass}
                data-testid={`${id}-input`}
                value={draft}
                placeholder="One per line"
                onChange={(event) => setDraft(event.target.value)}
              />
              <Button
                size="sm"
                variant="secondary"
                className="justify-self-start"
                disabled={isSaving || draft === stored}
                data-testid={`${id}-save`}
                onClick={() =>
                  void onSave(
                    draft
                      .split("\n")
                      .map((line) => line.trim())
                      .filter(Boolean),
                  )
                }
              >
                Update {label.toLowerCase()}
              </Button>
            </>
          ) : items.length === 0 ? (
            <p className={hintClass}>None.</p>
          ) : (
            <ul className="list-disc pl-5 text-sm">
              {items.map((item, index) => (
                <li key={`${item}-${index}`}>{item}</li>
              ))}
            </ul>
          )}
        </div>
      )}
    </ListDraft>
  );
}

function ListDraft({
  initial,
  children,
}: {
  initial: string;
  children: (draft: string, setDraft: (next: string) => void) => ReactNode;
}) {
  const [draft, setDraft] = useState(initial);
  return <>{children(draft, setDraft)}</>;
}
