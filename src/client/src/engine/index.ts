import { AnimatedSprite } from "@pixi/sprite-animated";
import { EngineLogger } from "./utils/logger";
import { Canvas } from "./canvas/canvas";
import { Token } from "./token/token";

EngineLogger.info("Loading Engine...");

export async function load() {
  EngineLogger.info("Calling load method");
  // The application will create a renderer using WebGL, if possible,
  // with a fallback to a canvas render. It will also setup the ticker
  // and the root stage PIXI.Container.

  // The application will create a canvas element for you that you
  // can then insert into the DOM.
  EngineLogger.info("Looking for engine element");
  let element = document.querySelector("#engine");
  EngineLogger.info(`element: ${element}`);

  if (element) {
    const canvas = new Canvas();
    canvas.create(element);

    // const bunny = new Token("bunny", "/assets/gifs/bunny.gif");
    // canvas.addToken(bunny);

    // // Listen for frame updates
    // app.ticker.add(() => {
    //   // each frame we spin the bunny around a bit
    //   bunny.rotation += 0.01;
    // });
  }
  return "";
}
