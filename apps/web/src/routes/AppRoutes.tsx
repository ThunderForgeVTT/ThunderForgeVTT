import { lazy, Suspense } from "react";
import type { ReactNode } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import { Loader } from "@/components/ui/loader/Loader";
import { MainLayout } from "@/layouts/main-layout/MainLayout";
import type { HeaderNavItem } from "@/components/navigation/AppHeader";
import type { SetupStatus } from "@/types/auth";
import { pageLoaders } from "./pageLoaders";

const LoginPage = lazy(pageLoaders.login);
const SignUpPage = lazy(pageLoaders.signup);
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
  return (
    <Suspense fallback={<Loader fullScreen label={label} />}>{page}</Suspense>
  );
}

export default function AppRoutes({
  setupStatus,
  onSetupStatusRefresh,
}: AppRoutesProps) {
  const setupRequired = setupStatus.setup_required;
  const navItems: readonly HeaderNavItem[] = setupRequired
    ? [
        { to: "/setup", label: "Setup", prefetch: "setup", icon: "settings" },
        { to: "/counter", label: "Status", prefetch: "counter", icon: "scene" },
      ]
    : [
        { to: "/login", label: "Login", prefetch: "login", icon: "shield" },
        { to: "/signup", label: "Sign up", prefetch: "signup", icon: "quill" },
        {
          to: "/counter",
          label: "Dashboard",
          prefetch: "counter",
          icon: "scene",
        },
      ];

  return (
    <Routes>
      <Route
        element={
          <MainLayout
            brandHref={setupRequired ? "/setup" : "/login"}
            navItems={navItems}
          />
        }
      >
        <Route
          index
          element={
            <Navigate to={setupRequired ? "/setup" : "/login"} replace />
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
              <Navigate to="/login" replace />
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
              <Navigate to="/login" replace />
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
              <Navigate to="/login" replace />
            )
          }
        />
        <Route
          path="/login"
          element={
            setupRequired ? (
              <Navigate to="/setup" replace />
            ) : (
              renderLazyPage(<LoginPage />, "Loading login screen")
            )
          }
        />
        <Route
          path="/signup"
          element={
            setupRequired ? (
              <Navigate to="/setup" replace />
            ) : (
              renderLazyPage(<SignUpPage />, "Loading sign-up screen")
            )
          }
        />
        <Route
          path="/counter"
          element={renderLazyPage(<CounterPage />, "Loading dashboard")}
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
        element={renderLazyPage(<WorldPage />, "Loading world workspace")}
      />
    </Routes>
  );
}
