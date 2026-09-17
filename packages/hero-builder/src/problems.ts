/**
 * A problem with a spec, split into the field it names and what is wrong.
 *
 * `validateHero` writes each problem as "field: message"; a host wants the
 * two apart so it can put the message beside the field.
 */
export interface HeroProblem {
  /** The spec field, or "" for a problem with the whole spec. */
  field: string;
  message: string;
}

export function toProblems(lines: readonly string[]): HeroProblem[] {
  return lines.map((line) => {
    const colon = line.indexOf(": ");
    return colon < 0
      ? { field: "", message: line }
      : { field: line.slice(0, colon), message: line.slice(colon + 2) };
  });
}
