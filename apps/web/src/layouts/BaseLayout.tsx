import { Link, Outlet, useLocation } from "react-router-dom";

export default function BaseLayout() {
  const location = useLocation();
  const isSetupFlow = location.pathname.startsWith("/setup");

  return (
    <div className="grid min-h-screen grid-rows-[auto_1fr]">
      <header className="border-b border-border px-6 py-4">
        <Link
          to={isSetupFlow ? "/setup" : "/login"}
          className="text-lg font-semibold text-foreground"
        >
          ThunderForge
        </Link>
      </header>
      <main className="flex flex-col">
        <Outlet />
      </main>
    </div>
  );
}
