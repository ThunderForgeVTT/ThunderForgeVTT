import { lazy, Suspense, type ReactNode } from "react";
import { Navigate, Route, Routes, useLocation } from "react-router-dom";
import { Loader } from "@/components/ui/loader/Loader";
import type { HeaderNavItem } from "@/components/navigation/AppHeader";
import { useAuth } from "@/hooks/useAuth";
import { MainLayout } from "@/layouts/main-layout/MainLayout";
import type { SetupStatus } from "@/types/auth";
import { pageLoaders } from "./pageLoaders";

const LoginPage = lazy(pageLoaders.login);
const RegisterPage = lazy(pageLoaders.signup);
const OAuthCallbackPage = lazy(pageLoaders.oauthCallback);
const DmcaCompliancePage = lazy(pageLoaders.dmcaCompliance);
const AdminSettingsPage = lazy(pageLoaders.adminSettings);
const AdminModerationReviewPage = lazy(pageLoaders.adminModerationReview);
const SetupPage = lazy(pageLoaders.setup);
const SetupCallbackPage = lazy(pageLoaders.setupCallback);
const CounterPage = lazy(pageLoaders.counter);
const WelcomePage = lazy(pageLoaders.welcome);
const WorldListPage = lazy(pageLoaders.worldList);
const CreateWorldPage = lazy(pageLoaders.createWorld);
const WorldDashboardPage = lazy(pageLoaders.worldDashboard);
const WorldPage = lazy(pageLoaders.worldWorkspace);
const WorldStagingRoutePage = lazy(pageLoaders.worldStaging);
const WorldCompendiumRoutePage = lazy(pageLoaders.worldCompendium);
const ScenesRoutePage = lazy(pageLoaders.worldScenes);
const SceneDetailRoutePage = lazy(pageLoaders.worldSceneDetail);
const PlayersRoutePage = lazy(pageLoaders.worldPlayers);
const ActorSelectionPage = lazy(pageLoaders.actorSelection);
const ActorDetailPage = lazy(pageLoaders.actorView);
const LoreEntryDetailPage = lazy(pageLoaders.loreEntryView);
const LoreRevisionHistory = lazy(pageLoaders.loreEntryHistory);
const SharedActorPage = lazy(pageLoaders.sharedActor);
const AbilityDetailPage = lazy(pageLoaders.abilityView);
const SharedAbilityPage = lazy(pageLoaders.sharedAbility);
const ItemDetailPage = lazy(pageLoaders.itemView);
const SharedItemPage = lazy(pageLoaders.sharedItem);
const WorldSystemSettingsPage = lazy(pageLoaders.worldSystemSettings);
const JoinWorldPage = lazy(pageLoaders.joinWorld);
const NotFoundPage = lazy(pageLoaders.notFound);

interface AppRoutesProps {
  setupStatus: SetupStatus;
  onSetupStatusRefresh: () => Promise<SetupStatus>;
}

function renderLazyPage(page: ReactNode, label: string) {
  return (
    <Suspense fallback={<Loader fullScreen label={label} />}>{page}</Suspense>
  );
}

function RequireAuthenticated({ children }: { children: ReactNode }) {
  const location = useLocation();
  const { isAuthenticated, isLoading } = useAuth();

  if (isLoading) {
    return <Loader fullScreen label="Restoring session" />;
  }

  if (!isAuthenticated) {
    const returnTo = `${location.pathname}${location.search}${location.hash}`;
    return (
      <Navigate
        to={`/login?returnTo=${encodeURIComponent(returnTo)}`}
        replace
      />
    );
  }

  return <>{children}</>;
}

function RequireAdmin({ children }: { children: ReactNode }) {
  const location = useLocation();
  const { isAdmin, isAuthenticated, isLoading } = useAuth();

  if (isLoading) {
    return <Loader fullScreen label="Restoring session" />;
  }

  if (!isAuthenticated) {
    const returnTo = `${location.pathname}${location.search}${location.hash}`;
    return (
      <Navigate
        to={`/login?returnTo=${encodeURIComponent(returnTo)}`}
        replace
      />
    );
  }

  if (!isAdmin) {
    return <Navigate to="/welcome" replace />;
  }

  return <>{children}</>;
}

/** Spec 008 (US2, FR-012): mirrors LoginView.tsx's/RegisterPage.tsx's own
 * `redirectTarget` helper. Without this, an already-authenticated visitor
 * who transiently lands on `/login?returnTo=/join/xyz` (e.g. JoinWorldPage
 * redirecting there before its own `isAuthenticated` read has caught up
 * with a just-completed login/register) gets bounced to `authenticatedHome`
 * unconditionally, silently dropping the invite code they were mid-way
 * through redeeming. */
function returnToFromSearch(search: string): string | null {
  const params = new URLSearchParams(search);
  const returnTo = params.get("returnTo");
  return returnTo && returnTo.startsWith("/") ? returnTo : null;
}

export default function AppRoutes({
  setupStatus,
  onSetupStatusRefresh,
}: AppRoutesProps) {
  const { isAdmin, isAuthenticated, isLoading, redirectAfterLogin } = useAuth();
  const location = useLocation();
  const setupRequired = setupStatus.setup_required;

  if (!setupRequired && isLoading) {
    return <Loader fullScreen label="Restoring session" />;
  }

  const authenticatedHome = redirectAfterLogin();
  const publicHome =
    isAuthenticated && !isLoading ? authenticatedHome : "/login";

  const navItems: readonly HeaderNavItem[] = setupRequired
    ? [
        { to: "/setup", label: "Setup", prefetch: "setup", icon: "settings" },
        { to: "/counter", label: "Status", prefetch: "counter", icon: "scene" },
      ]
      : isAuthenticated && isAdmin
      ? [
          {
            to: "/admin",
            label: "Admin",
            prefetch: "adminSettings",
            icon: "crown",
          },
          {
            to: "/counter",
            label: "Preview",
            prefetch: "counter",
            icon: "scene",
          },
          {
            to: "/worlds",
            label: "Worlds",
            prefetch: "worldList",
            icon: "worlds",
          },
        ]
      : isAuthenticated
        ? [
            {
              to: "/welcome",
              label: "Welcome",
              prefetch: "welcome",
              icon: "scene",
            },
            {
              to: "/counter",
              label: "Preview",
              prefetch: "counter",
              icon: "spark",
            },
            {
              to: "/worlds",
              label: "Worlds",
              prefetch: "worldList",
              icon: "worlds",
            },
          ]
        : [
            { to: "/login", label: "Login", prefetch: "login", icon: "shield" },
            {
              to: "/register",
              label: "Register",
              prefetch: "signup",
              icon: "quill",
            },
          ];

  return (
    <Routes>
      <Route
        element={
          <MainLayout
            brandHref={
              setupRequired
                ? "/setup"
                : isAuthenticated
                  ? authenticatedHome
                  : "/login"
            }
            navItems={navItems}
          />
        }
      >
        <Route
          index
          element={
            <Navigate to={setupRequired ? "/setup" : publicHome} replace />
          }
        />
        <Route
          path="/setup/:code"
          element={
            setupRequired ? (
              renderLazyPage(
                <SetupPage
                  setupStatus={setupStatus}
                  onSetupComplete={onSetupStatusRefresh}
                />,
                "Loading setup workspace",
              )
            ) : (
              <Navigate to={publicHome} replace />
            )
          }
        />
        <Route
          path="/setup"
          element={
            setupRequired ? (
              renderLazyPage(
                <SetupPage
                  setupStatus={setupStatus}
                  onSetupComplete={onSetupStatusRefresh}
                />,
                "Loading setup workspace",
              )
            ) : (
              <Navigate to={publicHome} replace />
            )
          }
        />
        <Route
          path="/setup/callback"
          element={
            setupRequired ? (
              renderLazyPage(
                <SetupCallbackPage onSetupComplete={onSetupStatusRefresh} />,
                "Finishing setup",
              )
            ) : (
              <Navigate to={publicHome} replace />
            )
          }
        />
        <Route
          path="/login"
          element={
            setupRequired ? (
              <Navigate to="/setup" replace />
            ) : isAuthenticated ? (
              <Navigate
                to={returnToFromSearch(location.search) ?? authenticatedHome}
                replace
              />
            ) : (
              renderLazyPage(<LoginPage />, "Loading login screen")
            )
          }
        />
        <Route
          path="/register"
          element={
            setupRequired ? (
              <Navigate to="/setup" replace />
            ) : isAuthenticated ? (
              <Navigate
                to={returnToFromSearch(location.search) ?? authenticatedHome}
                replace
              />
            ) : (
              renderLazyPage(<RegisterPage />, "Loading registration screen")
            )
          }
        />
        <Route path="/signup" element={<Navigate to="/register" replace />} />
        <Route
          path="/oauth/callback/:providerKey"
          element={renderLazyPage(
            <OAuthCallbackPage />,
            "Completing OAuth sign-in",
          )}
        />
        <Route
          path="/legal/dmca"
          element={renderLazyPage(
            <DmcaCompliancePage />,
            "Loading DMCA policy",
          )}
        />
        <Route
          path="/admin"
          element={
            <RequireAdmin>
              {renderLazyPage(
                <AdminSettingsPage initialSection="overview" />,
                "Loading admin command center",
              )}
            </RequireAdmin>
          }
        />
        <Route path="/admin/welcome" element={<Navigate to="/admin" replace />} />
        <Route
          path="/admin/settings"
          element={<Navigate to="/admin" replace />}
        />
        <Route
          path="/admin/configuration"
          element={
            <RequireAdmin>
              {renderLazyPage(
                <AdminSettingsPage initialSection="configuration" />,
                "Loading admin configuration",
              )}
            </RequireAdmin>
          }
        />
        <Route
          path="/admin/analytics"
          element={<Navigate to="/admin/storage" replace />}
        />
        <Route
          path="/admin/storage"
          element={
            <RequireAdmin>
              {renderLazyPage(
                <AdminSettingsPage initialSection="storage" />,
                "Loading admin analytics",
              )}
            </RequireAdmin>
          }
        />
        <Route
          path="/admin/oauth"
          element={<Navigate to="/admin/configuration" replace />}
        />
        <Route
          path="/admin/system"
          element={<Navigate to="/admin/security" replace />}
        />
        <Route
          path="/admin/security"
          element={
            <RequireAdmin>
              {renderLazyPage(
                <AdminSettingsPage initialSection="security" />,
                "Loading admin security",
              )}
            </RequireAdmin>
          }
        />
        <Route
          path="/admin/moderation"
          element={
            <RequireAdmin>
              {renderLazyPage(<AdminModerationReviewPage />, "Loading moderation review")}
            </RequireAdmin>
          }
        />
        <Route
          path="/welcome"
          element={
            <RequireAuthenticated>
              {isAdmin ? (
                <Navigate to="/admin" replace />
              ) : (
                renderLazyPage(<WelcomePage />, "Loading welcome page")
              )}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/counter"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<CounterPage />, "Loading dashboard")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/worlds"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<WorldListPage />, "Loading world archive")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/worlds/create"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<CreateWorldPage />, "Loading world creation")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id"
          element={
            <RequireAuthenticated>
              {renderLazyPage(
                <WorldDashboardPage />,
                "Loading world dashboard",
              )}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id/staging"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<WorldStagingRoutePage />, "Loading world staging")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id/actor-select"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<ActorSelectionPage />, "Loading Actor Selection")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id/compendium"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<WorldCompendiumRoutePage />, "Loading world compendium")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id/scenes"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<ScenesRoutePage />, "Loading scenes")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id/scenes/:sceneId"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<SceneDetailRoutePage />, "Loading scene")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id/players"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<PlayersRoutePage />, "Loading players")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id/actor/:actorId/view"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<ActorDetailPage mode="view" />, "Loading actor")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id/actor/:actorId/edit"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<ActorDetailPage mode="edit" />, "Loading actor")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id/lore/:slug/view"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<LoreEntryDetailPage mode="view" />, "Loading lore entry")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id/lore/:slug/edit"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<LoreEntryDetailPage mode="edit" />, "Loading lore entry")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id/lore/:slug/history"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<LoreRevisionHistory />, "Loading revision history")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/shared/actor/:code"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<SharedActorPage />, "Loading shared actor")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id/ability/:abilityId/view"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<AbilityDetailPage mode="view" />, "Loading ability")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id/ability/:abilityId/edit"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<AbilityDetailPage mode="edit" />, "Loading ability")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id/item/:itemId/view"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<ItemDetailPage mode="view" />, "Loading item")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id/item/:itemId/edit"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<ItemDetailPage mode="edit" />, "Loading item")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/shared/ability/:code"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<SharedAbilityPage />, "Loading shared ability")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/shared/item/:code"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<SharedItemPage />, "Loading shared item")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="/world/:id/settings/system"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<WorldSystemSettingsPage />, "Loading system settings")}
            </RequireAuthenticated>
          }
        />
        <Route
          path="*"
          element={renderLazyPage(
            <NotFoundPage setupRequired={setupRequired} />,
            "Loading page",
          )}
        />
      </Route>
      <Route
        path="/world/:id/play"
        element={
          <RequireAuthenticated>
            {renderLazyPage(<WorldPage />, "Loading world workspace")}
          </RequireAuthenticated>
        }
      />
      <Route
        path="/join/:code"
        element={
          <RequireAuthenticated>
            {renderLazyPage(<JoinWorldPage />, "Loading campaign preview")}
          </RequireAuthenticated>
        }
      />
    </Routes>
  );
}
