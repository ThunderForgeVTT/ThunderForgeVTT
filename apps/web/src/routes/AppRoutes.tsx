import React, { lazy } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import { SetupStatus } from "../api/auth";
import BaseLayout from "../layouts/BaseLayout";

const LoginView = lazy(() => import("../views/LoginView"));
const SignUpView = lazy(() => import("../views/SignUpView"));
const SetupView = lazy(() => import("../views/SetupView"));
const SetupCallbackView = lazy(() => import("../views/SetupCallbackView"));
const CounterView = lazy(() => import("../views/CounterView"));
const WorldView = lazy(() => import("../views/WorldView"));

interface AppRoutesProps {
  setupStatus: SetupStatus;
  onSetupStatusRefresh: () => Promise<SetupStatus>;
}

export default function AppRoutes({
  setupStatus,
  onSetupStatusRefresh,
}: AppRoutesProps) {
  const setupRequired = setupStatus.setup_required;

  return (
    <Routes>
      <Route element={<BaseLayout />}>
        <Route
          path="/setup"
          element={
            setupRequired ? (
              <SetupView
                setupStatus={setupStatus}
                onSetupComplete={onSetupStatusRefresh}
              />
            ) : (
              <Navigate to="/login" replace />
            )
          }
        />
        <Route
          path="/setup/callback"
          element={
            setupRequired ? (
              <SetupCallbackView onSetupComplete={onSetupStatusRefresh} />
            ) : (
              <Navigate to="/login" replace />
            )
          }
        />
        <Route
          path="/login"
          element={
            setupRequired ? <Navigate to="/setup" replace /> : <LoginView />
          }
        />
        <Route
          path="/signup"
          element={
            setupRequired ? <Navigate to="/setup" replace /> : <SignUpView />
          }
        />
        <Route path="/counter" element={<CounterView />} />
      </Route>
      <Route path="/world/:id" element={<WorldView />} />
      <Route
        path="*"
        element={<Navigate to={setupRequired ? "/setup" : "/login"} replace />}
      />
    </Routes>
  );
}
