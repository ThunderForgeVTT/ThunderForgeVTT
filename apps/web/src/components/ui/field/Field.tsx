import * as Label from "@radix-ui/react-label";
import type { ReactNode } from "react";
import styles from "./Field.module.scss";

export interface FieldProps {
  label: string;
  htmlFor: string;
  hint?: string;
  error?: string;
  accent?: string;
  children: ReactNode;
}

export function Field({ label, htmlFor, hint, error, accent, children }: FieldProps) {
  return (
    <div className={styles.field}>
      <Label.Root className={styles.label} htmlFor={htmlFor}>
        {label}
        {accent ? <span className={styles.accent}>{accent}</span> : null}
      </Label.Root>
      {children}
      {error ? <span className={styles.error}>{error}</span> : null}
      {hint && !error ? <span className={styles.hint}>{hint}</span> : null}
    </div>
  );
}
