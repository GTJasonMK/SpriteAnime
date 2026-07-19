import {
  setBoundaryEditorInputs
} from "../boundary-editor-ui";
import {
  createDefaultAutoBounds,
  defaultFrameRegion,
  recalculateAutoBounds,
  resetFrameRegion,
} from "../boundary-model";
import {
  createGridCellRects,
  getGridLinesSignature
} from "../grid-lines";
import {
  findBoundaryAtPoint,
  getFrameIndexAtPoint
} from "../region-model";
import type {
  AutoBoundsResult,
  SplitRegion
} from "../types";
import {
  normalizeGridSize
} from "../utils";

import type { SpritePage } from "../sprite-page";

export const spritePageBoundaryMethods = {
  handleBoundaryPointerDown(
    x: number,
    y: number,
    rows: number,
    cols: number,
    region: SplitRegion
  ): boolean {
    if (!this.els.autoTrim.checked && !this.autoBounds) {
      return false;
    }

    const cellIndex = getFrameIndexAtPoint(
      x,
      y,
      rows,
      cols,
      region,
      this.getActiveGridLines(rows, cols, region)
    );
    if (cellIndex === null) {
      return false;
    }

    const bounds = this.ensureEditableBounds(rows, cols, region);
    if (!bounds) {
      return false;
    }

    const hitIndex = findBoundaryAtPoint(x, y, bounds, this.getCanvasNaturalTolerance());
    this.selectBoundary(hitIndex ?? cellIndex, true);
    return true;
  },

  handleAddBoundary(): void {
    if (!this.canRunSpriteAction("editBoundary")) return;
    if (!this.sheetImage) {
      alert("请先预览网格");
      return;
    }

    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    const region = this.getValidatedRegion(rows, cols);
    const bounds = this.ensureEditableBounds(rows, cols, region);
    if (!bounds) return;

    const firstEmpty = bounds.frameBounds.find((frame) => frame.empty)?.index;
    const index = this.selectedBoundsIndex ?? firstEmpty ?? 0;
    this.boundaryEditOriginal = {
      index,
      frame: { ...bounds.frameBounds[index] },
    };
    this.createBoundaryFromCell(index);
    this.selectBoundary(index, true);
    this.afterBoundaryChanged();
  },

  handleEditBoundary(): void {
    if (!this.canRunSpriteAction("editBoundary")) return;
    if (this.selectedBoundsIndex === null) {
      alert("请先在画布中选择一个边界框");
      return;
    }
    this.openBoundaryEditor(this.selectedBoundsIndex);
  },

  handleApplyBoundary(): void {
    if (!this.canRunSpriteAction("editBoundary")) return;
    if (this.applyBoundaryEditorValues()) {
      this.boundaryEditOriginal = null;
      this.closeBoundaryEditor(false);
    }
  },

  handleLiveBoundaryInput(): void {
    if (!this.canRunSpriteAction("editBoundary")) return;
    this.applyBoundaryEditorValues();
  },

  handleCenterBoundaryAnchor(): void {
    if (!this.canRunSpriteAction("editBoundary")) return;
    const bounds = this.autoBounds;
    const index = this.selectedBoundsIndex;
    if (!bounds || index === null || index < 0 || index >= bounds.frameBounds.length) {
      return;
    }

    const frame = bounds.frameBounds[index];
    const editable = frame.empty ? defaultFrameRegion(frame) : frame;
    this.els.boundaryAnchorX.value = String(Math.round(editable.width / 2));
    this.applyBoundaryEditorValues();
  },

  handleDeleteBoundary(): void {
    if (!this.canRunSpriteAction("editBoundary")) return;
    const bounds = this.autoBounds;
    const index = this.selectedBoundsIndex;
    if (!bounds || index === null || index < 0 || index >= bounds.frameBounds.length) {
      alert("请先选择一个边界框");
      return;
    }

    const frame = bounds.frameBounds[index];
    resetFrameRegion(frame, true);
    recalculateAutoBounds(bounds);
    this.boundaryEditOriginal = null;
    this.closeBoundaryEditor(false);
    this.afterBoundaryChanged();
  },

  ensureEditableBounds(
    rows: number,
    cols: number,
    region: SplitRegion
  ): AutoBoundsResult | null {
    if (this.autoBounds && this.isAutoBoundsValid(rows, cols, region)) {
      return this.autoBounds;
    }

    const gridLines = this.getActiveGridLines(rows, cols, region);
    const cellRects = createGridCellRects(rows, cols, region, gridLines);
    this.autoBounds = createDefaultAutoBounds(
      rows,
      cols,
      region,
      this.els.autoExpand.checked,
      cellRects,
      getGridLinesSignature(rows, cols, region, gridLines)
    );
    this.els.autoTrim.checked = true;
    this.syncAutoBoundaryOptions();
    this.syncBoundaryControls();
    return this.autoBounds;
  },

  selectBoundary(index: number, openEditor: boolean): void {
    if (!this.autoBounds || index < 0 || index >= this.autoBounds.frameBounds.length) {
      return;
    }
    this.selectedBoundsIndex = index;
    this.syncBoundaryControls();
    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    this.drawGridPreview(rows, cols);
    if (openEditor) {
      this.openBoundaryEditor(index);
    }
  },

  switchSelectedBoundary(delta: number): boolean {
    const bounds = this.autoBounds;
    const current = this.selectedBoundsIndex;
    if (
      this.frameController.hasFrames() ||
      !bounds ||
      current === null ||
      bounds.frameBounds.length <= 1
    ) {
      return false;
    }

    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    const region = this.sheetImage ? this.getValidatedRegion(rows, cols) : null;
    if (!region || !this.isAutoBoundsValid(rows, cols, region)) {
      return false;
    }

    const count = bounds.frameBounds.length;
    const next = (current + delta + count) % count;
    this.boundaryEditOriginal = null;
    this.selectBoundary(next, true);
    return true;
  },

  openBoundaryEditor(index: number): void {
    const bounds = this.autoBounds;
    if (!bounds || index < 0 || index >= bounds.frameBounds.length) {
      return;
    }

    const frame = bounds.frameBounds[index];
    if (this.boundaryEditOriginal?.index !== index) {
      this.boundaryEditOriginal = {
        index,
        frame: { ...frame },
      };
    }

    this.selectedBoundsIndex = index;
    this.els.boundaryTitle.textContent = `边界框 ${index + 1}`;
    setBoundaryEditorInputs(this.els, frame, this.getBoundaryLimitRegion());

    this.syncBoundaryControls();
    this.els.boundaryPanel.classList.add("active");
  },

  closeBoundaryEditor(revert: boolean): void {
    if (revert) {
      this.restoreBoundaryEditOriginal();
    }
    this.boundaryEditOriginal = null;
    this.syncBoundaryControls();
  },

  restoreBoundaryEditOriginal(): void {
    const snapshot = this.boundaryEditOriginal;
    const bounds = this.autoBounds;
    if (!snapshot || !bounds || snapshot.index < 0 || snapshot.index >= bounds.frameBounds.length) {
      return;
    }

    bounds.frameBounds[snapshot.index] = { ...snapshot.frame };
    recalculateAutoBounds(bounds);
    this.afterBoundaryChanged();
    setBoundaryEditorInputs(
      this.els,
      bounds.frameBounds[snapshot.index],
      this.getBoundaryLimitRegion()
    );
  },


} satisfies ThisType<SpritePage>;

export type SpritePageBoundaryMethods = typeof spritePageBoundaryMethods;
