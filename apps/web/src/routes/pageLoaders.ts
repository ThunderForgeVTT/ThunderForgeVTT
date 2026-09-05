export const pageLoaders = {
  login: () => import("@/pages/auth/LoginPage"),
  signup: () => import("@/pages/auth/RegisterPage"),
  oauthCallback: () => import("@/pages/auth/OAuthCallbackPage"),
  dmcaCompliance: () => import("@/pages/legal/DmcaCompliancePage"),
  termsOfService: () => import("@/pages/legal/TermsOfServicePage"),
  privacyPolicy: () => import("@/pages/legal/PrivacyPolicyPage"),
  adminWelcome: () => import("@/pages/admin/AdminWelcomePage"),
  adminSettings: () => import("@/pages/admin/SettingsPage"),
  adminModerationReview: () => import("@/pages/admin/ModerationReviewPage"),
  setup: () => import("@/pages/setup/SetupPage"),
  setupCallback: () => import("@/pages/setup/SetupCallbackPage"),
  counter: () => import("@/pages/counter/CounterPage"),
  welcome: () => import("@/pages/user/WelcomePage"),
  storageSettings: () => import("@/pages/user/StorageSettingsPage"),
  worldList: () => import("@/pages/world/WorldListPage"),
  createWorld: () => import("@/pages/world/CreateWorldPage"),
  worldDashboard: () => import("@/pages/world/WorldDashboardPage"),
  worldWorkspace: () => import("@/pages/world/WorldPage"),
  worldStaging: () => import("@/pages/world/WorldStagingRoutePage"),
  worldCompendium: () => import("@/pages/world/WorldCompendiumRoutePage"),
  worldScenes: () => import("@/pages/world/scenes/ScenesRoutePage"),
  worldSceneDetail: () => import("@/pages/world/scenes/SceneDetailRoutePage"),
  worldPlayers: () => import("@/pages/world/players/PlayersRoutePage"),
  actorSelection: () => import("@/pages/world/ActorSelectionPage"),
  actorView: () => import("@/pages/world/actor/ActorDetailPage"),
  actorEdit: () => import("@/pages/world/actor/ActorDetailPage"),
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
  sharedCollection: () =>
    import("@/pages/collection-share/SharedCollectionPage"),
  worldCollections: () =>
    import("@/pages/world-collections/WorldCollectionsPage"),
  joinWorld: () => import("@/pages/world/JoinWorldPage"),
  status: () => import("@/pages/status/StatusPage"),
  notFound: () => import("@/pages/not-found/NotFoundPage"),
} as const;

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
