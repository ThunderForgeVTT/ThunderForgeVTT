export interface SeoAssetLink {
  href: string;
  as: "fetch" | "font" | "image" | "script" | "style";
  type?: string;
}

export interface SeoConfig {
  title: string;
  description: string;
  keywords?: string[];
  image?: string;
  canonicalPath?: string;
  noindex?: boolean;
  preloadAssets?: SeoAssetLink[];
  prefetchHrefs?: string[];
}
