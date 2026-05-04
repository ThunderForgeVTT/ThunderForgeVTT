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
const SetupPage = lazy(pageLoaders.setup);
const SetupCallbackPage = lazy(pageLoaders.setupCallback);
const CounterPage = lazy(pageLoaders.counter);
const WorldPage = lazy(pageLoaders.world);
const NotFoundPage = lazy(pageLoaders.notFound);

interface AppRoutesProps {
  setupStatus: SetupStatus;
  onSetupStatusRefresh: () => Promise<SetupStatus>;
}

function renderLazyPage(page: ReactNode, label: string) {
  return <Suspense fallback={<Loader fullScreen label={label} />}>{page}</Suspense>;
}

function RequireAuthenticated({ children }: { children: ReactNode }) {
  const location = useLocation();
  const { isAuthenticated, isLoading } = useAuth();

  if (isLoading) {
    return <Loader fullScreen label="Restoring session" />;
  }

  if (!isAuthenticated) {
    const returnTo = `${location.pathname}${location.search}${location.hash}`;
    return <Navigate to={`/login?returnTo=${encodeURIComponent(returnTo)}`} replace />;
  }

  return <>{children}</>;
}

export default function AppRoutes({
  setupStatus,
  onSetupStatusRefresh,
}: AppRoutesProps) {
  const { isAuthenticated, isLoading } = useAuth();
  const setupRequired = setupStatus.setup_required;

  if (!setupRequired && isLoading) {
    return <Loader fullScreen label="Restoring session" />;
  }

  const navItems: readonly HeaderNavItem[] = setupRequired
    ? [
        { to: "/setup", label: "Setup", prefetch: "setup", icon: "settings" },
        { to: "/counter", label: "Status", prefetch: "counter", icon: "scene" },
      ]
    : isAuthenticated
      ? [
          { to: "/counter", label: "Dashboard", prefetch: "counter", icon: "scene" },
          { to: "/world/demo-world", label: "World", prefetch: "world", icon: "worlds" },
        ]
      : [
          { to: "/login", label: "Login", prefetch: "login", icon: "shield" },
          { to: "/register", label: "Register", prefetch: "signup", icon: "quill" },
        ];

  const publicHome = isAuthenticated && !isLoading ? "/counter" : "/login";

  return (
    <Routes>
      <Route
        element={
          <MainLayout
            brandHref={setupRequired ? "/setup" : isAuthenticated ? "/counter" : "/login"}
            navItems={navItems}
          />
        }
      >
        <Route
          index
          element={<Navigate to={setupRequired ? "/setup" : publicHome} replace />}
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
              <Navigate to="/counter" replace />
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
              <Navigate to="/counter" replace />
            ) : (
              renderLazyPage(<RegisterPage />, "Loading registration screen")
            )
          }
        />
        <Route path="/signup" element={<Navigate to="/register" replace />} />
        <Route
          path="/counter"
          element={
            <RequireAuthenticated>
              {renderLazyPage(<CounterPage />, "Loading dashboard")}
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
        path="/world/:id"
        element={
          <RequireAuthenticated>
            {renderLazyPage(<WorldPage />, "Loading world workspace")}
          </RequireAuthenticated>
        }
      />
    </Routes>
  );
}
