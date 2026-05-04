import { withCsrf } from "@/api/auth";
import type {
  CreateWorldInput,
  DeleteWorldResult,
  WorldRecord,
} from "@/types/world";

type GraphQLError = {
  message?: string;
};

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: GraphQLError[];
};

const GRAPHQL_ENDPOINT = "/graphql";

const WORLD_FIELDS = `
  id
  name
  description
  gameSystemId
  interfacePackId
  scenes
  actors
  tokens
  events
  gameSystem
  interfacePack
  createdBy
  updatedBy
  createdAt
  updatedAt
`;

async function postGraphQL<TData>(
  query: string,
  variables?: Record<string, unknown>,
): Promise<TData> {
  const response = await fetch(GRAPHQL_ENDPOINT, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({
      "Content-Type": "application/json",
    }),
    body: JSON.stringify({
      query,
      variables,
    }),
  });

  const payload = (await response.json()) as GraphQLResponse<TData>;
  if (!response.ok) {
    throw new Error(payload.errors?.[0]?.message || "GraphQL request failed");
  }

  if (payload.errors?.length) {
    throw new Error(payload.errors[0]?.message || "GraphQL request failed");
  }

  if (!payload.data) {
    throw new Error("GraphQL response did not include data");
  }

  return payload.data;
}

type MyWorldsQuery = {
  myWorlds: WorldRecord[];
};

type AllWorldsQuery = {
  allWorlds: WorldRecord[];
};

type WorldQuery = {
  world: WorldRecord | null;
};

type CreateWorldMutation = {
  createWorld: WorldRecord;
};

type DeleteWorldMutation = {
  deleteWorld: DeleteWorldResult;
};

export function getMyWorlds(): Promise<WorldRecord[]> {
  return postGraphQL<MyWorldsQuery>(`
    query MyWorlds {
      myWorlds {
        ${WORLD_FIELDS}
      }
    }
  `).then((data) => data.myWorlds);
}

export function getAllWorlds(): Promise<WorldRecord[]> {
  return postGraphQL<AllWorldsQuery>(`
    query AllWorlds {
      allWorlds {
        ${WORLD_FIELDS}
      }
    }
  `).then((data) => data.allWorlds);
}

export function getWorld(id: string): Promise<WorldRecord | null> {
  return postGraphQL<WorldQuery>(
    `
      query WorldDashboard($id: UUID!) {
        world(id: $id) {
          ${WORLD_FIELDS}
        }
      }
    `,
    { id },
  ).then((data) => data.world);
}

export function createWorld(input: CreateWorldInput): Promise<WorldRecord> {
  return postGraphQL<CreateWorldMutation>(
    `
      mutation CreateWorld($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) {
          ${WORLD_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.createWorld);
}

export function deleteWorld(id: string): Promise<DeleteWorldResult> {
  return postGraphQL<DeleteWorldMutation>(
    `
      mutation DeleteWorld($id: UUID!) {
        deleteWorld(id: $id) {
          id
          status
          message
        }
      }
    `,
    { id },
  ).then((data) => data.deleteWorld);
}
