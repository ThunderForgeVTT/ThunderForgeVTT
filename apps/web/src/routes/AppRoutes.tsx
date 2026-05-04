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
const AdminWelcomePage = lazy(pageLoaders.adminWelcome);
const AdminSettingsPage = lazy(pageLoaders.adminSettings);
const SetupPage = lazy(pageLoaders.setup);
const SetupCallbackPage = lazy(pageLoaders.setupCallback);
const CounterPage = lazy(pageLoaders.counter);
const WelcomePage = lazy(pageLoaders.welcome);
const WorldListPage = lazy(pageLoaders.worldList);
const CreateWorldPage = lazy(pageLoaders.createWorld);
const WorldDashboardPage = lazy(pageLoaders.worldDashboard);
const WorldPage = lazy(pageLoaders.worldWorkspace);
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

export default function AppRoutes({
  setupStatus,
  onSetupStatusRefresh,
}: AppRoutesProps) {
  const { isAdmin, isAuthenticated, isLoading, redirectAfterLogin } = useAuth();
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
            to: "/admin/welcome",
            label: "Admin",
            prefetch: "adminWelcome",
            icon: "crown",
          },
          {
            to: "/admin/settings",
            label: "Settings",
            prefetch: "adminSettings",
            icon: "settings",
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
              <Navigate to={authenticatedHome} replace />
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
              <Navigate to={authenticatedHome} replace />
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
          path="/admin/welcome"
          element={
            <RequireAdmin>
              {renderLazyPage(<AdminWelcomePage />, "Loading admin welcome")}
            </RequireAdmin>
          }
        />
        <Route
          path="/admin/settings"
          element={
            <RequireAdmin>
              {renderLazyPage(
                <AdminSettingsPage initialSection="overview" />,
                "Loading admin settings",
              )}
            </RequireAdmin>
          }
        />
        <Route
          path="/admin/analytics"
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
          element={
            <RequireAdmin>
              {renderLazyPage(
                <AdminSettingsPage initialSection="configuration" />,
                "Loading OAuth settings",
              )}
            </RequireAdmin>
          }
        />
        <Route
          path="/admin/system"
          element={
            <RequireAdmin>
              {renderLazyPage(
                <AdminSettingsPage initialSection="configuration" />,
                "Loading system settings",
              )}
            </RequireAdmin>
          }
        />
        <Route
          path="/welcome"
          element={
            <RequireAuthenticated>
              {isAdmin ? (
                <Navigate to="/admin/welcome" replace />
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
    </Routes>
  );
}
