import { lazy, Suspense } from "react";
import { Hero } from "./sections/Hero.tsx";
import { Dream } from "./sections/Dream.tsx";
import { Systems } from "./sections/Systems.tsx";
import { Numbers } from "./sections/Numbers.tsx";
import { SelfHost } from "./sections/SelfHost.tsx";
import { Stance } from "./sections/Stance.tsx";
import { WhatWeMeasure } from "./sections/WhatWeMeasure.tsx";
import { Support } from "./sections/Support.tsx";
import { Footer } from "./sections/Footer.tsx";

// The dice (WebAssembly) and the map (a large image and its geometry) load
// after the first viewport, each in its own chunk.
const DiceTray = lazy(() => import("./sections/DiceTray.tsx"));
const MapLegend = lazy(() => import("./sections/MapLegend.tsx"));
const StarChart = lazy(() => import("./sections/StarChart.tsx"));

export function App() {
  return (
    <>
      <Hero />
      <main>
        <Dream />
        <section id="dice" data-section="dice" className="section dice-section" aria-labelledby="dice-title">
          <div className="dice-copy">
            <h2 id="dice-title" className="marker-head">
              Roll the real dice.
            </h2>
            <p className="lede">
              These are the server's own dice: the same Rust code that settles every roll at the
              table, compiled to WebAssembly and running in your browser right now. Keep the
              highest, drop the lowest, Fate dice and more.
            </p>
          </div>
          <Suspense fallback={<div className="tray tray--loading" />}>
            <DiceTray />
          </Suspense>
        </section>
        <section id="map" data-section="map" className="section map-section" aria-labelledby="map-title">
          <h2 id="map-title" className="marker-head">
            Everything on the map is real.
          </h2>
          <Suspense fallback={<div className="legend-layout legend-layout--loading" />}>
            <MapLegend />
          </Suspense>
        </section>
        <Systems />
        <Numbers />
        <SelfHost />
        <WhatWeMeasure />
        <Stance />
        <Suspense fallback={<div className="section stars" />}>
          <StarChart />
        </Suspense>
        <Support />
      </main>
      <Footer />
    </>
  );
}
