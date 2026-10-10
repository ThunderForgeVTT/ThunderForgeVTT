import { describe, expect, it } from "vitest";
import type { Attrs, OpenSpan, Telemetry } from "@thunderforge/telemetry";
import { noopTelemetry } from "@thunderforge/telemetry";
import { graphqlShape, requestTracer } from "../graphqlSpan";

describe("graphqlShape", () => {
  it("reads the operation type and the first root field", () => {
    expect(
      graphqlShape("query W($id: UUID!) { world(id: $id) { id } }"),
    ).toEqual({
      "graphql.operation.type": "query",
      "graphql.root_field": "world",
    });
    expect(graphqlShape("mutation { moveToken(input: {}) { ok } }")).toEqual({
      "graphql.operation.type": "mutation",
      "graphql.root_field": "moveToken",
    });
    expect(graphqlShape("{ me { id } }")).toEqual({
      "graphql.operation.type": "query",
      "graphql.root_field": "me",
    });
  });

  it("names the field, never the alias, and skips comments", () => {
    expect(
      graphqlShape("# a comment { x }\nquery { mine: worldAbilities { id } }"),
    ).toEqual({
      "graphql.operation.type": "query",
      "graphql.root_field": "worldAbilities",
    });
  });

  it("gives only the type for a document it cannot read", () => {
    expect(graphqlShape("not graphql")).toEqual({
      "graphql.operation.type": "query",
    });
  });
});

describe("requestTracer", () => {
  function recorder(traceparent: string | null) {
    const ended: { start: number; end: number; attrs?: Attrs }[] = [];
    let at = 100;
    const t: Telemetry = {
      ...noopTelemetry,
      begin(_name, start, attrs): OpenSpan {
        return {
          ref: { traceId: "a".repeat(32), spanId: "b".repeat(16) },
          traceparent,
          end: (end) => ended.push({ start, end, attrs }),
        };
      },
    };
    const tracer = requestTracer(t, () => (at += 5));
    return { tracer, ended };
  }

  it("opens a graphql.request span and ends it when the request does", () => {
    const tp = `00-${"a".repeat(32)}-${"b".repeat(16)}-01`;
    const { tracer, ended } = recorder(tp);
    const trace = tracer("query { world { id } }");
    expect(trace?.traceparent).toBe(tp);
    trace?.end(true);
    expect(ended).toEqual([
      {
        start: 105,
        end: 110,
        attrs: {
          "graphql.operation.type": "query",
          "graphql.root_field": "world",
        },
      },
    ]);
  });

  it("gives nothing in an unsampled session", () => {
    const { tracer } = recorder(null);
    expect(tracer("query { world { id } }")).toBeNull();
  });
});
