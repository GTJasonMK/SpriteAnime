import { createWorkflowActionChecker } from "../workflow-permissions";

export type SpriteWorkflowState =
  | "empty"
  | "loadingPreview"
  | "editingGrid"
  | "detectingBounds"
  | "splitting"
  | "previewingFrames";

export type SpriteAction =
  | "pickImage"
  | "selectImage"
  | "previewGrid"
  | "editGrid"
  | "editRegion"
  | "detectBounds"
  | "splitFrames"
  | "returnToGrid"
  | "editBoundary"
  | "playFrames"
  | "exportFrames";

export type SpritePermissions = Record<SpriteAction, boolean>;

export interface SpriteWorkflowContext {
  hasImage: boolean;
  hasFrames: boolean;
}

const SPRITE_ALLOWED_ACTIONS: Record<SpriteWorkflowState, readonly SpriteAction[]> = {
  empty: ["pickImage", "selectImage", "previewGrid"],
  loadingPreview: [],
  editingGrid: [
    "pickImage",
    "selectImage",
    "previewGrid",
    "editGrid",
    "editRegion",
    "detectBounds",
    "splitFrames",
    "editBoundary",
  ],
  detectingBounds: [],
  splitting: [],
  previewingFrames: ["returnToGrid", "playFrames", "exportFrames"],
};

export function getSpriteWorkflowPermissions(
  state: SpriteWorkflowState,
  context: SpriteWorkflowContext
): SpritePermissions {
  const can = createWorkflowActionChecker(SPRITE_ALLOWED_ACTIONS, state);
  return {
    pickImage: can("pickImage"),
    selectImage: can("selectImage"),
    previewGrid: can("previewGrid"),
    editGrid: can("editGrid") && context.hasImage,
    editRegion: can("editRegion") && context.hasImage,
    detectBounds: can("detectBounds") && context.hasImage,
    splitFrames: can("splitFrames") && context.hasImage,
    returnToGrid: can("returnToGrid") && context.hasFrames,
    editBoundary: can("editBoundary") && context.hasImage,
    playFrames: can("playFrames") && context.hasFrames,
    exportFrames: can("exportFrames") && context.hasFrames,
  };
}

export function deriveSpriteBaseState(context: SpriteWorkflowContext): SpriteWorkflowState {
  if (context.hasFrames) {
    return "previewingFrames";
  }
  if (context.hasImage) {
    return "editingGrid";
  }
  return "empty";
}
