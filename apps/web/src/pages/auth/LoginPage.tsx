import { SEO } from "@/components/seo/SEO";
import type { SeoConfig } from "@/types/seo";
import { LoginView } from "./LoginView";

export const loginPageSeo: SeoConfig = {
  title: "Login",
  description:
    "Access ThunderForge VTT to manage your instance, review setup status, and enter collaborative worlds.",
  keywords: [
    "ThunderForge login",
    "virtual tabletop login",
    "tabletop control room",
  ],
  canonicalPath: "/login",
  prefetchHrefs: ["/register", "/welcome"],
};

export default function LoginPage() {
  return (
    <>
      <SEO {...loginPageSeo} />
      <LoginView />
    </>
  );
}
