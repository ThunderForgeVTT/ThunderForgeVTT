import React from "react";
import { Link, Outlet, useLocation } from "react-router-dom";

export default function BaseLayout() {
  const location = useLocation();
  const isSetupFlow = location.pathname.startsWith("/setup");

  return (
    <div className="base-layout">
      <header className="base-layout__header">
        <Link to={isSetupFlow ? "/setup" : "/login"} className="base-layout__brand">
          ThunderForge
        </Link>
      </header>
      <main className="base-layout__main">
        <Outlet />
      </main>
    </div>
  );
}
