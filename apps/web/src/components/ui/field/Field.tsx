import type { ReactNode } from "react";
import { Label } from "@/components/ui/label";

export interface FieldProps {
  label: string;
  htmlFor: string;
  hint?: string;
  error?: string;
  accent?: string;
  children: ReactNode;
}

export function Field({
  label,
  htmlFor,
  hint,
  error,
  accent,
  children,
}: FieldProps) {
  return (
    <div className="grid gap-2">
      <Label
        htmlFor={htmlFor}
        className="flex items-center justify-between gap-3 text-xs font-semibold tracking-wider text-muted-foreground uppercase"
      >
        {label}
        {accent ? (
          <span className="text-[0.7rem] text-primary">{accent}</span>
        ) : null}
      </Label>
      {children}
      {error ? <span className="text-sm text-destructive">{error}</span> : null}
      {hint && !error ? (
        <span className="text-sm text-muted-foreground">{hint}</span>
      ) : null}
    </div>
  );
}
