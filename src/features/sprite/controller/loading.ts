import {
  extractSpriteFrames,
  type FrameCrop,
  type FrameData
} from "../../../api/commands";
import { getErrorMessage } from "../../../utils/errors";
import { loadHtmlImageFromPath } from "../../../utils/image";
import {
  detectFrameBoundsForImageAsync,
} from "../auto-trim";
import {
  createGridCellRects,
  getGridLinesSignature
} from "../grid-lines";
import { drawGridPreviewScene } from "../preview-renderer";
import {
  clampSplitRegion,
  isFullRegion
} from "../region-model";
import type {
  AutoBoundsResult,
  SplitRegion
} from "../types";
import {
  clampNumber,
  normalizeGridSize,
  normalizeThreshold,
  sameRegion,
  summarizeFrameSizes
} from "../utils";

import type { SpritePage } from "../sprite-page";

export const spritePageLoadingMethods = {
  async handleLoadSplit(): Promise<void> {
    if (!this.canRunSpriteAction("splitFrames")) return;

    const imagePath = this.els.imageSelect.value;
    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    this.setGridSize(rows, cols, false);

    if (!imagePath) {
      await this.handlePreviewGrid();
      return;
    }

    if (this.sheetImagePath !== imagePath || !this.sheetImage) {
      const loaded = await this.loadGridPreview(imagePath);
      if (!loaded) {
        return;
      }
    }

    const region = this.getValidatedRegion(rows, cols);
    const detectedBounds = this.hasDetectedAutoBounds(rows, cols, region)
      ? this.autoBounds
      : null;
    if (detectedBounds) {
      await this.loadAutoSplit(detectedBounds, rows, cols);
      return;
    }

    const sourceLabel = this.sheetImage &&
      isFullRegion(region, this.sheetImage.naturalWidth, this.sheetImage.naturalHeight)
      ? "原图"
      : "区域";
    await this.loadAndSplit(imagePath, rows, cols, region, sourceLabel);
  },

  async loadGridPreview(imagePath: string): Promise<boolean> {
    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    this.setGridSize(rows, cols, false);
    const imageChanged = this.sheetImagePath !== imagePath;
    this.setWorkflowState("loadingPreview");

    try {
      this.frameController.stopPlayback();
      this.frameController.destroyLoadedFrames();
      this.clearSplitResult("确认网格后点击拆分帧");
      this.els.placeholder.textContent = "正在加载网格...";
      this.els.placeholder.style.display = "block";

      this.sheetImage = await loadHtmlImageFromPath(imagePath);
      this.sheetImagePath = imagePath;
      this.autoBounds = null;
      this.gridLineDrag = null;
      this.hoveredGridLine = null;
      if (imageChanged || !this.splitRegion) {
        this.setFullRegion(false, true);
      } else {
        this.splitRegion = clampSplitRegion(
          this.splitRegion,
          rows,
          cols,
          this.sheetImage.naturalWidth,
          this.sheetImage.naturalHeight
        );
        this.clearSelectedBounds();
        this.syncRegionInputs();
      }
      this.drawGridPreview(rows, cols);
      this.settleWorkflowState();
      this.syncRegionInputs();
      return true;
    } catch (err) {
      this.sheetImage = null;
      this.sheetImagePath = "";
      this.splitRegion = null;
      this.autoBounds = null;
      this.clearSelectedBounds();
      this.syncRegionInputs();
      this.resetFrames("网格预览失败");
      this.setWorkflowState("empty");
      console.error("[sprite] 网格预览失败:", err);
      alert(`网格预览失败: ${getErrorMessage(err)}`);
      return false;
    }
  },

  renderGridPreviewFromCurrentImage(): void {
    if (!this.sheetImage) return;
    this.frameController.stopPlayback();
    if (this.frameController.hasFrames()) {
      this.frameController.destroyLoadedFrames();
      this.clearSplitResult("网格已更新，点击拆分帧重新生成");
    }
    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    this.setGridSize(rows, cols, false);
    this.splitRegion = this.getValidatedRegion(rows, cols);
    this.syncRegionInputs();
    this.drawGridPreview(rows, cols);
  },

  drawGridPreview(rows: number, cols: number): void {
    if (!this.sheetImage) return;

    const region = this.getValidatedRegion(rows, cols);
    const gridLines = this.getActiveGridLines(rows, cols, region);
    const info = drawGridPreviewScene({
      canvas: this.els.canvas,
      placeholder: this.els.placeholder,
      sheetImage: this.sheetImage,
      rows,
      cols,
      region,
      gridLines,
      highlightedGridLine: this.gridLineDrag ?? this.hoveredGridLine,
      autoBounds: this.autoBounds,
      autoBoundsValid: this.isAutoBoundsValid(rows, cols, region),
      selectedBoundsIndex: this.selectedBoundsIndex,
      autoTrimMode: this.els.autoTrimMode.value,
    });
    if (info) {
      this.els.frameSizeInfo.textContent = info;
    }
  },

  async loadAndSplit(
    imagePath: string,
    rows: number,
    cols: number,
    region: SplitRegion,
    sourceLabel: string
  ): Promise<void> {
    this.frameController.stopPlayback();
    this.setWorkflowState("splitting");
    this.clearSplitResult("正在拆分...");
    this.frameController.destroyLoadedFrames();
    this.frameController.clearCanvas();

    try {
      const result = await extractSpriteFrames(
        imagePath,
        this.createGridCrops(region, rows, cols)
      );

      await this.applySplitFrames(
        result.frames,
        rows,
        cols,
        `${sourceLabel}: ${result.original_size.width}x${result.original_size.height}`
      );
    } catch (err) {
      this.clearSplitResult("拆分失败，请检查网格");
      if (this.sheetImage) {
        const rows = normalizeGridSize(this.els.rows.value, 3);
        const cols = normalizeGridSize(this.els.cols.value, 4);
        this.drawGridPreview(rows, cols);
      } else {
        this.resetFrames("加载失败");
      }
      this.settleWorkflowState();
      console.error("[sprite] 加载分割失败:", err);
      alert(`加载分割失败: ${getErrorMessage(err)}`);
    }
  },

  createGridCrops(region: SplitRegion, rows: number, cols: number): FrameCrop[] {
    const cellRects = this.getGridCellRects(rows, cols, region);
    if (cellRects.some((cell) => cell.width < 1 || cell.height < 1)) {
      throw new Error("网格太密，单帧尺寸无效");
    }
    return cellRects.map((cell, index) => ({
      index,
      x: cell.x,
      y: cell.y,
      width: cell.width,
      height: cell.height,
      anchorX: cell.width / 2,
    }));
  },

  createAutoCrops(bounds: AutoBoundsResult, mode: "tight" | "fixed"): FrameCrop[] {
    return bounds.frameBounds.map((frame) => {
      const crop = mode === "fixed"
        ? {
          x: frame.cellX + bounds.fixedOffsetX,
          y: frame.cellY + bounds.fixedOffsetY,
          width: bounds.fixedWidth,
          height: bounds.fixedHeight,
        }
        : {
          x: frame.x,
          y: frame.y,
          width: frame.width,
          height: frame.height,
        };
      return {
        index: frame.index,
        ...crop,
        anchorX: clampNumber(frame.anchorX - crop.x, 0, crop.width),
      };
    });
  },

  async loadAutoSplit(
    bounds: AutoBoundsResult,
    rows: number,
    cols: number
  ): Promise<void> {
    this.frameController.stopPlayback();
    this.setWorkflowState("splitting");
    this.clearSplitResult("正在按自动边界拆分...");
    this.frameController.destroyLoadedFrames();
    this.frameController.clearCanvas();

    try {
      const mode = this.els.autoTrimMode.value === "fixed" ? "fixed" : "tight";
      if (!this.sheetImage || !this.sheetImagePath) {
        throw new Error("没有可拆分的图片");
      }
      const crops = this.createAutoCrops(bounds, mode);
      const result = await extractSpriteFrames(this.sheetImagePath, crops);
      const sourceInfo = mode === "fixed"
        ? `自动统一: ${bounds.fixedWidth}x${bounds.fixedHeight}`
        : `自动逐帧: ${summarizeFrameSizes(result.frames)}`;
      await this.applySplitFrames(
        result.frames,
        rows,
        cols,
        sourceInfo
      );
      const detected = bounds.frameBounds.length - bounds.emptyCount;
      this.els.frameSizeInfo.textContent += ` | 已检测: ${detected}/${bounds.frameBounds.length}`;
    } catch (err) {
      this.clearSplitResult("自动拆分失败，请调整阈值");
      this.renderGridPreviewFromCurrentImage();
      this.settleWorkflowState();
      console.error("[sprite] 自动拆分失败:", err);
      alert(`自动拆分失败: ${getErrorMessage(err)}`);
    }
  },

  async applySplitFrames(
    frames: FrameData[],
    rows: number,
    cols: number,
    sourceInfo: string
  ): Promise<void> {
    const applied = await this.frameController.applyFrames(frames, rows, cols, sourceInfo);
    if (!applied) {
      this.resetFrames("分割失败");
      alert("分割失败：帧尺寸无效，请检查行列设置");
      return;
    }
    this.clearSelectedBounds(false);
    this.setWorkflowState("previewingFrames");
  },

  async detectAutoBounds(): Promise<AutoBoundsResult | null> {
    if (!this.canRunSpriteAction("detectBounds")) return null;
    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    const bounds = await this.getAutoBounds(rows, cols);
    if (bounds) {
      this.syncBoundaryControls();
      this.renderGridPreviewFromCurrentImage();
    }
    return bounds;
  },

  hasDetectedAutoBounds(rows: number, cols: number, region: SplitRegion): boolean {
    return this.isAutoBoundsValid(rows, cols, region);
  },

  syncAutoBoundaryOptions(): void {
    const autoEnabled = this.els.autoTrim.checked;
    this.els.autoExpandToggle.style.display = autoEnabled ? "inline-flex" : "none";
    if (!autoEnabled) {
      this.els.autoExpand.checked = false;
    }
  },

  async getAutoBounds(rows: number, cols: number): Promise<AutoBoundsResult | null> {
    if (!this.sheetImage) {
      alert("请先预览网格");
      return null;
    }

    const region = this.getValidatedRegion(rows, cols);
    if (this.autoBounds && this.isAutoBoundsValid(rows, cols, region)) {
      return this.autoBounds;
    }

    try {
      this.setWorkflowState("detectingBounds");
      const threshold = normalizeThreshold(this.els.autoThreshold.value);
      this.els.autoThreshold.value = String(threshold);
      this.els.frameSizeInfo.textContent = "正在检测边界...";
      const gridLines = this.getActiveGridLines(rows, cols, region);
      const cellRects = createGridCellRects(rows, cols, region, gridLines);
      this.autoBounds = await detectFrameBoundsForImageAsync({
        imagePath: this.sheetImagePath,
        rows,
        cols,
        region,
        cellRects,
        gridSignature: getGridLinesSignature(rows, cols, region, gridLines),
        bgMode: this.els.autoBgMode.value,
        threshold,
        allowExpand: this.els.autoExpand.checked,
      });
      return this.autoBounds;
    } catch (err) {
      this.autoBounds = null;
      console.error("[sprite] 自动边界检测失败:", err);
      alert(`自动边界检测失败: ${getErrorMessage(err)}`);
      return null;
    } finally {
      this.settleWorkflowState();
    }
  },

  isAutoBoundsValid(
    rows: number,
    cols: number,
    region: SplitRegion
  ): boolean {
    return Boolean(
      this.autoBounds &&
      this.els.autoTrim.checked &&
      this.autoBounds.rows === rows &&
      this.autoBounds.cols === cols &&
      this.autoBounds.allowExpand === this.els.autoExpand.checked &&
      this.autoBounds.gridSignature === this.getCurrentGridSignature(rows, cols, region) &&
      sameRegion(this.autoBounds.region, region)
    );
  },


} satisfies ThisType<SpritePage>;

export type SpritePageLoadingMethods = typeof spritePageLoadingMethods;
