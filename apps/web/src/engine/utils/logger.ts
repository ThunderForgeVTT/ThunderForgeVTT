import pino, { type Logger } from "pino";
export class LoggerInstance {
  constructor(public source = "engine") {}
  public create(): Logger {
    return pino({ browser: { asObject: true } }).child({ source: this.source });
  }
}

export const EngineLogger = new LoggerInstance().create();
export const LoaderLogger = new LoggerInstance("loader").create();
export const FrontendLogger = new LoggerInstance("frontend").create();
