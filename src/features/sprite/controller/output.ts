import { handleSpriteExport } from "../export-actions";
import { updateGridPresetState as updateGridPresetStateUi } from "../grid-controls";
import {
  normalizeGridSize
} from "../utils";

import type { SpritePage } from "../sprite-page";

export const spritePageOutputMethods = {
  setGridSize(rows: number, cols: number, render: boolean = true): void {
    const previousRows = normalizeGridSize(this.els.rows.value, 3);
    const previousCols = normalizeGridSize(this.els.cols.value, 4);
    const safeRows = normalizeGridSize(String(rows), 3);
    const safeCols = normalizeGridSize(String(cols), 4);
    this.els.rows.value = String(safeRows);
    this.els.cols.value = String(safeCols);
    if (previousRows !== safeRows || previousCols !== safeCols) {
      this.gridLines = null;
      this.hoveredGridLine = null;
      this.autoBounds = null;
      this.clearSelectedBounds();
    }
    updateGridPresetStateUi(this.els, safeRows, safeCols);
    if (render) {
      this.renderGridPreviewFromCurrentImage();
    }
  },

  resetFrames(message: string): void {
    this.frameController.stopPlayback();
    this.sheetImage = null;
    this.sheetImagePath = "";
    this.splitRegion = null;
    this.regionDrag = null;
    this.gridLines = null;
    this.gridLineDrag = null;
    this.hoveredGridLine = null;
    this.autoBounds = null;
    this.clearSelectedBounds();
    this.clearSplitResult(message);
    this.frameController.destroyLoadedFrames();
    this.frameController.clearCanvas();
    this.syncRegionInputs();
    this.els.canvas.style.cursor = "default";
    this.els.placeholder.textContent = message;
    this.els.placeholder.style.display = "block";
    this.els.canvas.closest(".preview-area")?.classList.remove("has-image");
    this.els.frameSizeInfo.textContent = "尺寸: -";
    this.setWorkflowState("empty");
  },

  clearSplitResult(message: string): void {
    this.frameController.clearFrameList(message);
    this.syncBoundaryControls();
  },

  renderFallbackPreview(): boolean {
    if (!this.sheetImage) {
      return false;
    }
    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    this.drawGridPreview(rows, cols);
    return true;
  },

  // ==================== 导出 ====================

  async handleExport(): Promise<void> {
    if (!this.canRunSpriteAction("exportFrames")) return;
    await handleSpriteExport({
      frames: this.frameController.getFrames(),
      selectedIndices: this.frameController.getSelectedIndices(),
      sheetImagePath: this.sheetImagePath,
      fps: this.frameController.getFps(),
    });
  },
} satisfies ThisType<SpritePage>;

export type SpritePageOutputMethods = typeof spritePageOutputMethods;
