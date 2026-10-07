/**
 * The GraphQL multipart request (the form `postGraphQLMultipart` sends and
 * async-graphql reads): `operations` is the operation with each file `null`,
 * `map` says which form field fills which variable, and each field is a file.
 * The demo puts the file where the map says, and the `Upload` scalar hands it
 * to the handler as it is.
 */
import type { OperationRequest } from "../backend/execute";

function setPath(
  target: Record<string, unknown>,
  path: string,
  value: unknown,
) {
  const keys = path.split(".");
  let at = target as Record<string, unknown>;
  for (const key of keys.slice(0, -1)) {
    at = at[key] as Record<string, unknown>;
    if (!at || typeof at !== "object") return;
  }
  at[keys[keys.length - 1]] = value;
}

export async function multipartOperation(
  form: FormData,
): Promise<OperationRequest> {
  const operation = JSON.parse(
    String(form.get("operations") ?? "{}"),
  ) as OperationRequest;
  const map = JSON.parse(String(form.get("map") ?? "{}")) as Record<
    string,
    string[]
  >;
  const holder = operation as unknown as Record<string, unknown>;
  for (const [field, paths] of Object.entries(map)) {
    const file = form.get(field);
    for (const path of paths) setPath(holder, path, file);
  }
  return operation;
}
