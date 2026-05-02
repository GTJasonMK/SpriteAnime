export type GeneratorWorkflowState =
  | "empty"
  | "ready"
  | "generating"
  | "optimizing"
  | "matting"
  | "mattingProcessing";

export type GeneratorAction =
  | "editGenerationParams"
  | "optimizePrompt"
  | "generate"
  | "addRecord"
  | "selectRecord"
  | "openSelected"
  | "revealSelected"
  | "enterMatting"
  | "exitMatting"
  | "runAutoMatting"
  | "eraseMatting"
  | "undoMatting"
  | "redoMatting"
  | "saveMatting"
  | "sendToSprite"
  | "deleteRecord"
  | "clearRecords"
  | "openSettings";

export type GeneratorPermissions = Record<GeneratorAction, boolean>;

export interface GeneratorWorkflowContext {
  hasRecords: boolean;
  hasSelection: boolean;
  hasMattingCanvas: boolean;
  mattingDirty: boolean;
  hasMattingUndo: boolean;
  hasMattingRedo: boolean;
}

const GENERATOR_ALLOWED_ACTIONS: Record<GeneratorWorkflowState, readonly GeneratorAction[]> = {
  empty: [
    "editGenerationParams",
    "optimizePrompt",
    "generate",
    "addRecord",
    "openSettings",
  ],
  ready: [
    "editGenerationParams",
    "optimizePrompt",
    "generate",
    "addRecord",
    "selectRecord",
    "openSelected",
    "revealSelected",
    "enterMatting",
    "sendToSprite",
    "deleteRecord",
    "clearRecords",
    "openSettings",
  ],
  generating: [],
  optimizing: [],
  matting: [
    "selectRecord",
    "openSelected",
    "revealSelected",
    "exitMatting",
    "runAutoMatting",
    "eraseMatting",
    "undoMatting",
    "redoMatting",
    "saveMatting",
  ],
  mattingProcessing: [],
};

export function getGeneratorWorkflowPermissions(
  state: GeneratorWorkflowState,
  context: GeneratorWorkflowContext
): GeneratorPermissions {
  const allowed = new Set<GeneratorAction>(GENERATOR_ALLOWED_ACTIONS[state]);
  const can = (action: GeneratorAction): boolean => allowed.has(action);
  return {
    editGenerationParams: can("editGenerationParams"),
    optimizePrompt: can("optimizePrompt"),
    generate: can("generate"),
    addRecord: can("addRecord"),
    selectRecord: can("selectRecord"),
    openSelected: can("openSelected") && context.hasSelection,
    revealSelected: can("revealSelected") && context.hasSelection,
    enterMatting: can("enterMatting") && context.hasSelection,
    exitMatting: can("exitMatting"),
    runAutoMatting: can("runAutoMatting") && context.hasSelection && context.hasMattingCanvas,
    eraseMatting: can("eraseMatting") && context.hasSelection && context.hasMattingCanvas,
    undoMatting: can("undoMatting") && context.hasMattingUndo,
    redoMatting: can("redoMatting") && context.hasMattingRedo,
    saveMatting: can("saveMatting") && context.mattingDirty && context.hasMattingCanvas,
    sendToSprite: can("sendToSprite") && context.hasSelection,
    deleteRecord: can("deleteRecord") && context.hasSelection,
    clearRecords: can("clearRecords") && context.hasRecords,
    openSettings: can("openSettings"),
  };
}

export function deriveGeneratorBaseState(context: Pick<GeneratorWorkflowContext, "hasSelection">): GeneratorWorkflowState {
  return context.hasSelection ? "ready" : "empty";
}
