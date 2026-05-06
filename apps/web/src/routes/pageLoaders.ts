export const pageLoaders = {
  login: () => import("@/pages/auth/LoginPage"),
  signup: () => import("@/pages/auth/RegisterPage"),
  oauthCallback: () => import("@/pages/auth/OAuthCallbackPage"),
  adminWelcome: () => import("@/pages/admin/AdminWelcomePage"),
  adminSettings: () => import("@/pages/admin/SettingsPage"),
  setup: () => import("@/pages/setup/SetupPage"),
  setupCallback: () => import("@/pages/setup/SetupCallbackPage"),
  counter: () => import("@/pages/counter/CounterPage"),
  welcome: () => import("@/pages/user/WelcomePage"),
  worldList: () => import("@/pages/world/WorldListPage"),
  createWorld: () => import("@/pages/world/CreateWorldPage"),
  worldDashboard: () => import("@/pages/world/WorldDashboardPage"),
  worldWorkspace: () => import("@/pages/world/WorldPage"),
  joinWorld: () => import("@/pages/world/JoinWorldPage"),
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
