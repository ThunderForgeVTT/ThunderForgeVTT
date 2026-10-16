import { retryImport } from "@/components/ui/lazy-boundary/retryImport";

const loaders = {
  login: () => import("@/pages/auth/LoginPage"),
  signup: () => import("@/pages/auth/RegisterPage"),
  oauthCallback: () => import("@/pages/auth/OAuthCallbackPage"),
  dmcaCompliance: () => import("@/pages/legal/DmcaCompliancePage"),
  operatorResponsibilities: () =>
    import("@/pages/legal/OperatorResponsibilitiesPage"),
  termsOfService: () => import("@/pages/legal/TermsOfServicePage"),
  privacyPolicy: () => import("@/pages/legal/PrivacyPolicyPage"),
  adminWelcome: () => import("@/pages/admin/AdminWelcomePage"),
  adminSettings: () => import("@/pages/admin/SettingsPage"),
  adminModerationReview: () => import("@/pages/admin/ModerationReviewPage"),
  setup: () => import("@/pages/setup/SetupPage"),
  setupCallback: () => import("@/pages/setup/SetupCallbackPage"),
  welcome: () => import("@/pages/user/WelcomePage"),
  storageSettings: () => import("@/pages/user/StorageSettingsPage"),
  securitySettings: () => import("@/pages/user/SecuritySettingsPage"),
  standing: () => import("@/pages/user/StandingPage"),
  accountSettings: () => import("@/pages/user/AccountSettingsPage"),
  worldList: () => import("@/pages/world/WorldListPage"),
  createWorld: () => import("@/pages/world/CreateWorldPage"),
  worldOverview: () => import("@/pages/world/overview/WorldOverviewPage"),
  worldWorkspace: () => import("@/pages/world/WorldPage"),
  worldStaging: () => import("@/pages/world/WorldStagingRoutePage"),
  worldCompendium: () => import("@/pages/world/WorldCompendiumRoutePage"),
  worldScenes: () => import("@/pages/world/scenes/ScenesRoutePage"),
  worldSceneDetail: () => import("@/pages/world/scenes/SceneDetailRoutePage"),
  worldPlayers: () => import("@/pages/world/players/PlayersRoutePage"),
  actorSelection: () => import("@/pages/world/ActorSelectionPage"),
  actorView: () => import("@/pages/world/actor/ActorDetailPage"),
  actorEdit: () => import("@/pages/world/actor/ActorDetailPage"),
  actorImport: () => import("@/pages/world/actor/import/SheetImportPage"),
  loreEntryView: () => import("@/pages/world/lore/LoreEntryDetailPage"),
  loreEntryEdit: () => import("@/pages/world/lore/LoreEntryDetailPage"),
  loreEntryHistory: () => import("@/pages/world/lore/LoreRevisionHistory"),
  sharedActor: () => import("@/pages/actor-share/SharedActorPage"),
  abilityView: () => import("@/pages/world/ability/AbilityDetailPage"),
  abilityEdit: () => import("@/pages/world/ability/AbilityDetailPage"),
  sharedAbility: () => import("@/pages/ability-share/SharedAbilityPage"),
  itemView: () => import("@/pages/world/item/ItemDetailPage"),
  itemEdit: () => import("@/pages/world/item/ItemDetailPage"),
  worldSystemSettings: () =>
    import("@/pages/world/settings/WorldSystemSettingsPage"),
  sharedItem: () => import("@/pages/item-share/SharedItemPage"),
  instanceInvite: () => import("@/pages/invite/InstanceInvitePage"),
  sharedCollection: () =>
    import("@/pages/collection-share/SharedCollectionPage"),
  worldCollections: () =>
    import("@/pages/world-collections/WorldCollectionsPage"),
  joinWorld: () => import("@/pages/world/JoinWorldPage"),
  status: () => import("@/pages/status/StatusPage"),
  notFound: () => import("@/pages/not-found/NotFoundPage"),
} as const;

/** Every page, each download retried on a dropped connection. */
export const pageLoaders = retried(loaders);

function retried<T extends Record<string, () => Promise<unknown>>>(
  table: T,
): T {
  return Object.fromEntries(
    Object.entries(table).map(([name, load]) => [name, retryImport(load)]),
  ) as T;
}

export type PrefetchablePage = keyof typeof pageLoaders;

export function prefetchPage(page: PrefetchablePage): void {
  void pageLoaders[page]();
}

export function schedulePagePrefetch(pages: readonly PrefetchablePage[]): void {
  if (typeof window === "undefined") {
    return;
  }

  const run = () => {
    pages.forEach(prefetchPage);
  };

  const requestIdle = (
    window as Window & {
      requestIdleCallback?: (callback: () => void) => number;
    }
  ).requestIdleCallback;

  if (requestIdle) {
    requestIdle(run);
    return;
  }

  window.setTimeout(run, 350);
}
