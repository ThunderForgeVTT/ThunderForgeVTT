import { CC_BY_SA, MAPS_SOURCE, REPO } from "../links.ts";

export function Footer() {
  return (
    <footer className="footer" data-section="footer">
      <div className="footer-brand">
        <img src="/brand-mark.svg" alt="" width="32" height="32" />
        <span>ThunderForgeVTT</span>
      </div>
      <p>
        Open source under the{" "}
        <a href={`${REPO}/blob/main/LICENSE`}>GNU AGPL v3.0 or later</a>.{" "}
        <a href={REPO}>Source on GitHub</a>.
      </p>
      <p>
        Example map by MBRound18, from <a href={MAPS_SOURCE}>vtt-maps</a>, shared under{" "}
        <a href={CC_BY_SA}>CC BY-SA 4.0</a>. Tokens drawn by the hero generator in this repository.
      </p>
    </footer>
  );
}
