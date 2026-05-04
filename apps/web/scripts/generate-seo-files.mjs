import fs from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const appRoot = process.cwd();
const publicDir = path.join(appRoot, "public");
const siteUrl = new URL(process.env.VITE_SITE_URL ?? "https://example.com");
const routes = ["/", "/login", "/signup", "/setup", "/counter"];

const sitemap = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${routes
  .map(
    (route) => `  <url>
    <loc>${new URL(route, siteUrl).toString()}</loc>
    <changefreq>weekly</changefreq>
    <priority>${route === "/" ? "1.0" : "0.8"}</priority>
  </url>`,
  )
  .join("\n")}
</urlset>
`;

const robots = `User-agent: *
Allow: /

Sitemap: ${new URL("/sitemap.xml", siteUrl).toString()}
`;

await fs.mkdir(publicDir, { recursive: true });
await Promise.all([
  fs.writeFile(path.join(publicDir, "sitemap.xml"), sitemap, "utf8"),
  fs.writeFile(path.join(publicDir, "robots.txt"), robots, "utf8"),
]);
