import type { DurableMutation, TokenDelta, WorldSyncTransport } from "./types";

type CreateGraphQLWorldSyncTransportOptions = {
  graphqlEndpoint?: string;
  worldEventPathTemplate?: string;
  durableMutationName?: string;
  tokenDeltaMutationName?: string;
};

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: Array<{ message?: string }>;
};

type GenericMutationResponse = Record<string, unknown>;

const DEFAULT_GRAPHQL_ENDPOINT = "/graphql";
const DEFAULT_WORLD_EVENT_PATH_TEMPLATE = "/world/{worldId}/event";

function resolveTemplate(template: string, worldId: string): string {
  return template.replace("{worldId}", encodeURIComponent(worldId));
}

async function postJson(path: string, body: unknown): Promise<Response> {
  return fetch(path, {
    method: "POST",
    credentials: "same-origin",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  });
}

async function postGraphQL<TData>(
  endpoint: string,
  query: string,
  variables: Record<string, unknown>
): Promise<GraphQLResponse<TData>> {
  const response = await postJson(endpoint, { query, variables });
  if (!response.ok) {
    throw new Error(`GraphQL request failed with status ${response.status}`);
  }

  return (await response.json()) as GraphQLResponse<TData>;
}

function worldEventPayload(kind: string, payload: unknown) {
  return {
    event_type: kind,
    payload,
    emitted_at: new Date().toISOString(),
  };
}

export function createGraphQLWorldSyncTransport(
  options: CreateGraphQLWorldSyncTransportOptions = {}
): WorldSyncTransport {
  const graphqlEndpoint = options.graphqlEndpoint ?? import.meta.env.VITE_GRAPHQL_ENDPOINT ?? DEFAULT_GRAPHQL_ENDPOINT;
  const worldEventPathTemplate =
    options.worldEventPathTemplate ??
    import.meta.env.VITE_WORLD_EVENT_PATH_TEMPLATE ??
    DEFAULT_WORLD_EVENT_PATH_TEMPLATE;
  const durableMutationName =
    options.durableMutationName ?? import.meta.env.VITE_WORLD_SYNC_DURABLE_MUTATION ?? "syncWorldMutations";
  const tokenDeltaMutationName =
    options.tokenDeltaMutationName ?? import.meta.env.VITE_WORLD_SYNC_DELTA_MUTATION ?? "publishTokenDeltas";

  let activeWorldId: string | null = null;

  async function fallbackDurable(worldId: string, mutations: DurableMutation[]) {
    const path = resolveTemplate(worldEventPathTemplate, worldId);
    await postJson(path, worldEventPayload("durable_world_mutations", { worldId, mutations }));
  }

  async function fallbackDeltas(worldId: string, deltas: TokenDelta[]) {
    const path = resolveTemplate(worldEventPathTemplate, worldId);
    await postJson(path, worldEventPayload("token_deltas", { worldId, deltas }));
  }

  return {
    async start(worldId) {
      activeWorldId = worldId;
    },

    async stop(worldId) {
      if (activeWorldId === worldId) {
        activeWorldId = null;
      }
    },

    async pushDurableMutations(mutations) {
      if (!activeWorldId || mutations.length === 0) {
        return;
      }

      const query = `mutation ${durableMutationName}($worldId: String!, $mutations: JSON!) {\n  ${durableMutationName}(worldId: $worldId, mutations: $mutations)\n}`;

      try {
        const response = await postGraphQL<GenericMutationResponse>(graphqlEndpoint, query, {
          worldId: activeWorldId,
          mutations,
        });

        if (response.errors && response.errors.length > 0) {
          await fallbackDurable(activeWorldId, mutations);
        }
      } catch {
        await fallbackDurable(activeWorldId, mutations);
      }
    },

    async sendTokenDeltas(deltas) {
      if (!activeWorldId || deltas.length === 0) {
        return;
      }

      const query = `mutation ${tokenDeltaMutationName}($worldId: String!, $deltas: JSON!) {\n  ${tokenDeltaMutationName}(worldId: $worldId, deltas: $deltas)\n}`;

      try {
        const response = await postGraphQL<GenericMutationResponse>(graphqlEndpoint, query, {
          worldId: activeWorldId,
          deltas,
        });

        if (response.errors && response.errors.length > 0) {
          await fallbackDeltas(activeWorldId, deltas);
        }
      } catch {
        await fallbackDeltas(activeWorldId, deltas);
      }
    },
  };
}
