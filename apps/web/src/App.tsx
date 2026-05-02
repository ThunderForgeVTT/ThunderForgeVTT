import React from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import LoginView from "./views/LoginView";
import SignUpView from "./views/SignUpView";
import CounterView from "./views/CounterView";
import WorldView from "./views/WorldView";

export default function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginView />} />
      <Route path="/signup" element={<SignUpView />} />
      <Route path="/counter" element={<CounterView />} />
      <Route path="/world/:id" element={<WorldView />} />
      <Route path="*" element={<Navigate to="/login" replace />} />
    </Routes>
  );
}