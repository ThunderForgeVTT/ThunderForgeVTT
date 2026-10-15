import { useMemo } from "react";
import { createHero, monsterPack, PRESET_HEROES } from "@thunderforge/heroes";
import { Nav } from "../Nav.tsx";
import { DEMO, REPO, SPONSORS } from "../links.ts";
import { ArrowMark, HeartMark, StarMark } from "../Icons.tsx";
import { HeroTavern, type TavernPiece } from "./HeroTavern.tsx";

// The page opens in the tavern, with the whole party inside
// (`HeroTavern.tsx`): beside the headline on a wide page, under it on a
// narrow one. Every adventure starts in a tavern.

const PARTY = ["sir-pip", "mira-starweave", "fenna-swiftbow", "brother-oak"];

function svgSrc(svg: string) {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

function useParty(): { party: TavernPiece[]; goblins: TavernPiece[] } {
  return useMemo(() => {
    const party = PARTY.map((slug) => PRESET_HEROES.find((p) => p.slug === slug))
      .filter((p) => p !== undefined)
      .map((p) => ({
        id: p.slug,
        name: p.spec.name ?? p.slug,
        src: svgSrc(createHero(p.spec).token()),
      }));
    const goblins = monsterPack({ name: "Goblin", descriptor: "Small humanoid" }, 3).map(
      (g, i) => ({
        id: `goblin-${i}`,
        name: `Goblin ${i + 1}`,
        src: svgSrc(g.token({ idPrefix: `gob${i}` })),
      }),
    );
    return { party, goblins };
  }, []);
}

export function Hero() {
  const { party, goblins } = useParty();
  return (
    <header className="hero hero--tavern" data-section="hero">
      <Nav />
      <div className="hero-body">
        <div className="room">
          <h1>
            Get back to the <span className="ink-blue">fantasy.</span>
          </h1>
          <p className="offer">
            ThunderForge is a free, open source virtual tabletop you host yourself.{" "}
            <span className="offer-more">
              Built for tables that meet in person, online, or both at once, and built to stay out
              of the story's way.
            </span>
          </p>
          <div className="actions">
            <a className="ink-btn ink-btn--blue" data-cta="try_demo" data-placement="hero" href={DEMO}>
              Play the demo world
              <ArrowMark />
            </a>
            <div className="actions-pair">
              <a className="ink-btn ink-btn--red-line" data-cta="sponsor" data-placement="hero" href={SPONSORS}>
                <HeartMark />
                Donate
              </a>
              <a className="ink-btn ink-btn--line" data-cta="github" data-placement="hero" href={REPO}>
                <StarMark />
                <span>
                  Star<span className="wide-only"> on GitHub</span>
                </span>
              </a>
            </div>
          </div>
          <p className="aside">
            Not released yet. Built in the open, every day, by one person who loves this hobby.
          </p>
        </div>
        <HeroTavern party={party} foes={goblins} />
      </div>
    </header>
  );
}
