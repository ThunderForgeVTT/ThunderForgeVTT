import { DISCUSSIONS, KOFI, PROJECTS, REPO, SPONSORS } from "../links.ts";
import { CupMark, GitHubMark, HeartMark, StarMark } from "../Icons.tsx";
import { useStars } from "../useStars.ts";

export function Support() {
  const stars = useStars();
  return (
    <section id="support" data-section="support" className="section support" aria-labelledby="support-title">
      <h2 id="support-title" className="marker-head marker-head--big">
        Pull up a chair.
      </h2>
      <p className="lede">
        This isn't a cry for money. ThunderForge is one person's passion, worked on every day,
        and it will take time. If you want to see it reach your table, here is how to help it get
        there faster.
      </p>
      <div className="support-ways">
        <div className="way">
          <h3>Fund the work</h3>
          <p>Every sponsor buys hours away from the day job and toward the table.</p>
          <div className="way-actions">
            <a className="ink-btn ink-btn--red" data-cta="sponsor" data-placement="support" href={SPONSORS}>
              <HeartMark />
              Sponsor on GitHub
            </a>
            <a className="ink-btn ink-btn--red-line" href={KOFI}>
              <CupMark />
              Buy a coffee on Ko-fi
            </a>
          </div>
        </div>
        <div className="way">
          <h3>Follow the build</h3>
          <p>A star tells other players this is worth a look. Progress happens in the open.</p>
          <div className="way-actions">
            <a className="ink-btn ink-btn--black" data-cta="github" data-placement="support" href={REPO}>
              <StarMark />
              Star on GitHub
              {stars !== null && <span className="count">{stars.toLocaleString("en-US")}</span>}
            </a>
            <a className="ink-btn ink-btn--line" href={PROJECTS}>
              Watch the roadmap
            </a>
          </div>
        </div>
        <div className="way">
          <h3>Tell us how you play</h3>
          <p>Your table, your system, your connection. It shapes what gets built next.</p>
          <div className="way-actions">
            <a className="ink-btn ink-btn--blue-line" href={DISCUSSIONS}>
              <GitHubMark />
              Join the discussions
            </a>
          </div>
        </div>
      </div>
      <p className="thanks">Thank you for your patience. See you at the table.</p>
    </section>
  );
}
