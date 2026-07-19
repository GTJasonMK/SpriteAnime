import { createWorkflowActionChecker } from "../../workflows/permissions";

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
  | "deleteRecord"
  | "clearRecords";

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
    "deleteRecord",
    "clearRecords",
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
  const can = createWorkflowActionChecker(GENERATOR_ALLOWED_ACTIONS, state);
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
    deleteRecord: can("deleteRecord") && context.hasSelection,
    clearRecords: can("clearRecords") && context.hasRecords,
  };
}

export function deriveGeneratorBaseState(context: Pick<GeneratorWorkflowContext, "hasSelection">): GeneratorWorkflowState {
  return context.hasSelection ? "ready" : "empty";
}
