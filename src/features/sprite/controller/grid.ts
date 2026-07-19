import {
  readBoundaryEditorValues,
  syncBoundaryControls as syncBoundaryControlsUi,
  updateBoundaryAnchorInputConstraints
} from "../boundary-editor-ui";
import {
  recalculateAutoBounds,
  resetFrameRegion
} from "../boundary-model";
import {
  createEvenGridLines,
  createGridCellRects,
  getGridLinesSignature,
  normalizeGridLines
} from "../grid-lines";
import { syncSplitModeControls as syncSplitModeControlsUi } from "../split-mode-controls";
import type {
  GridLines,
  SplitRegion
} from "../types";
import {
  normalizeGridSize
} from "../utils";

import type { SpritePage } from "../sprite-page";

export const spritePageGridMethods = {
  applyBoundaryEditorValues(): boolean {
    const bounds = this.autoBounds;
    const index = this.selectedBoundsIndex;
    if (!bounds || index === null || index < 0 || index >= bounds.frameBounds.length) {
      return false;
    }

    const frame = bounds.frameBounds[index];
    if (!this.boundaryEditOriginal || this.boundaryEditOriginal.index !== index) {
      this.boundaryEditOriginal = {
        index,
        frame: { ...frame },
      };
    }
    const edited = readBoundaryEditorValues(
      this.els,
      this.getBoundaryLimitRegion()
    );
    if (!edited) {
      return false;
    }

    frame.x = edited.region.x;
    frame.y = edited.region.y;
    frame.width = edited.region.width;
    frame.height = edited.region.height;
    frame.anchorX = edited.anchorX;
    frame.empty = false;
    updateBoundaryAnchorInputConstraints(this.els, frame);
    recalculateAutoBounds(bounds);
    this.afterBoundaryChanged();
    return true;
  },

  getBoundaryLimitRegion(): SplitRegion {
    if (!this.sheetImage) {
      return { x: 0, y: 0, width: 1, height: 1 };
    }
    return {
      x: 0,
      y: 0,
      width: this.sheetImage.naturalWidth,
      height: this.sheetImage.naturalHeight,
    };
  },

  clearSelectedBounds(revertEditor: boolean = true): void {
    if (revertEditor) {
      this.closeBoundaryEditor(true);
    } else {
      this.closeBoundaryEditor(false);
    }
    this.selectedBoundsIndex = null;
    this.syncBoundaryControls();
  },

  syncSplitModeControls(): void {
    if (!this.els) return;
    syncSplitModeControlsUi(this.els, this.workflowState, this.getSpriteWorkflowContext());
  },

  syncBoundaryControls(): void {
    if (!this.els) return;

    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    const region = this.sheetImage ? this.getValidatedRegion(rows, cols) : null;
    const hasFrames = this.frameController.hasFrames();
    const hasImage = Boolean(this.sheetImage && region);
    const shouldShowEditor = hasImage && !hasFrames;
    const boundsValid = Boolean(
      region && this.autoBounds && this.isAutoBoundsValid(rows, cols, region)
    );
    const index = this.selectedBoundsIndex;
    const hasSelection = Boolean(
      boundsValid &&
      index !== null &&
      index >= 0 &&
      this.autoBounds &&
      index < this.autoBounds.frameBounds.length
    );
    const selectedFrame = hasSelection && this.autoBounds && index !== null
      ? this.autoBounds.frameBounds[index]
      : null;
    syncBoundaryControlsUi(this.els, {
      hasFrames,
      hasImage,
      shouldShowEditor,
      hasSelection,
      selectedFrame,
    });
    this.syncSplitModeControls();
  },

  createBoundaryFromCell(index: number): void {
    const bounds = this.autoBounds;
    if (!bounds || index < 0 || index >= bounds.frameBounds.length) {
      return;
    }

    const frame = bounds.frameBounds[index];
    resetFrameRegion(frame, false);
    recalculateAutoBounds(bounds);
  },

  afterBoundaryChanged(): void {
    this.els.autoTrim.checked = true;
    this.syncAutoBoundaryOptions();
    this.syncBoundaryControls();
    if (this.frameController.hasFrames()) {
      this.frameController.stopPlayback();
      this.frameController.destroyLoadedFrames();
      this.clearSplitResult("边界已更新，点击拆分帧重新生成");
    }
    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    this.drawGridPreview(rows, cols);
  },

  getCanvasNaturalTolerance(): number {
    if (!this.sheetImage) return 4;
    const rect = this.els.canvas.getBoundingClientRect();
    if (rect.width <= 0) return 4;
    const naturalPerCssPixel = this.sheetImage.naturalWidth / rect.width;
    return Math.max(2, naturalPerCssPixel * 6);
  },

  getCanvasDisplayScale(): number {
    if (!this.sheetImage) return 1;
    const rect = this.els.canvas.getBoundingClientRect();
    return rect.width / Math.max(1, this.sheetImage.naturalWidth);
  },

  getActiveGridLines(rows: number, cols: number, region: SplitRegion): GridLines {
    this.gridLines = normalizeGridLines(this.gridLines, rows, cols, region);
    return this.gridLines;
  },

  getGridCellRects(rows: number, cols: number, region: SplitRegion): SplitRegion[] {
    return createGridCellRects(rows, cols, region, this.getActiveGridLines(rows, cols, region));
  },

  getCurrentGridSignature(rows: number, cols: number, region: SplitRegion): string {
    return getGridLinesSignature(rows, cols, region, this.getActiveGridLines(rows, cols, region));
  },

  resetGridLines(render: boolean = true): void {
    if (!this.canRunSpriteAction("editGrid")) return;
    if (!this.sheetImage) {
      this.gridLines = null;
      this.hoveredGridLine = null;
      return;
    }

    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    const region = this.getValidatedRegion(rows, cols);
    this.gridLines = createEvenGridLines(rows, cols, region);
    this.gridLineDrag = null;
    this.hoveredGridLine = null;
    this.invalidateAutoBounds(false);
    this.syncRegionInputs();
    if (render) {
      this.drawGridPreview(rows, cols);
    }
  },

  releaseCanvasPointer(pointerId: number): void {
    if (this.els.canvas.hasPointerCapture(pointerId)) {
      this.els.canvas.releasePointerCapture(pointerId);
    }
  },


} satisfies ThisType<SpritePage>;

export type SpritePageGridMethods = typeof spritePageGridMethods;
