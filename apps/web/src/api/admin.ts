import { withCsrf } from "@/api/auth";
import type {
  AdminSettingsData,
  AdminStats,
  AdminWelcomeSummary,
  AuthSecuritySettings,
  OAuthProviderConfig,
  SystemManifest,
  UpdateOAuthProviderInput,
} from "@/types/admin";

type GraphQLError = {
  message?: string;
};

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: GraphQLError[];
};

const GRAPHQL_ENDPOINT = "/api/graphql";

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

type AdminWelcomeSummaryQuery = {
  adminWelcomeSummary: AdminWelcomeSummary;
};

type AdminSettingsQuery = {
  adminStats: AdminStats;
  systemManifest: SystemManifest;
  oauthProviders: OAuthProviderConfig[];
  authSecuritySettings: AuthSecuritySettings;
  adminBootstrapSettings: AdminSettingsData["adminBootstrapSettings"];
};

type UpdateOAuthProviderMutation = {
  updateOauthProvider: OAuthProviderConfig;
};

type UpdateManifestKeyMutation = {
  updateManifestKey: SystemManifest;
};

type RecalculateDiskUsageMutation = {
  recalculateDiskUsage: AdminStats;
};

type UpdateTwoFactorPolicyMutation = {
  updateTwoFactorPolicy: AuthSecuritySettings;
};

export function getAdminWelcomeSummary(): Promise<AdminWelcomeSummary> {
  return postGraphQL<AdminWelcomeSummaryQuery>(`
    query AdminWelcomeSummary {
      adminWelcomeSummary {
        totalUsers
        totalWorlds
        totalTokens
        totalEvents
        diskUsage
      }
    }
  `).then((data) => data.adminWelcomeSummary);
}

export function getAdminSettingsData(): Promise<AdminSettingsData> {
  return postGraphQL<AdminSettingsQuery>(`
    query AdminSettingsData {
      adminStats {
        totalUsers
        totalWorlds
        totalWorldTokens
        totalWorldEvents
        totalPolicies
        diskUsageBytes
        diskUsage {
          totalBytes
          worldsBytes
          assetsBytes
          clientBytes
          databasesBytes
          modulesBytes
        }
      }
      systemManifest {
        path
        schemaVersion
        updatedAt
        entries {
          key
          value
          editable
        }
      }
      oauthProviders {
        id
        providerKey
        displayName
        authorizationUrl
        tokenUrl
        userinfoUrl
        scopes
        oauthClientId
        configured
        enabled
        hasClientSecret
        updatedAt
        configSource
      }
      authSecuritySettings {
        twoFactorRequiredForAllUsers
        updatedAt
      }
      adminBootstrapSettings {
        setupCompleted
        adminCodeGeneratedAt
        setupCompletedAt
        updatedAt
      }
    }
  `).then((data) => ({
    adminStats: data.adminStats,
    systemManifest: data.systemManifest,
    oauthProviders: data.oauthProviders,
    authSecuritySettings: data.authSecuritySettings,
    adminBootstrapSettings: data.adminBootstrapSettings,
  }));
}

export function updateOAuthProvider(
  providerId: string,
  config: UpdateOAuthProviderInput,
): Promise<OAuthProviderConfig> {
  return postGraphQL<UpdateOAuthProviderMutation>(
    `
      mutation UpdateOAuthProvider($providerId: UUID!, $config: GraphQLOAuthProviderConfigInput!) {
        updateOauthProvider(providerId: $providerId, config: $config) {
          id
          providerKey
          displayName
          authorizationUrl
          tokenUrl
          userinfoUrl
          scopes
          oauthClientId
          configured
          enabled
          hasClientSecret
          updatedAt
          configSource
        }
      }
    `,
    {
      providerId,
      config,
    },
  ).then((data) => data.updateOauthProvider);
}

export function updateManifestKey(
  key: string,
  value: string,
): Promise<SystemManifest> {
  return postGraphQL<UpdateManifestKeyMutation>(
    `
      mutation UpdateManifestKey($key: String!, $value: String!) {
        updateManifestKey(key: $key, value: $value) {
          path
          schemaVersion
          updatedAt
          entries {
            key
            value
            editable
          }
        }
      }
    `,
    {
      key,
      value,
    },
  ).then((data) => data.updateManifestKey);
}

export function recalculateDiskUsage(): Promise<AdminStats> {
  return postGraphQL<RecalculateDiskUsageMutation>(`
    mutation RecalculateDiskUsage {
      recalculateDiskUsage {
        totalUsers
        totalWorlds
        totalWorldTokens
        totalWorldEvents
        totalPolicies
        diskUsageBytes
        diskUsage {
          totalBytes
          worldsBytes
          assetsBytes
          clientBytes
          databasesBytes
          modulesBytes
        }
      }
    }
  `).then((data) => data.recalculateDiskUsage);
}

export function updateTwoFactorPolicy(
  requiredForAllUsers: boolean,
): Promise<AuthSecuritySettings> {
  return postGraphQL<UpdateTwoFactorPolicyMutation>(
    `
      mutation UpdateTwoFactorPolicy($requiredForAllUsers: Boolean!) {
        updateTwoFactorPolicy(requiredForAllUsers: $requiredForAllUsers) {
          twoFactorRequiredForAllUsers
          updatedAt
        }
      }
    `,
    {
      requiredForAllUsers,
    },
  ).then((data) => data.updateTwoFactorPolicy);
}
