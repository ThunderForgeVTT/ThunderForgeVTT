import * as PIXI from "pixi.js";
import { EngineLogger } from "../utils/logger";
import { Token } from "../token/token";

export class Canvas {
  private application = new PIXI.Application();
  public tokens: Token[] = [];

  public create(element: Element) {
    EngineLogger.info("Rendering Canvas");
    element.appendChild(this.application.view);
  }
  public addToken(token: Token) {
    token.attach(this.application);
    this.tokens.push(token);
  }
}
