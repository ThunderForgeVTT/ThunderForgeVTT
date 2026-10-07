/**
 * The server's schema, run in the page.
 *
 * `apps/thunderforge/schema.graphql` is the SDL the server itself prints
 * (`thunderforge-schema`) and the web app's operations are already checked
 * against (`scripts/check-graphql-contract.mjs`). Building the demo's schema
 * from that same file is what makes FR-010 true without a test having to
 * remember it: a query for a field the server does not have fails validation
 * here exactly as it would there, and a handler that returns a row missing a
 * field the schema calls non-null is an error in the answer, not a blank on
 * the page.
 *
 * Only the top of each operation is ours. A root field is looked up in the
 * declared handlers (FR-009); everything beneath it is read off the row the
 * handler returned.
 */
import {
  buildSchema,
  defaultFieldResolver,
  execute,
  graphql,
  GraphQLError,
  parse,
  validate,
  type ExecutionResult,
  type GraphQLFieldResolver,
} from "graphql";
import sdl from "../../../server/schema.graphql?raw";
import { mutations, queries } from "./handlers";
import {
  NOT_IN_DEMO_CODE,
  notInDemoMessage,
  reportNotInDemo,
} from "./notInDemo";

const schema = buildSchema(sdl);

const fieldResolver: GraphQLFieldResolver<unknown, unknown> = (
  source,
  args,
  context,
  info,
) => {
  if (info.path.prev !== undefined) {
    return defaultFieldResolver(source, args, context, info);
  }
  const kind = info.operation.operation;
  if (kind === "subscription") {
    // One event, already chosen by whoever is delivering it.
    return (source as Record<string, unknown>)[info.fieldName];
  }
  const handler = (kind === "mutation" ? mutations : queries)[info.fieldName];
  if (!handler) {
    reportNotInDemo(info.fieldName);
    throw new GraphQLError(notInDemoMessage(info.fieldName), {
      extensions: { code: NOT_IN_DEMO_CODE },
    });
  }
  return handler(args);
};

export interface OperationRequest {
  query: string;
  variables?: Record<string, unknown> | null;
  operationName?: string | null;
}

export function runOperation(
  request: OperationRequest,
): Promise<ExecutionResult> {
  return graphql({
    schema,
    source: request.query,
    variableValues: request.variables ?? undefined,
    operationName: request.operationName ?? undefined,
    fieldResolver,
  });
}

/**
 * A subscription the page opened. `subscriptionField` names what it asked
 * for; `shape` turns one delivered value into the answer the operation's own
 * selection describes.
 */
export function openSubscription(request: OperationRequest):
  | { errors: readonly GraphQLError[] }
  | {
      field: string;
      args: Record<string, unknown>;
      shape: (value: unknown) => Promise<ExecutionResult>;
    } {
  let document;
  try {
    document = parse(request.query);
  } catch (error) {
    return { errors: [error as GraphQLError] };
  }
  const errors = validate(schema, document);
  if (errors.length > 0) return { errors };

  const operation = document.definitions.find(
    (definition) => definition.kind === "OperationDefinition",
  );
  const selection = operation?.selectionSet.selections[0];
  if (!selection || selection.kind !== "Field") {
    return {
      errors: [new GraphQLError("The demo could not read that subscription.")],
    };
  }
  const field = selection.name.value;
  return {
    field,
    args: request.variables ?? {},
    shape: async (value) =>
      execute({
        schema,
        document,
        rootValue: { [selection.alias?.value ?? field]: value, [field]: value },
        variableValues: request.variables ?? undefined,
        fieldResolver,
      }),
  };
}
