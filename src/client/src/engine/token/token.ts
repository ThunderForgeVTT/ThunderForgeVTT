import * as PIXI from "pixi.js";
import { AnimatedSprite } from "@pixi/sprite-animated";
import { Sprite } from "@pixi/sprite";
import { EngineLogger } from "../utils/logger";

export class Token {
  public loaded: boolean = false;
  public sprite: AnimatedSprite | Sprite;
  constructor(
    public name: string,
    public resourceUrl: string,
    public options = { animated: false }
  ) {}

  attach(application: PIXI.Application) {
    application.loader
      .add(this.name, this.resourceUrl)
      .load((loader: any, resources: any) => {
        const SpriteKlass = this.options.animated ? AnimatedSprite : Sprite;
        EngineLogger.info({status: "Loading", name: this.name})
        this.sprite = new SpriteKlass(resources[this.name].texture);
        this.x = application.renderer.width / 2;
        this.y = application.renderer.height / 2;
        // this.sprite.destroy()
        // Rotate around the center
        // this.sprite.anchor.x = 0.5;
        // this.sprite.anchor.y = 0.5;
        // Add the bunny to the scene we are building.
        application.stage.addChild(this.sprite);
        EngineLogger.info({status: "Loaded", name: this.name});
      });
  }

  get x() {
    return this.sprite.x;
  }
  set x(x: number) {
    if (this.loaded) {
      this.sprite.x = x;
    }
  }
  get y() {
      return this.sprite.y
  }
  set y(y: number) {
    if (this.loaded) {
      this.sprite.y = y;
    }
  }
}
