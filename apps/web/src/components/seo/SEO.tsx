import { Helmet } from "react-helmet-async";
import { useLocation } from "react-router-dom";
import type { SeoConfig } from "@/types/seo";
import { getDefaultOgImage, toAbsoluteUrl } from "@/utils/env";

const SITE_NAME = "ThunderForge VTT";

export function SEO({
  title,
  description,
  keywords,
  image,
  canonicalPath,
  noindex = false,
  preloadAssets = [],
  prefetchHrefs = [],
}: SeoConfig) {
  const location = useLocation();
  const canonicalUrl = toAbsoluteUrl(canonicalPath ?? location.pathname + location.search);
  const metaTitle = title.includes(SITE_NAME) ? title : `${title} | ${SITE_NAME}`;
  const resolvedImage = toAbsoluteUrl(image ?? getDefaultOgImage());

  return (
    <Helmet prioritizeSeoTags>
      <html lang="en" />
      <title>{metaTitle}</title>
      <meta name="description" content={description} />
      <meta
        name="keywords"
        content={keywords?.join(", ") ?? "virtual tabletop, collaborative worldbuilding"}
      />
      <meta name="robots" content={noindex ? "noindex, nofollow" : "index, follow"} />
      <link rel="canonical" href={canonicalUrl} />
      <meta property="og:site_name" content={SITE_NAME} />
      <meta property="og:type" content="website" />
      <meta property="og:title" content={metaTitle} />
      <meta property="og:description" content={description} />
      <meta property="og:url" content={canonicalUrl} />
      <meta property="og:image" content={resolvedImage} />
      <meta name="twitter:card" content="summary_large_image" />
      <meta name="twitter:title" content={metaTitle} />
      <meta name="twitter:description" content={description} />
      <meta name="twitter:image" content={resolvedImage} />
      {preloadAssets.map((asset) => (
        <link
          key={`preload-${asset.href}`}
          rel="preload"
          href={asset.href}
          as={asset.as}
          type={asset.type}
        />
      ))}
      {prefetchHrefs.map((href) => (
        <link key={`prefetch-${href}`} rel="prefetch" href={href} />
      ))}
    </Helmet>
  );
}
