import { useState } from "react";
import {
  PERSONA_LONG,
  PERSONA_LONG_MAX,
  PERSONA_SHORT,
  PERSONA_SHORT_MAX,
} from "./data";
import {
  fieldClass,
  hintClass,
  sectionHeadingClass,
  textareaClass,
} from "../components/styles";

/**
 * Spec 048: who the character is, away from the dice. A few short answers
 * about how they look and what they believe, then the paragraphs a player
 * writes about them. Each one saves when it loses focus.
 */
export default function PersonaSection({
  persona,
  canEdit,
  disabled,
  onSave,
}: {
  persona: Record<string, string>;
  canEdit: boolean;
  disabled: boolean;
  onSave: (key: string, value: string) => void;
}) {
  const shown = (key: string) => canEdit || persona[key] !== "";
  const short = PERSONA_SHORT.filter(({ key }) => shown(key));
  const long = PERSONA_LONG.filter(({ key }) => shown(key));
  if (short.length === 0 && long.length === 0) {
    return (
      <p className={hintClass} data-testid="dnd5e-persona">
        Nothing written yet.
      </p>
    );
  }
  return (
    <div className="grid gap-3" data-testid="dnd5e-persona">
      {short.length > 0 ? (
        <dl className="grid grid-cols-2 gap-2 @md:grid-cols-4">
          {short.map(({ key, label }) => (
            <div key={key} className="grid gap-1">
              <dt>
                <label
                  htmlFor={canEdit ? `dnd5e-${key}` : undefined}
                  className={sectionHeadingClass}
                >
                  {label}
                </label>
              </dt>
              <dd className="grid">
                {canEdit ? (
                  <Draft
                    key={persona[key]}
                    id={`dnd5e-${key}`}
                    value={persona[key]}
                    max={PERSONA_SHORT_MAX}
                    disabled={disabled}
                    onCommit={(value) => onSave(key, value)}
                  />
                ) : (
                  <span className="text-sm" data-testid={`dnd5e-${key}`}>
                    {persona[key]}
                  </span>
                )}
              </dd>
            </div>
          ))}
        </dl>
      ) : null}
      {long.map(({ key, label }) => (
        <div key={key} className="grid gap-1">
          <label
            htmlFor={canEdit ? `dnd5e-${key}` : undefined}
            className={sectionHeadingClass}
          >
            {label}
          </label>
          {canEdit ? (
            <Draft
              key={persona[key]}
              id={`dnd5e-${key}`}
              value={persona[key]}
              max={PERSONA_LONG_MAX}
              multiline
              disabled={disabled}
              onCommit={(value) => onSave(key, value)}
            />
          ) : (
            <p
              className="text-sm whitespace-pre-wrap break-words"
              data-testid={`dnd5e-${key}`}
            >
              {persona[key]}
            </p>
          )}
        </div>
      ))}
    </div>
  );
}

function Draft({
  id,
  value,
  max,
  multiline = false,
  disabled,
  onCommit,
}: {
  id: string;
  value: string;
  max: number;
  multiline?: boolean;
  disabled: boolean;
  onCommit: (value: string) => void;
}) {
  const [draft, setDraft] = useState(value);
  const commit = () => {
    const next = multiline ? draft.trimEnd() : draft.trim();
    if (next !== value) onCommit(next);
  };
  return multiline ? (
    <textarea
      id={id}
      className={textareaClass}
      rows={4}
      maxLength={max}
      value={draft}
      disabled={disabled}
      data-testid={`${id}-input`}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
    />
  ) : (
    <input
      id={id}
      type="text"
      className={fieldClass}
      maxLength={max}
      value={draft}
      disabled={disabled}
      data-testid={`${id}-input`}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
    />
  );
}
