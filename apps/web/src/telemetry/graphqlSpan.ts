/**
 * The `graphql.request` span (spec 086 US7, contracts/browser-events.md).
 *
 * Only the operation's type and its first root field are recorded, parsed
 * from the document: never the operation's name, its variables or its
 * answer. The server's span joins this one through `traceparent`.
 */
import type { Attrs, Telemetry } from "@thunderforge/telemetry";
import type { RequestTracer } from "../api/requestTracing";

const NAME = /[_A-Za-z][_0-9A-Za-z]*/y;

export function graphqlShape(query: string): Attrs {
  const text = query.replace(/#[^\n]*/g, "");
  const open = text.indexOf("{");
  const head = open < 0 ? "" : text.slice(0, open).trim();
  const type = /^(mutation|subscription)\b/.exec(head)?.[1] ?? "query";
  const attrs: Attrs = { "graphql.operation.type": type };
  if (open < 0) return attrs;

  let at = open + 1;
  const word = (): string | null => {
    while (/\s|,/.test(text[at] ?? "")) at += 1;
    NAME.lastIndex = at;
    const match = NAME.exec(text);
    if (!match) return null;
    at = NAME.lastIndex;
    return match[0];
  };
  const first = word();
  if (!first) return attrs;
  while (/\s/.test(text[at] ?? "")) at += 1;
  const field = text[at] === ":" ? (at++, word()) : first;
  if (field) attrs["graphql.root_field"] = field;
  return attrs;
}

/** The tracer the telemetry chunk gives `graphqlClient`. */
export function requestTracer(
  t: Telemetry,
  now: () => number = () => Date.now(),
): RequestTracer {
  return (query) => {
    const open = t.begin("graphql.request", now(), graphqlShape(query));
    if (!open.traceparent) return null;
    return {
      traceparent: open.traceparent,
      end: () => open.end(now()),
    };
  };
}
