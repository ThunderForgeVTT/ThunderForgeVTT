import { REPO, SPONSORS } from "./links.ts";
import { GitHubMark, StarMark } from "./Icons.tsx";
import { useStars } from "./useStars.ts";

export function Nav() {
  const stars = useStars();
  return (
    <nav className="nav" aria-label="Main">
      <a className="brand" href="/" aria-label="ThunderForge home">
        <img src="/brand-mark.svg" alt="" width="40" height="40" />
        <span>ThunderForge</span>
      </a>
      <ul className="nav-links">
        <li className="wide-only">
          <a href="#dream">The dream</a>
        </li>
        <li className="wide-only">
          <a href="#dice">Dice</a>
        </li>
        <li className="wide-only">
          <a href="#map">Features</a>
        </li>
        <li className="wide-only">
          <a href="#self-host">Self-host</a>
        </li>
        <li>
          <a className="nav-star" data-cta="github" data-placement="nav" href={REPO}>
            <GitHubMark />
            <span className="wide-only">Star</span>
            {stars !== null && (
              <span className="count" aria-label={`${stars} stars`}>
                <StarMark />
                {stars.toLocaleString("en-US")}
              </span>
            )}
          </a>
        </li>
        <li>
          <a className="ink-btn ink-btn--red ink-btn--small" data-cta="sponsor" data-placement="nav" href={SPONSORS}>
            Donate
          </a>
        </li>
      </ul>
    </nav>
  );
}
