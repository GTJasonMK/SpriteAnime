import {
  openImageFile,
  importImageToLibrary,
  extractSpriteFrames,
  type SplitResult,
  type FrameData,
  type FrameCrop,
} from "../api/commands";
import { loadHtmlImageFromPath } from "../utils/image";
import { CanvasPlayer } from "../widgets/canvas-player";
import type { GeneratorPage } from "./generator";
import {
  detectFrameBoundsForImageAsync,
} from "./sprite/auto-trim";
import {
  readBoundaryEditorValues,
  setBoundaryEditorInputs,
  syncBoundaryControls as syncBoundaryControlsUi,
  updateBoundaryAnchorInputConstraints,
} from "./sprite/boundary-editor-ui";
import {
  createDefaultAutoBounds,
  defaultFrameRegion,
  recalculateAutoBounds,
} from "./sprite/boundary-model";
import { cacheSpriteElements, type SpriteElements } from "./sprite/dom";
import { handleSpriteExport } from "./sprite/export-actions";
import { SpriteFrameController } from "./sprite/frame-controller";
import {
  createEvenGridLines,
  createGridCellRects,
  cursorForGridLine,
  getGridLinesSignature,
  hitTestGridLine,
  moveGridLine,
  normalizeGridLines,
  sameGridLineHit,
} from "./sprite/grid-lines";
import { updateGridPresetState as updateGridPresetStateUi } from "./sprite/grid-controls";
import {
  addImageSource as addImageSourceOption,
  syncGeneratedImageSources,
} from "./sprite/image-sources";
import { drawGridPreviewScene } from "./sprite/preview-renderer";
import {
  clampSplitRegion,
  findBoundaryAtPoint,
  getCanvasNaturalPoint,
  getFrameIndexAtPoint,
  hitTestRegion,
  isFullRegion,
  regionFromDrag,
} from "./sprite/region-model";
import type {
  AutoBoundsResult,
  BoundaryEditSnapshot,
  GridLineDragState,
  GridLineHit,
  GridLines,
  RegionDragState,
  SplitRegion,
} from "./sprite/types";
import {
  clampNumber,
  cursorForDragMode,
  getRegionCenterX,
  normalizeGridSize,
  normalizeThreshold,
  parseRegionNumber,
  sameRegion,
  summarizeFrameSizes,
} from "./sprite/utils";
import { syncSplitModeControls as syncSplitModeControlsUi } from "./sprite/split-mode-controls";
import {
  deriveSpriteBaseState,
  getSpriteWorkflowPermissions,
  type SpriteAction,
  type SpriteWorkflowState,
} from "./sprite/workflow-state";
import { getTabButton, onPrepareSpriteFromGenerator } from "./navigation";

/// 序列帧预览页面控制器
export class SpritePage {
  // 状态
  private sheetImage: HTMLImageElement | null = null;
  private sheetImagePath: string = "";
  private splitRegion: SplitRegion | null = null;
  private regionDrag: RegionDragState | null = null;
  private gridLines: GridLines | null = null;
  private gridLineDrag: GridLineDragState | null = null;
  private hoveredGridLine: GridLineHit | null = null;
  private autoBounds: AutoBoundsResult | null = null;
  private selectedBoundsIndex: number | null = null;
  private boundaryEditOriginal: BoundaryEditSnapshot | null = null;
  private workflowState: SpriteWorkflowState = "empty";

  // 子组件
  private canvasPlayer!: CanvasPlayer;
  private frameController!: SpriteFrameController;

  // 关联的生成页面引用
  private generatorPage!: GeneratorPage;

  // DOM元素缓存
  private els!: SpriteElements;

  constructor() {
    this.els = cacheSpriteElements();
    this.canvasPlayer = new CanvasPlayer(this.els.canvas);
    this.frameController = new SpriteFrameController(this.els, this.canvasPlayer, {
      renderFallbackPreview: () => this.renderFallbackPreview(),
    });
  }

  private getSpriteWorkflowContext() {
    return {
      hasImage: Boolean(this.sheetImage && this.splitRegion),
      hasFrames: this.frameController.hasFrames(),
    };
  }

  private canRunSpriteAction(action: SpriteAction): boolean {
    return getSpriteWorkflowPermissions(this.workflowState, this.getSpriteWorkflowContext())[action];
  }

  private setWorkflowState(nextState: SpriteWorkflowState): void {
    this.workflowState = nextState;
    this.syncSpriteWorkflowControls();
  }

  private settleWorkflowState(): void {
    this.setWorkflowState(deriveSpriteBaseState(this.getSpriteWorkflowContext()));
  }

  async init(generatorPage: GeneratorPage): Promise<void> {
    console.log("[sprite] 初始化...");
    this.generatorPage = generatorPage;
    this.bindEvents();
    this.updateGridPresetState();
    this.syncAutoBoundaryOptions();
    this.syncRegionInputs();
    this.syncBoundaryControls();
    this.syncSpriteWorkflowControls();
    console.log("[sprite] 初始化完成");
  }

  /// 添加图片到源选择列表
  addImageSource(path: string, select: boolean = true): boolean {
    return addImageSourceOption(this.els, path, select);
  }

  private bindEvents(): void {
    getTabButton("sprite")?.addEventListener("click", () => {
      this.syncGeneratedImages({ selectLatest: true });
    });
    onPrepareSpriteFromGenerator(() => {
      this.syncGeneratedImages({ selectLatest: true, applyPreferredGrid: true });
      this.handlePreviewGrid();
    });

    // 本地图片选择
    this.els.pickImage.addEventListener("click", () => this.handlePickImage());

    this.els.imageSelect.addEventListener("change", () => {
      this.handlePreviewGrid();
    });

    this.els.rows.addEventListener("input", () => {
      this.gridLines = null;
      this.hoveredGridLine = null;
      this.autoBounds = null;
      this.clearSelectedBounds();
      this.renderGridPreviewFromCurrentImage();
    });
    this.els.cols.addEventListener("input", () => {
      this.gridLines = null;
      this.hoveredGridLine = null;
      this.autoBounds = null;
      this.clearSelectedBounds();
      this.renderGridPreviewFromCurrentImage();
    });
    this.els.gridPresets.forEach((button) => {
      button.addEventListener("click", () => {
        this.setGridSize(
          Number.parseInt(button.dataset.rows || "3", 10),
          Number.parseInt(button.dataset.cols || "4", 10)
        );
      });
    });
    [
      this.els.regionX,
      this.els.regionY,
      this.els.regionW,
      this.els.regionH,
    ].forEach((input) => {
      input.addEventListener("input", () => this.handleRegionInput());
    });
    this.els.regionFull.addEventListener("click", () => this.setFullRegion());
    this.els.resetGridLines.addEventListener("click", () => this.resetGridLines());
    this.els.autoTrim.addEventListener("change", () => {
      this.autoBounds = null;
      this.clearSelectedBounds();
      this.syncAutoBoundaryOptions();
      this.renderGridPreviewFromCurrentImage();
    });
    this.els.autoBgMode.addEventListener("change", () => this.invalidateAutoBounds());
    this.els.autoExpand.addEventListener("change", () => this.invalidateAutoBounds());
    this.els.autoTrimMode.addEventListener("change", () => {
      this.syncBoundaryControls();
      this.renderGridPreviewFromCurrentImage();
    });
    this.els.autoThreshold.addEventListener("input", () => this.invalidateAutoBounds());
    this.els.detectBounds.addEventListener("click", () => {
      this.els.autoTrim.checked = true;
      this.syncAutoBoundaryOptions();
      this.clearSelectedBounds();
      void this.detectAutoBounds();
    });
    this.els.boundaryAdd.addEventListener("click", () => this.handleAddBoundary());
    this.els.boundaryEdit.addEventListener("click", () => this.handleEditBoundary());
    this.els.boundaryDelete.addEventListener("click", () => this.handleDeleteBoundary());
    this.els.boundaryApply.addEventListener("click", () => this.handleApplyBoundary());
    [
      this.els.boundaryLeft,
      this.els.boundaryTop,
      this.els.boundaryRight,
      this.els.boundaryBottom,
      this.els.boundaryAnchorX,
    ].forEach((input) => {
      input.addEventListener("input", () => this.handleLiveBoundaryInput());
    });
    this.els.boundaryAnchorCenter.addEventListener("click", () => this.handleCenterBoundaryAnchor());
    this.els.boundaryCancel.addEventListener("click", () => this.closeBoundaryEditor(true));
    this.els.boundaryClose.addEventListener("click", () => this.clearSelectedBounds(true));
    this.els.boundaryEditorDelete.addEventListener("click", () => this.handleDeleteBoundary());
    document.addEventListener("keydown", (event) => this.handleBoundaryKeyboard(event));
    this.els.canvas.addEventListener("pointerdown", (event) => this.handleRegionPointerDown(event));
    this.els.canvas.addEventListener("pointermove", (event) => this.handleRegionPointerMove(event));
    this.els.canvas.addEventListener("pointerup", (event) => this.handleRegionPointerUp(event));
    this.els.canvas.addEventListener("pointercancel", (event) => this.handleRegionPointerUp(event));

    // 先预览网格，再拆分
    this.els.previewGrid.addEventListener("click", () => this.handlePreviewGrid());
    this.els.loadSplit.addEventListener("click", () => this.handleLoadSplit());

    this.frameController.bindEvents();

    // 导出
    this.els.export.addEventListener("click", () => this.handleExport());
  }

  private handleBoundaryKeyboard(event: KeyboardEvent): void {
    if (!this.canRunSpriteAction("editBoundary")) {
      return;
    }
    if (this.selectedBoundsIndex === null) {
      return;
    }

    if (event.key === "Escape") {
      this.clearSelectedBounds(true);
      return;
    }

    if (
      event.key !== "ArrowLeft" &&
      event.key !== "ArrowRight"
    ) {
      return;
    }

    if (
      event.altKey ||
      event.ctrlKey ||
      event.metaKey
    ) {
      return;
    }

    const boundaryInput = getBoundaryEditorInputTarget(event.target);
    if (!boundaryInput && isEditableKeyboardTarget(event.target)) {
      return;
    }

    if (boundaryInput && !this.applyBoundaryEditorValues(false)) {
      return;
    }

    const delta = event.key === "ArrowLeft" ? -1 : 1;
    if (this.switchSelectedBoundary(delta)) {
      event.preventDefault();
      boundaryInput?.focus();
    }
  }

  /// 处理：加载图片并分割为帧
  private async handlePickImage(): Promise<void> {
    if (!this.canRunSpriteAction("pickImage")) return;
    try {
      const result = await openImageFile();
      const imported = await importImageToLibrary(result.file_path);
      this.addImageSource(imported.file_path);
      this.els.imageSelect.value = imported.file_path;
      await this.handlePreviewGrid();
    } catch (err) {
      if (String(err) !== "用户取消选择") {
        console.error("[sprite] 选择图片失败:", err);
        alert("选择图片失败: " + String(err));
      }
    }
  }

  private async handlePreviewGrid(): Promise<void> {
    if (!this.canRunSpriteAction("previewGrid")) {
      return;
    }

    this.syncGeneratedImages();

    let imagePath = this.els.imageSelect.value;
    if (!imagePath) {
      const generated = this.generatorPage.getLastGeneratedImages();
      if (generated.length > 0) {
        imagePath = generated[generated.length - 1];
        this.addImageSource(imagePath);
        this.applyPreferredGridFromGenerator(false);
      }
    }
    if (!imagePath) {
      try {
        const result = await openImageFile();
        const imported = await importImageToLibrary(result.file_path);
        this.addImageSource(imported.file_path);
        imagePath = imported.file_path;
      } catch (_) {
        return;
      }
    }

    await this.loadGridPreview(imagePath);
  }

  private async handleLoadSplit(): Promise<void> {
    if (this.workflowState === "previewingFrames") {
      this.returnToPreSplitState();
      return;
    }
    if (!this.canRunSpriteAction("splitFrames")) return;

    this.syncGeneratedImages();

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
  }

  private async loadGridPreview(imagePath: string): Promise<boolean> {
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

      this.sheetImage = await this.loadImageElement(imagePath);
      this.sheetImagePath = imagePath;
      this.autoBounds = null;
      this.gridLineDrag = null;
      this.hoveredGridLine = null;
      this.clearSelectedBounds();
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
      alert("网格预览失败: " + String(err));
      return false;
    }
  }

  private loadImageElement(imagePath: string): Promise<HTMLImageElement> {
    return loadHtmlImageFromPath(imagePath);
  }

  private renderGridPreviewFromCurrentImage(): void {
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
  }

  private drawGridPreview(rows: number, cols: number): void {
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
  }

  private async loadAndSplit(
    imagePath: string,
    rows: number,
    cols: number,
    region: SplitRegion,
    sourceLabel: string = "原图"
  ): Promise<void> {
    this.frameController.stopPlayback();
    this.setWorkflowState("splitting");
    this.clearSplitResult("正在拆分...");
    this.frameController.destroyLoadedFrames();
    this.frameController.clearCanvas();

    try {
      const result = await this.extractFrames(imagePath, this.createGridCrops(region, rows, cols));

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
      alert("加载分割失败: " + String(err));
    }
  }

  private async extractFrames(imagePath: string, crops: FrameCrop[]): Promise<SplitResult> {
    return extractSpriteFrames(imagePath, crops);
  }

  private createGridCrops(region: SplitRegion, rows: number, cols: number): FrameCrop[] {
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
  }

  private createAutoCrops(bounds: AutoBoundsResult, mode: "tight" | "fixed"): FrameCrop[] {
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
  }

  private async loadAutoSplit(
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
      const result = await this.extractFrames(this.sheetImagePath, crops);
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
      alert("自动拆分失败: " + String(err));
    }
  }

  private async applySplitFrames(
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
  }

  private async detectAutoBounds(): Promise<AutoBoundsResult | null> {
    if (!this.canRunSpriteAction("detectBounds")) return null;
    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    const bounds = await this.getAutoBounds(rows, cols);
    if (bounds) {
      this.syncBoundaryControls();
      this.renderGridPreviewFromCurrentImage();
    }
    return bounds;
  }

  private hasDetectedAutoBounds(rows?: number, cols?: number, region?: SplitRegion): boolean {
    if (!this.autoBounds) {
      return false;
    }
    const safeRows = rows ?? normalizeGridSize(this.els.rows.value, 3);
    const safeCols = cols ?? normalizeGridSize(this.els.cols.value, 4);
    const safeRegion = region ?? (this.sheetImage ? this.getValidatedRegion(safeRows, safeCols) : null);
    return Boolean(safeRegion && this.isAutoBoundsValid(safeRows, safeCols, safeRegion));
  }

  private syncAutoBoundaryOptions(): void {
    const autoEnabled = this.els.autoTrim.checked;
    this.els.autoExpandToggle.style.display = autoEnabled ? "inline-flex" : "none";
    if (!autoEnabled) {
      this.els.autoExpand.checked = false;
    }
  }

  private async getAutoBounds(rows: number, cols: number): Promise<AutoBoundsResult | null> {
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
        sheetImage: this.sheetImage,
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
      alert("自动边界检测失败: " + String(err));
      return null;
    } finally {
      this.settleWorkflowState();
    }
  }

  private isAutoBoundsValid(
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
  }

  private handleBoundaryPointerDown(
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
  }

  private handleAddBoundary(): void {
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
  }

  private handleEditBoundary(): void {
    if (!this.canRunSpriteAction("editBoundary")) return;
    if (this.selectedBoundsIndex === null) {
      alert("请先在画布中选择一个边界框");
      return;
    }
    this.openBoundaryEditor(this.selectedBoundsIndex);
  }

  private handleApplyBoundary(): void {
    if (!this.canRunSpriteAction("editBoundary")) return;
    if (this.applyBoundaryEditorValues(false)) {
      this.boundaryEditOriginal = null;
      this.closeBoundaryEditor(false);
    }
  }

  private handleLiveBoundaryInput(): void {
    if (!this.canRunSpriteAction("editBoundary")) return;
    this.applyBoundaryEditorValues(true);
  }

  private handleCenterBoundaryAnchor(): void {
    if (!this.canRunSpriteAction("editBoundary")) return;
    const bounds = this.autoBounds;
    const index = this.selectedBoundsIndex;
    if (!bounds || index === null || index < 0 || index >= bounds.frameBounds.length) {
      return;
    }

    const frame = bounds.frameBounds[index];
    const editable = frame.empty ? defaultFrameRegion(frame) : frame;
    this.els.boundaryAnchorX.value = String(Math.round(editable.width / 2));
    this.applyBoundaryEditorValues(true);
  }

  private handleDeleteBoundary(): void {
    if (!this.canRunSpriteAction("editBoundary")) return;
    const bounds = this.autoBounds;
    const index = this.selectedBoundsIndex;
    if (!bounds || index === null || index < 0 || index >= bounds.frameBounds.length) {
      alert("请先选择一个边界框");
      return;
    }

    const frame = bounds.frameBounds[index];
    const fallback = defaultFrameRegion(frame);
    frame.x = fallback.x;
    frame.y = fallback.y;
    frame.width = fallback.width;
    frame.height = fallback.height;
    frame.anchorX = getRegionCenterX(fallback.x, fallback.width);
    frame.empty = true;
    recalculateAutoBounds(bounds);
    this.boundaryEditOriginal = null;
    this.closeBoundaryEditor(false);
    this.afterBoundaryChanged();
  }

  private ensureEditableBounds(
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
  }

  private selectBoundary(index: number, openEditor: boolean): void {
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
  }

  private switchSelectedBoundary(delta: number): boolean {
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
  }

  private openBoundaryEditor(index: number): void {
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
  }

  private closeBoundaryEditor(revert: boolean = false): void {
    if (revert) {
      this.restoreBoundaryEditOriginal();
    }
    this.boundaryEditOriginal = null;
    this.syncBoundaryControls();
  }

  private restoreBoundaryEditOriginal(): void {
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
  }

  private applyBoundaryEditorValues(strict: boolean): boolean {
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
    const fallback = frame.empty ? defaultFrameRegion(frame) : frame;
    const edited = readBoundaryEditorValues(
      this.els,
      frame,
      fallback,
      this.getBoundaryLimitRegion(),
      strict
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
  }

  private getBoundaryLimitRegion(): SplitRegion {
    if (!this.sheetImage) {
      return { x: 0, y: 0, width: 1, height: 1 };
    }
    return {
      x: 0,
      y: 0,
      width: this.sheetImage.naturalWidth,
      height: this.sheetImage.naturalHeight,
    };
  }

  private clearSelectedBounds(revertEditor: boolean = true): void {
    if (revertEditor) {
      this.closeBoundaryEditor(true);
    } else {
      this.closeBoundaryEditor(false);
    }
    this.selectedBoundsIndex = null;
    this.syncBoundaryControls();
  }

  private returnToPreSplitState(): void {
    this.frameController.stopPlayback();
    this.frameController.destroyLoadedFrames();
    this.frameController.clearCanvas();
    this.clearSplitResult("确认网格后点击拆分帧");
    this.clearSelectedBounds(false);
    this.settleWorkflowState();
    this.syncRegionInputs();

    if (this.sheetImage) {
      const rows = normalizeGridSize(this.els.rows.value, 3);
      const cols = normalizeGridSize(this.els.cols.value, 4);
      this.drawGridPreview(rows, cols);
    } else {
      this.els.placeholder.textContent = "动画预览区域";
      this.els.placeholder.style.display = "block";
      this.els.canvas.closest(".preview-area")?.classList.remove("has-image");
      this.els.frameSizeInfo.textContent = "尺寸: -";
    }
  }

  private syncSplitModeControls(): void {
    if (!this.els) return;
    syncSplitModeControlsUi(this.els, this.workflowState, this.getSpriteWorkflowContext());
  }

  private syncSpriteWorkflowControls(): void {
    this.syncBoundaryControls();
  }

  private syncBoundaryControls(): void {
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
  }

  private createBoundaryFromCell(index: number): void {
    const bounds = this.autoBounds;
    if (!bounds || index < 0 || index >= bounds.frameBounds.length) {
      return;
    }

    const frame = bounds.frameBounds[index];
    const fallback = defaultFrameRegion(frame);
    frame.x = fallback.x;
    frame.y = fallback.y;
    frame.width = fallback.width;
    frame.height = fallback.height;
    frame.anchorX = getRegionCenterX(fallback.x, fallback.width);
    frame.empty = false;
    recalculateAutoBounds(bounds);
  }

  private afterBoundaryChanged(): void {
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
  }

  private getCanvasNaturalTolerance(): number {
    if (!this.sheetImage) return 4;
    const rect = this.els.canvas.getBoundingClientRect();
    if (rect.width <= 0) return 4;
    const naturalPerCssPixel = this.sheetImage.naturalWidth / rect.width;
    return Math.max(2, naturalPerCssPixel * 6);
  }

  private getCanvasDisplayScale(): number {
    if (!this.sheetImage) return 1;
    const rect = this.els.canvas.getBoundingClientRect();
    return rect.width / Math.max(1, this.sheetImage.naturalWidth);
  }

  private getActiveGridLines(rows: number, cols: number, region: SplitRegion): GridLines {
    this.gridLines = normalizeGridLines(this.gridLines, rows, cols, region);
    return this.gridLines;
  }

  private getGridCellRects(rows: number, cols: number, region: SplitRegion): SplitRegion[] {
    return createGridCellRects(rows, cols, region, this.getActiveGridLines(rows, cols, region));
  }

  private getCurrentGridSignature(rows: number, cols: number, region: SplitRegion): string {
    return getGridLinesSignature(rows, cols, region, this.getActiveGridLines(rows, cols, region));
  }

  private resetGridLines(render: boolean = true): void {
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
  }

  private releaseCanvasPointer(pointerId: number): void {
    if (this.els.canvas.hasPointerCapture(pointerId)) {
      this.els.canvas.releasePointerCapture(pointerId);
    }
  }

  private invalidateAutoBounds(render: boolean = true): void {
    this.autoBounds = null;
    this.clearSelectedBounds();
    if (render) {
      this.renderGridPreviewFromCurrentImage();
    }
  }

  private handleRegionInput(): void {
    if (!this.canRunSpriteAction("editRegion")) return;
    if (!this.sheetImage) return;

    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    const current = this.getValidatedRegion(rows, cols);
    this.splitRegion = clampSplitRegion(
      {
        x: parseRegionNumber(this.els.regionX.value, current.x),
        y: parseRegionNumber(this.els.regionY.value, current.y),
        width: parseRegionNumber(this.els.regionW.value, current.width),
        height: parseRegionNumber(this.els.regionH.value, current.height),
      },
      rows,
      cols,
      this.sheetImage.naturalWidth,
      this.sheetImage.naturalHeight
    );
    this.gridLines = null;
    this.hoveredGridLine = null;
    this.invalidateAutoBounds(false);
    this.syncRegionInputs();
    this.renderGridPreviewFromCurrentImage();
  }

  private handleRegionPointerDown(event: PointerEvent): void {
    if (!this.canRunSpriteAction("editRegion")) return;
    if (!this.sheetImage) return;

    const point = getCanvasNaturalPoint(
      event,
      this.els.canvas,
      this.sheetImage.naturalWidth,
      this.sheetImage.naturalHeight
    );
    if (!point) return;

    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    const startRegion = this.getValidatedRegion(rows, cols);
    const gridLines = this.getActiveGridLines(rows, cols, startRegion);
    const hitMode = hitTestRegion(point.x, point.y, this.splitRegion, this.getCanvasDisplayScale());
    const isRegionResizeHandle = Boolean(hitMode && hitMode !== "move");
    if (!isRegionResizeHandle) {
      const gridLineHit = hitTestGridLine(
        point.x,
        point.y,
        rows,
        cols,
        startRegion,
        gridLines,
        this.getCanvasNaturalTolerance()
      );
      if (gridLineHit) {
        this.gridLineDrag = {
          ...gridLineHit,
          pointerId: event.pointerId,
        };
        this.hoveredGridLine = gridLineHit;
        this.els.canvas.setPointerCapture(event.pointerId);
        this.els.canvas.style.cursor = cursorForGridLine(gridLineHit) ?? "default";
        this.drawGridPreview(rows, cols);
        event.preventDefault();
        return;
      }
    }
    if (
      !isRegionResizeHandle &&
      this.handleBoundaryPointerDown(point.x, point.y, rows, cols, startRegion)
    ) {
      event.preventDefault();
      return;
    }
    const mode = hitMode && !(
      hitMode === "move" &&
      isFullRegion(startRegion, this.sheetImage.naturalWidth, this.sheetImage.naturalHeight)
    )
      ? hitMode
      : "new";

    this.regionDrag = {
      mode,
      startX: point.x,
      startY: point.y,
      startRegion,
      pointerId: event.pointerId,
    };
    this.els.canvas.setPointerCapture(event.pointerId);
    event.preventDefault();
  }

  private handleRegionPointerMove(event: PointerEvent): void {
    if (!this.canRunSpriteAction("editRegion")) {
      this.els.canvas.style.cursor = "default";
      return;
    }
    if (!this.sheetImage) return;

    const point = getCanvasNaturalPoint(
      event,
      this.els.canvas,
      this.sheetImage.naturalWidth,
      this.sheetImage.naturalHeight
    );
    if (!point) return;

    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    const gridDrag = this.gridLineDrag;
    if (gridDrag && gridDrag.pointerId === event.pointerId) {
      const region = this.getValidatedRegion(rows, cols);
      this.gridLines = moveGridLine(
        this.gridLines,
        rows,
        cols,
        region,
        gridDrag.axis,
        gridDrag.lineIndex,
        gridDrag.axis === "x" ? point.x : point.y
      );
      this.hoveredGridLine = {
        axis: gridDrag.axis,
        lineIndex: gridDrag.lineIndex,
      };
      this.invalidateAutoBounds(false);
      this.drawGridPreview(rows, cols);
      this.els.canvas.style.cursor = cursorForGridLine(this.hoveredGridLine) ?? "default";
      event.preventDefault();
      return;
    }

    const drag = this.regionDrag;
    if (!drag) {
      const regionHit = hitTestRegion(point.x, point.y, this.splitRegion, this.getCanvasDisplayScale());
      const isRegionResizeHandle = Boolean(regionHit && regionHit !== "move");
      const region = this.getValidatedRegion(rows, cols);
      const gridLineHit = !isRegionResizeHandle
        ? hitTestGridLine(
          point.x,
          point.y,
          rows,
          cols,
          region,
          this.getActiveGridLines(rows, cols, region),
          this.getCanvasNaturalTolerance()
        )
        : null;
      if (!sameGridLineHit(this.hoveredGridLine, gridLineHit)) {
        this.hoveredGridLine = gridLineHit;
        this.drawGridPreview(rows, cols);
      }
      this.els.canvas.style.cursor = isRegionResizeHandle
        ? cursorForDragMode(regionHit)
        : cursorForGridLine(gridLineHit) ?? cursorForDragMode(regionHit);
      return;
    }

    this.splitRegion = regionFromDrag(
      point.x,
      point.y,
      rows,
      cols,
      this.sheetImage.naturalWidth,
      this.sheetImage.naturalHeight,
      drag
    );
    if (drag.mode !== "move") {
      this.gridLines = null;
      this.hoveredGridLine = null;
    }
    this.invalidateAutoBounds(false);
    this.syncRegionInputs();
    this.renderGridPreviewFromCurrentImage();
    event.preventDefault();
  }

  private handleRegionPointerUp(event: PointerEvent): void {
    if (!this.canRunSpriteAction("editRegion")) return;

    if (this.gridLineDrag && this.gridLineDrag.pointerId === event.pointerId) {
      this.gridLineDrag = null;
      this.releaseCanvasPointer(event.pointerId);
      if (!this.sheetImage) {
        this.els.canvas.style.cursor = "default";
        return;
      }
      const rows = normalizeGridSize(this.els.rows.value, 3);
      const cols = normalizeGridSize(this.els.cols.value, 4);
      const point = getCanvasNaturalPoint(
        event,
        this.els.canvas,
        this.sheetImage.naturalWidth,
        this.sheetImage.naturalHeight
      );
      const region = this.getValidatedRegion(rows, cols);
      this.hoveredGridLine = point
        ? hitTestGridLine(
          point.x,
          point.y,
          rows,
          cols,
          region,
          this.getActiveGridLines(rows, cols, region),
          this.getCanvasNaturalTolerance()
        )
        : null;
      this.els.canvas.style.cursor = cursorForGridLine(this.hoveredGridLine) ?? "default";
      this.drawGridPreview(rows, cols);
      return;
    }

    if (!this.regionDrag || this.regionDrag.pointerId !== event.pointerId) return;

    this.regionDrag = null;
    this.releaseCanvasPointer(event.pointerId);
    if (!this.sheetImage) {
      this.els.canvas.style.cursor = "default";
      return;
    }
    const point = getCanvasNaturalPoint(
      event,
      this.els.canvas,
      this.sheetImage.naturalWidth,
      this.sheetImage.naturalHeight
    );
    this.els.canvas.style.cursor = point
      ? cursorForDragMode(hitTestRegion(point.x, point.y, this.splitRegion, this.getCanvasDisplayScale()))
      : "default";
  }

  private setFullRegion(render: boolean = true, force: boolean = false): void {
    if (!force && !this.canRunSpriteAction("editRegion") && this.sheetImage) return;
    if (!this.sheetImage) {
      this.splitRegion = null;
      this.syncRegionInputs();
      return;
    }

    this.splitRegion = {
      x: 0,
      y: 0,
      width: this.sheetImage.naturalWidth,
      height: this.sheetImage.naturalHeight,
    };
    this.gridLines = null;
    this.hoveredGridLine = null;
    this.invalidateAutoBounds(false);
    this.syncRegionInputs();
    if (render) {
      this.renderGridPreviewFromCurrentImage();
    }
  }

  private getValidatedRegion(rows: number, cols: number): SplitRegion {
    if (!this.sheetImage) {
      return { x: 0, y: 0, width: 1, height: 1 };
    }

    this.splitRegion = clampSplitRegion(
      this.splitRegion || {
        x: 0,
        y: 0,
        width: this.sheetImage.naturalWidth,
        height: this.sheetImage.naturalHeight,
      },
      rows,
      cols,
      this.sheetImage.naturalWidth,
      this.sheetImage.naturalHeight
    );
    return this.splitRegion;
  }

  private syncRegionInputs(): void {
    const enabled = Boolean(this.sheetImage && this.splitRegion) && this.canRunSpriteAction("editRegion");
    [
      this.els.regionX,
      this.els.regionY,
      this.els.regionW,
    this.els.regionH,
    ].forEach((input) => {
      input.disabled = !enabled;
    });
    this.els.regionFull.disabled = !enabled;
    this.els.resetGridLines.disabled = !enabled;
    this.els.autoTrim.disabled = !enabled;
    this.els.autoExpand.disabled = !enabled;
    this.els.autoBgMode.disabled = !enabled;
    this.els.autoTrimMode.disabled = !enabled;
    this.els.autoThreshold.disabled = !enabled;
    this.els.detectBounds.disabled = !enabled;
    this.syncBoundaryControls();

    if (!enabled || !this.splitRegion || !this.sheetImage) {
      this.els.regionX.value = "0";
      this.els.regionY.value = "0";
      this.els.regionW.value = "0";
      this.els.regionH.value = "0";
      return;
    }

    this.els.regionX.max = String(this.sheetImage.naturalWidth - 1);
    this.els.regionY.max = String(this.sheetImage.naturalHeight - 1);
    this.els.regionW.max = String(this.sheetImage.naturalWidth);
    this.els.regionH.max = String(this.sheetImage.naturalHeight);
    this.els.regionX.value = String(this.splitRegion.x);
    this.els.regionY.value = String(this.splitRegion.y);
    this.els.regionW.value = String(this.splitRegion.width);
    this.els.regionH.value = String(this.splitRegion.height);
  }

  private syncGeneratedImages(options: { selectLatest?: boolean; applyPreferredGrid?: boolean } = {}): void {
    syncGeneratedImageSources({
      els: this.els,
      generatorPage: this.generatorPage,
      locked: !this.canRunSpriteAction("selectImage"),
      selectLatest: options.selectLatest,
      applyPreferredGrid: options.applyPreferredGrid,
      onApplyPreferredGrid: () => this.applyPreferredGridFromGenerator(false),
    });
  }

  private applyPreferredGridFromGenerator(render: boolean = true): void {
    const grid = this.generatorPage.getPreferredSpriteGrid();
    this.setGridSize(grid.rows, grid.cols, render);
  }

  private setGridSize(rows: number, cols: number, render: boolean = true): void {
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
    this.updateGridPresetState(safeRows, safeCols);
    if (render) {
      this.renderGridPreviewFromCurrentImage();
    }
  }

  private updateGridPresetState(rows?: number, cols?: number): void {
    updateGridPresetStateUi(this.els, rows, cols);
  }

  private resetFrames(message: string = "动画预览区域"): void {
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
  }

  private clearSplitResult(message: string): void {
    this.frameController.clearFrameList(message);
    this.syncBoundaryControls();
  }

  private renderFallbackPreview(): boolean {
    if (!this.sheetImage) {
      return false;
    }
    const rows = normalizeGridSize(this.els.rows.value, 3);
    const cols = normalizeGridSize(this.els.cols.value, 4);
    this.drawGridPreview(rows, cols);
    return true;
  }

  // ==================== 导出 ====================

  private async handleExport(): Promise<void> {
    if (!this.canRunSpriteAction("exportFrames")) return;
    await handleSpriteExport({
      frames: this.frameController.getFrames(),
      selectedIndices: this.frameController.getSelectedIndices(),
      sheetImagePath: this.sheetImagePath,
      fps: this.frameController.getFps(),
    });
  }
}

function isEditableKeyboardTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  if (target.isContentEditable || target.closest("[contenteditable='true']")) {
    return true;
  }
  return ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}

const BOUNDARY_EDITOR_INPUT_IDS = new Set([
  "boundary-left-input",
  "boundary-top-input",
  "boundary-right-input",
  "boundary-bottom-input",
  "boundary-anchor-x-input",
]);

function getBoundaryEditorInputTarget(target: EventTarget | null): HTMLInputElement | null {
  if (!(target instanceof HTMLInputElement)) {
    return null;
  }
  return BOUNDARY_EDITOR_INPUT_IDS.has(target.id) ? target : null;
}
