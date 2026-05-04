export interface DiskUsageBreakdown {
  totalBytes: number;
  worldsBytes: number;
  assetsBytes: number;
  clientBytes: number;
  databasesBytes: number;
  modulesBytes: number;
}

export interface AdminStats {
  totalUsers: number;
  totalWorlds: number;
  totalWorldTokens: number;
  totalWorldEvents: number;
  totalPolicies: number;
  diskUsageBytes: number;
  diskUsage: DiskUsageBreakdown;
}

export interface AdminWelcomeSummary {
  totalUsers: number;
  totalWorlds: number;
  totalTokens: number;
  totalEvents: number;
  diskUsage: number;
}

export interface OAuthProviderConfig {
  id: string;
  providerKey: string;
  displayName: string;
  authorizationUrl: string;
  tokenUrl: string;
  userinfoUrl: string | null;
  scopes: string[];
  oauthClientId: string | null;
  configured: boolean;
  enabled: boolean;
  hasClientSecret: boolean;
  updatedAt: string;
}

export interface ManifestEntry {
  key: string;
  value: string;
  editable: boolean;
}

export interface SystemManifest {
  path: string;
  schemaVersion: string;
  updatedAt: string;
  entries: ManifestEntry[];
}

export interface AuthSecuritySettings {
  twoFactorRequiredForAllUsers: boolean;
  updatedAt: string;
}

export interface AdminBootstrapSettings {
  setupCompleted: boolean;
  adminCodeGeneratedAt: string | null;
  setupCompletedAt: string | null;
  updatedAt: string;
}

export interface AdminSettingsData {
  adminStats: AdminStats;
  systemManifest: SystemManifest;
  oauthProviders: OAuthProviderConfig[];
  authSecuritySettings: AuthSecuritySettings;
  adminBootstrapSettings: AdminBootstrapSettings | null;
}

export interface UpdateOAuthProviderInput {
  displayName?: string;
  oauthClientId?: string;
  oauthClientSecret?: string;
  enabled?: boolean;
  userinfoUrl?: string;
  scopes?: string[];
}
