import type { SpriteElements } from "./dom";
import {
  getSpriteWorkflowPermissions,
  type SpriteWorkflowState,
} from "./workflow-state";

export function syncSplitModeControls(
  els: SpriteElements,
  state: SpriteWorkflowState,
  context: { hasImage: boolean; hasFrames: boolean }
): void {
  const permissions = getSpriteWorkflowPermissions(state, context);
  const sourceControls: Array<HTMLInputElement | HTMLSelectElement | HTMLButtonElement> = [
    els.imageSelect,
    els.pickImage,
    els.previewGrid,
  ];
  const gridControls: Array<HTMLInputElement | HTMLSelectElement | HTMLButtonElement> = [
    els.rows,
    els.cols,
    ...els.gridPresets,
  ];
  const editRegionControls: Array<HTMLInputElement | HTMLSelectElement | HTMLButtonElement> = [
    els.regionX,
    els.regionY,
    els.regionW,
    els.regionH,
    els.regionFull,
    els.resetGridLines,
    els.autoTrim,
    els.autoExpand,
    els.autoBgMode,
    els.autoTrimMode,
    els.autoThreshold,
    els.detectBounds,
  ];
  const boundaryControls: Array<HTMLInputElement | HTMLSelectElement | HTMLButtonElement> = [
    els.boundaryAdd,
    els.boundaryEdit,
    els.boundaryDelete,
    els.boundaryLeft,
    els.boundaryTop,
    els.boundaryRight,
    els.boundaryBottom,
    els.boundaryAnchorX,
    els.boundaryAnchorCenter,
    els.boundaryApply,
    els.boundaryCancel,
    els.boundaryClose,
    els.boundaryEditorDelete,
  ];

  sourceControls.forEach((control) => {
    control.disabled = control === els.previewGrid
      ? !permissions.previewGrid
      : !(control === els.pickImage ? permissions.pickImage : permissions.selectImage);
  });
  gridControls.forEach((control) => {
    control.disabled = !permissions.editGrid;
  });
  editRegionControls.forEach((control) => {
    control.disabled = !permissions.editRegion;
  });
  boundaryControls.forEach((control) => {
    if (!permissions.editBoundary) {
      control.disabled = true;
    }
  });
  if (!permissions.detectBounds) {
    els.detectBounds.disabled = true;
  }
  els.loadSplit.disabled = !(permissions.splitFrames || permissions.returnToGrid);
  els.loadSplit.textContent = state === "previewingFrames"
    ? "返回调整"
    : state === "splitting"
      ? "拆分中"
      : "拆分帧";
  els.loadSplit.title = state === "previewingFrames"
    ? "清空当前帧列表并回到网格与边界调整"
    : "按当前网格和边界拆分序列帧";
  [
    els.prevFrame,
    els.playPause,
    els.nextFrame,
    els.fpsSlider,
    els.scaleSlider,
    els.selectAll,
    els.invert,
    els.clear,
  ].forEach((control) => {
    control.disabled = !permissions.playFrames;
  });
  els.export.disabled = !permissions.exportFrames;
  if (state !== "editingGrid") {
    els.canvas.style.cursor = "default";
  }
}
