import * as PIXI from "pixi.js";
import { EngineLogger } from "../utils/logger";
import { Token } from "../token/token";
import {string2hex, TextureCache} from "@pixi/utils";
import {CompositeTilemap, settings} from "@pixi/tilemap";
import {Layer} from "@pixi/layers";

export class Canvas {
  private application = new PIXI.Application({
    resizeTo: window,
    autoDensity: true,
    backgroundColor: string2hex("rgb(34, 40, 49)"),
    sharedLoader: true,
  });
  public tokens: Token[] = [];

  public create(element: Element) {
    EngineLogger.info("Rendering Canvas");
    settings.use32bitIndex = true;

    let assets = [
        "/assets/gifs/bunny.gif",
        "/assets/jpg/grass-tile.jpg"
    ]
    this.application.loader.add(assets);

    const layer = new Layer();
    const token_layer = new Layer();
    layer.zIndex = 1;
    token_layer.zIndex = 2;
    token_layer.parent = layer;

    const base_tilemap = new CompositeTilemap();
    layer.addChild(base_tilemap);



    const token_tilemap = new CompositeTilemap();
    token_layer.addChild(token_tilemap)
    this.application.stage.addChild(layer);


    EngineLogger.info(this.application.loader.resources)

    this.application.loader.load(()=> {
      base_tilemap.addFrame("/assets/jpg/grass-tile.jpg", 0, 0)
    })
    this.application.loader.load(()=>{
      token_tilemap.addFrame("/assets/gifs/bunny.gif", 0, 0)
    })

    element.appendChild(this.application.view);
  }
  public addToken(token: Token) {
    token.attach(this.application);
    this.tokens.push(token);
  }
}
