export type EngineMountOptions = {
  canvasSelector: string;
  worldId: string;
};

export type EngineState = {
  started: boolean;
  canvasSelector: string | null;
  worldId: string | null;
};
