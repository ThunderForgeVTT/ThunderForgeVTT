import { postGraphQL } from "@/api/graphqlClient";
import type {
  AdminSettingsData,
  AdminStats,
  AdminWelcomeSummary,
  AuthSecuritySettings,
  OAuthProviderConfig,
  StorageConnectionReport,
  SystemManifest,
  UpdateOAuthProviderInput,
} from "@/types/admin";

/**
 * Every field of a provider the UI reads, in one place.
 *
 * Three queries select it — the admin panel's bundle, the update mutation's
 * response, and the setup wizard's own smaller query — and a provider the
 * wizard could not see because one copy of this list lagged behind is a bug
 * with no symptom but a missing field.
 */
const OAUTH_PROVIDER_FIELDS = `
  id
  providerKey
  displayName
  authorizationUrl
  tokenUrl
  userinfoUrl
  issuerUrl
  requiresIssuerUrl
  scopes
  oauthClientId
  configured
  enabled
  hasClientSecret
  updatedAt
  configSource
`;

type AdminWelcomeSummaryQuery = {
  adminWelcomeSummary: AdminWelcomeSummary;
};

type AdminSettingsQuery = {
  adminStats: AdminStats;
  systemManifest: SystemManifest;
  oauthProviders: OAuthProviderConfig[];
  authSecuritySettings: AuthSecuritySettings;
  twoFactorCoverage: AdminSettingsData["twoFactorCoverage"];
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
          source
          fixedBy
        }
      }
      oauthProviders { ${OAUTH_PROVIDER_FIELDS} }
      authSecuritySettings {
        twoFactorRequiredForAllUsers
        updatedAt
      }
      twoFactorCoverage {
        enrolled
        notEnrolled
        requiredNotEnrolled
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
    twoFactorCoverage: data.twoFactorCoverage,
    adminBootstrapSettings: data.adminBootstrapSettings,
  }));
}

/**
 * The sign-in providers alone.
 *
 * The setup wizard needs these and nothing else in `adminSettings` — disk
 * usage and the system manifest on a wizard step are both a waste and a
 * surface for an unrelated resolver to fail the step with.
 */
export function getOAuthProviders(): Promise<OAuthProviderConfig[]> {
  return postGraphQL<{ oauthProviders: OAuthProviderConfig[] }>(
    `
      query SetupOAuthProviders {
        oauthProviders { ${OAUTH_PROVIDER_FIELDS} }
      }
    `,
  ).then((data) => data.oauthProviders);
}

/**
 * Ask the server whether the storage answers actually reach an object store.
 *
 * Creates the bucket if it is missing, which on a fresh instance is the
 * expected state rather than a failure.
 */
export function testStorageConnection(): Promise<StorageConnectionReport> {
  return postGraphQL<{ testStorageConnection: StorageConnectionReport }>(`
    mutation TestStorageConnection {
      testStorageConnection {
        reachable
        endpoint
        bucket
        detail
      }
    }
  `).then((data) => data.testStorageConnection);
}

export function updateOAuthProvider(
  providerId: string,
  config: UpdateOAuthProviderInput,
): Promise<OAuthProviderConfig> {
  return postGraphQL<UpdateOAuthProviderMutation>(
    `
      mutation UpdateOAuthProvider($providerId: UUID!, $config: GraphQLOAuthProviderConfigInput!) {
        updateOauthProvider(providerId: $providerId, config: $config) { ${OAUTH_PROVIDER_FIELDS} }
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
            source
            fixedBy
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
