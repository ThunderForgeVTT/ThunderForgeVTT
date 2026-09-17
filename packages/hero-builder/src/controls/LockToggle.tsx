export interface LockToggleProps {
  field: string;
  label: string;
  locked: boolean;
  onToggle(): void;
}

/** Keeps a field out of the dice (FR-007). */
export function LockToggle({
  field,
  label,
  locked,
  onToggle,
}: LockToggleProps) {
  return (
    <button
      type="button"
      className="tfhb-lock"
      data-testid={`hero-lock-${field}`}
      aria-pressed={locked}
      aria-label={`Keep ${label} when rolling`}
      title={locked ? "Kept when rolling" : "Rolled with the dice"}
      onClick={onToggle}
    >
      <span aria-hidden="true">{locked ? "🔒" : "🔓"}</span>
    </button>
  );
}
