import { Helmet } from "react-helmet-async";
import type { SeoConfig } from "@/types/seo";

/**
 * Stands in for the web app's `SEO` (`vite.config.mts` aliases it here).
 *
 * The real one tells a crawler where the page lives on an instance and asks
 * the browser to fetch pages ahead of the visitor. The demo is not an
 * instance's page and must not be indexed as one, and a prefetch of `/login`
 * is a request outside the demo's own files (SC-003). A title is all it
 * keeps.
 */
export function SEO({ title }: SeoConfig) {
  return (
    <Helmet>
      <html lang="en" />
      <title>{`${title} | ThunderForge demo`}</title>
    </Helmet>
  );
}
