import React, { Suspense, lazy } from "react";
import { Navigate, Route, Routes } from "react-router-dom";

const LoginView = lazy(() => import("./views/LoginView"));
const SignUpView = lazy(() => import("./views/SignUpView"));
const CounterView = lazy(() => import("./views/CounterView"));
const WorldView = lazy(() => import("./views/WorldView"));

export default function App() {
  return (
    <Suspense fallback={null}>
      <Routes>
        <Route path="/login" element={<LoginView />} />
        <Route path="/signup" element={<SignUpView />} />
        <Route path="/counter" element={<CounterView />} />
        <Route path="/world/:id" element={<WorldView />} />
        <Route path="*" element={<Navigate to="/login" replace />} />
      </Routes>
    </Suspense>
  );
}